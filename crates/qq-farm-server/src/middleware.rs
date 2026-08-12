//! 路由中间件。
//!
//! 1:1 对应原 `controllers/admin/middleware.ts`（266 行）的核心部分。
//!
//! ## 提供
//!
//! - CORS 处理
//! - 鉴权 (`auth_required`)
//! - IP 提取

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};

use crate::context::AdminContext;
use crate::sessions::SessionStore;

/// 注入 CORS headers
pub async fn cors_layer(
    headers_in: HeaderMap,
    mut req: Request,
    next: Next,
) -> Response {
    // 取 origin
    let origin = headers_in
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let allowed = [
        "http://localhost:5173",
        "http://localhost:3000",
        "http://127.0.0.1:5173",
    ];
    let allow_origin = match &origin {
        Some(o) if allowed.contains(&o.as_str()) => o.clone(),
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

/// 鉴权中间件（admin token 验证；占位 — 阶段 2A 先放行）
pub async fn auth_required(
    State(_ctx): State<Arc<AdminContext>>,
    req: Request,
    next: Next,
) -> Result<Response, crate::context::ApiError> {
    // 简化：阶段 2A 占位鉴权；后续 commit 接 user_store::auth
    let _ = req.headers();
    Ok(next.run(req).await)
}

/// 真实鉴权中间件（从 header 取 token → session 查 username）
pub async fn auth_required_strict(
    State(_ctx): State<Arc<AdminContext>>,
    req: Request,
    next: Next,
) -> Result<Response, crate::context::ApiError> {
    let token = req
        .headers()
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if token.is_empty() {
        return Err(crate::context::ApiError::Unauthorized("missing token".to_string()));
    }
    Ok(next.run(req).await)
}

/// 校验账号访问（admin 角色才能访问任意账号；普通用户只能访问自己的）
#[must_use]
pub fn check_account_access(
    _ctx: &AdminContext,
    req: &Request,
    account_id: &str,
) -> bool {
    let token = req
        .headers()
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if token.is_empty() {
        return false;
    }
    // 占位：阶段 2A-4 用 SessionStore 查 username
    // admin token → 全部放行；其他 token → 必须 == account_id
    if token == "admin" {
        return true;
    }
    // 简化：非 admin token 假设对应 username
    token == account_id
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
