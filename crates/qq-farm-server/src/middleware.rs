//! 路由中间件。
//!
//! 1:1 对应原 `controllers/admin/middleware.ts`（266 行）的核心部分。
//!
//! ## 提供
//!
//! - CORS 处理
//! - 鉴权（`auth_required_strict_ext` / `admin_required_strict_ext`）
//! - IP 提取

use std::sync::Arc;

use axum::{
    extract::{Extension, Request, State},
    http::{header, HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};

use crate::config::ServerConfig;
use crate::context::AdminContext;
use crate::sessions::SessionStore;

/// 注入 CORS headers（允许列表来自 [`ServerConfig`]，缺省走 Default）
pub async fn cors_layer(
    headers_in: HeaderMap,
    mut req: Request,
    next: Next,
) -> Response {
    let cfg = ServerConfig::from_env();
    let origin = headers_in
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let allow_origin = match &origin {
        Some(o) if cfg.allows_origin(o) => o.clone(),
        None => "*".to_string(),
        Some(_) => String::new(),
    };

    // 处理 OPTIONS preflight
    if req.method() == axum::http::Method::OPTIONS {
        let mut resp = Response::new(axum::body::Body::empty());
        let h = resp.headers_mut();
        if !allow_origin.is_empty() {
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_str(&allow_origin).unwrap());
        }
        h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, DELETE, OPTIONS, PUT"));
        h.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Content-Type, x-account-id, x-admin-token, x-proxy-api-key, x-proxy-api-url, x-proxy-app-id"),
        );
        h.insert(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, HeaderValue::from_static("true"));
        h.insert(header::ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("86400"));
        return resp;
    }
    // 注入 header
    req.headers_mut().insert(
        "x-allow-origin",
        HeaderValue::from_str(&allow_origin).unwrap_or(HeaderValue::from_static("")),
    );
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    if !allow_origin.is_empty() {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_str(&allow_origin).unwrap_or(HeaderValue::from_static("")));
    }
    h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, DELETE, OPTIONS, PUT"));
    h.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Content-Type, x-account-id, x-admin-token, x-proxy-api-key, x-proxy-api-url, x-proxy-app-id"),
    );
    h.insert(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, HeaderValue::from_static("true"));
    h.insert(header::ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("86400"));
    resp
}

/// 真实鉴权中间件：x-admin-token → SessionStore 查 → 失败 401
pub async fn auth_required_strict(
    State(ctx): State<Arc<AdminContext>>,
    req: Request,
    next: Next,
) -> Result<Response, crate::context::ApiError> {
    auth_required_strict_ext(ctx, req, next).await
}

/// 真实鉴权中间件（用 Extension 拿 ctx，用于 from_fn 无 state middleware）
pub async fn auth_required_strict_ext(
    ctx: Arc<AdminContext>,
    req: Request,
    next: Next,
) -> Result<Response, crate::context::ApiError> {
    let token = req
        .headers()
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if token.is_empty() {
        return Err(crate::context::ApiError::Unauthorized(
            "missing x-admin-token".to_string(),
        ));
    }
    match ctx.sessions.get(token) {
        Some(info) => {
            ctx.sessions.touch(token);
            let _ = info;
            Ok(next.run(req).await)
        }
        None => Err(crate::context::ApiError::Unauthorized(
            "invalid or expired token".to_string(),
        )),
    }
}

/// Admin-only 鉴权（要求 role=admin）
pub async fn admin_required_strict(
    State(ctx): State<Arc<AdminContext>>,
    req: Request,
    next: Next,
) -> Result<Response, crate::context::ApiError> {
    admin_required_strict_ext(ctx, req, next).await
}

/// Admin-only 鉴权（用 Extension 拿 ctx，用于 from_fn 无 state middleware）
pub async fn admin_required_strict_ext(
    ctx: Arc<AdminContext>,
    req: Request,
    next: Next,
) -> Result<Response, crate::context::ApiError> {
    let token = req
        .headers()
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if token.is_empty() {
        return Err(crate::context::ApiError::Unauthorized(
            "missing x-admin-token".to_string(),
        ));
    }
    match ctx.sessions.get(token) {
        Some(info) if info.role == "admin" => {
            ctx.sessions.touch(token);
            Ok(next.run(req).await)
        }
        Some(_) => Err(crate::context::ApiError::Unauthorized(
            "admin role required".to_string(),
        )),
        None => Err(crate::context::ApiError::Unauthorized(
            "invalid or expired token".to_string(),
        )),
    }
}

/// 校验账号访问权限（admin 全放行；普通用户只能访问自己的）
#[must_use]
pub fn check_account_access(ctx: &AdminContext, req: &Request, account_id: &str) -> bool {
    let token = req
        .headers()
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if token.is_empty() {
        return false;
    }
    let Some(info) = ctx.sessions.get(token) else {
        return false;
    };
    if info.role == "admin" {
        return true;
    }
    // 普通用户：账号.username == session.username
    accounts_username_lookup(account_id)
        .map(|u| u == info.username)
        .unwrap_or(false)
}

/// 查账号的 username 字段
fn accounts_username_lookup(account_id: &str) -> Option<String> {
    qq_farm_core::models::store::accounts::get_accounts()
        .into_iter()
        .find(|a| a.id == account_id)
        .map(|a| a.username.clone())
}

/// 提取 client IP（按 X-Forwarded-For / CF-Connecting-IP）
pub fn extract_client_ip(headers: &HeaderMap) -> String {
    for k in [
        "cf-connecting-ip",
        "x-forwarded-for",
        "x-real-ip",
        "x-client-ip",
    ] {
        if let Some(v) = headers.get(k).and_then(|v| v.to_str().ok()) {
            return v.split(',').next().unwrap_or(v).trim().to_string();
        }
    }
    String::new()
}
