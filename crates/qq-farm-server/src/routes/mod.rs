//! 路由模块。
//!
//! 1:1 对应原 `controllers/admin/*-routes.ts`。

pub mod account;
pub mod activity_center;
pub mod admin;
pub mod auth;
pub mod commerce;
pub mod daily_gifts;
pub mod farm;
pub mod friend;
pub mod game_config;
pub mod wx_login;

use axum::http::HeaderMap;

use qq_farm_app::accounts::AclPolicy;
use qq_farm_core::services::account_resolver::normalize_account_ref;

/// 从面板会话构造 ACL 策略。
pub fn acl_policy_from_session(sess: &crate::sessions::SessionInfo) -> AclPolicy {
    AclPolicy::PanelUser {
        username: sess.username.clone(),
        role: sess.role.clone(),
    }
}

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

/// 解析 account_id 并校验 ACL（允许空 id，供 farm 等路由自行处理缺失场景）。
pub fn resolve_id(
    ctx: &AdminContext,
    headers: &HeaderMap,
    query_id: Option<&str>,
) -> Result<String, crate::context::ApiError> {
    let id = resolve_account_id(ctx, headers, query_id);
    ensure_account_access(ctx, headers, &id)?;
    Ok(id)
}

/// 获取某账号的 WorkerLoop（cloned Arc，handler 异步安全）。
pub fn get_loop(
    ctx: &AdminContext,
    account_id: &str,
) -> Result<std::sync::Arc<qq_farm_core::runtime::worker_loop::WorkerLoop>, crate::context::ApiError> {
    qq_farm_app::farm::require_worker_loop(&ctx.app_context(), account_id).map_err(Into::into)
}

/// 同 [`get_loop`]，语义别名。
pub fn require_worker_loop(
    ctx: &AdminContext,
    account_id: &str,
) -> Result<std::sync::Arc<qq_farm_core::runtime::worker_loop::WorkerLoop>, crate::context::ApiError> {
    get_loop(ctx, account_id)
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
    qq_farm_app::accounts::account_accessible(&acl_policy_from_session(&sess), account_id)
}

/// 无权限时 Forbidden
pub fn ensure_account_access(
    ctx: &AdminContext,
    headers: &HeaderMap,
    account_id: &str,
) -> Result<(), crate::context::ApiError> {
    let Some(sess) = current_session(ctx, headers) else {
        return Err(crate::context::ApiError::Forbidden(
            "无权访问该账号".to_string(),
        ));
    };
    qq_farm_app::accounts::ensure_account_access(&acl_policy_from_session(&sess), account_id)
        .map_err(Into::into)
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

/// 当前会话可访问的账号 ID。
///
/// - admin：全部账号
/// - 普通用户：仅 `username` 匹配自己的账号
pub fn accessible_account_ids(ctx: &AdminContext, headers: &HeaderMap) -> Vec<String> {
    let Some(sess) = current_session(ctx, headers) else {
        return Vec::new();
    };
    qq_farm_app::accounts::accessible_account_ids(&acl_policy_from_session(&sess))
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

    // auth：login/register 等公开（部分 handler 自行校验 token）
    // admin：admin_check
    // account / farm / friend / commerce / activity / wx_login：auth_check
    let authed = Router::new()
        .merge(account::router())
        .merge(friend::router())
        .merge(wx_login::router())
        .merge(farm::router())
        .merge(daily_gifts::router())
        .merge(game_config::router())
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
