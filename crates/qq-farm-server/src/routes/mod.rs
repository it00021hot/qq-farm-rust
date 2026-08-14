//! 路由模块。
//!
//! 1:1 对应原 `controllers/admin/*-routes.ts`。

pub mod account;
pub mod activity_center;
pub mod admin;
pub mod auth;
pub mod commerce;
pub mod farm;
pub mod friend;
pub mod wx_login;

use axum::http::HeaderMap;

use qq_farm_core::services::account_resolver::normalize_account_ref;

/// 共享 helper：解析 account_id（query > header > env fallback）
///
/// 各 routes 里的简化版统一收敛到这里，输出经 [`normalize_account_ref`] 归一化。
pub fn resolve_account_id(
    _ctx: &AdminContext,
    headers: &HeaderMap,
    query_id: Option<&str>,
) -> String {
    let raw = if let Some(id) = query_id {
        if !id.is_empty() {
            id.to_string()
        } else {
            String::new()
        }
    } else if let Some(v) = headers.get("x-account-id").and_then(|v| v.to_str().ok()) {
        v.to_string()
    } else {
        String::new()
    };
    let normalized = normalize_account_ref(Some(&serde_json::Value::String(raw.clone())));
    if !normalized.is_empty() {
        return normalized;
    }
    // fallback：FARM_ACCOUNT_ID env
    std::env::var("FARM_ACCOUNT_ID").unwrap_or_default()
}

/// 严格版：缺失时返回 `BadRequest`
///
/// 用于 friend / activity_center / commerce 等需要确保 account_id 存在的路由。
pub fn resolve_account_id_required(
    ctx: &AdminContext,
    headers: &HeaderMap,
    query_id: Option<&str>,
) -> Result<String, crate::context::ApiError> {
    let id = resolve_account_id(ctx, headers, query_id);
    if id.is_empty() {
        Err(crate::context::ApiError::BadRequest(
            "missing x-account-id".to_string(),
        ))
    } else {
        ensure_account_access(ctx, headers, &id)?;
        Ok(id)
    }
}

/// 账号 ACL：admin 全放行；普通用户只能访问自己的账号
pub fn account_accessible(ctx: &AdminContext, headers: &HeaderMap, account_id: &str) -> bool {
    if account_id.is_empty() {
        return true;
    }
    let Some(sess) = current_session(ctx, headers) else {
        return false;
    };
    if sess.role == "admin" {
        return true;
    }
    qq_farm_core::models::store::accounts::get_accounts()
        .into_iter()
        .any(|a| a.id == account_id && a.username == sess.username)
}

/// 无权限时 Forbidden
pub fn ensure_account_access(
    ctx: &AdminContext,
    headers: &HeaderMap,
    account_id: &str,
) -> Result<(), crate::context::ApiError> {
    if account_accessible(ctx, headers, account_id) {
        Ok(())
    } else {
        Err(crate::context::ApiError::Forbidden(
            "无权访问该账号".to_string(),
        ))
    }
}

pub fn current_session(ctx: &AdminContext, headers: &HeaderMap) -> Option<crate::sessions::SessionInfo> {
    let token = headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if token.is_empty() {
        return None;
    }
    ctx.sessions.get(token)
}

/// 当前用户名下账号 ID（含管理员；对齐 getAccessibleAccountIds）
pub fn accessible_account_ids(ctx: &AdminContext, headers: &HeaderMap) -> Vec<String> {
    let Some(sess) = current_session(ctx, headers) else {
        return Vec::new();
    };
    qq_farm_core::models::store::accounts::get_accounts()
        .into_iter()
        .filter(|a| a.username == sess.username)
        .map(|a| a.id)
        .collect()
}

use std::sync::Arc;

use axum::{extract::Request, middleware, Router};

use crate::context::AdminContext;

/// 构造全部 admin 路由
pub fn build(ctx: Arc<AdminContext>) -> Router<Arc<AdminContext>> {
    let ctx_for_inject = ctx.clone();
    let ctx_for_admin = ctx.clone();
    let ctx_for_auth = ctx.clone();

    // 顶层注入 ctx 到 request extensions
    let inject_layer = middleware::from_fn(move |mut req: Request, next: axum::middleware::Next| {
        let ctx = ctx_for_inject.clone();
        async move {
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
    });

    // admin 鉴权
    let admin_check = middleware::from_fn(move |req: Request, next: axum::middleware::Next| {
        let ctx = ctx_for_admin.clone();
        async move {
            crate::middleware::admin_required_strict_ext(ctx, req, next).await
        }
    });

    // 普通用户鉴权
    let auth_check = middleware::from_fn(move |req: Request, next: axum::middleware::Next| {
        let ctx = ctx_for_auth.clone();
        async move {
            crate::middleware::auth_required_strict_ext(ctx, req, next).await
        }
    });

    // auth 路由不挂 auth_check（login/register 公开）
    // admin 挂 admin_check
    // account / friend 挂 auth_check
    // 其余挂 inject（不需要登录）
    let authed = Router::new()
        .merge(account::router())
        .merge(friend::router())
        .merge(wx_login::router())
        .merge(farm::router())
        .merge(commerce::router())
        .merge(activity_center::router())
        .route_layer(auth_check);

    Router::new()
        .merge(auth::router())
        .merge(authed)
        .merge(admin::router().route_layer(admin_check))
        .merge(admin::public_router())
        .route_layer(inject_layer)
}
