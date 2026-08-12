//! Admin 路由 context（共享状态）。
//!
//! 1:1 对应原 `controllers/admin/context.ts`（33 行）。
//!
//! ## 与原 TS 的差异
//!
//! - 原 TS 用 `app.locals` 存 ctx；本实现用 axum 的 `State<Arc<AdminContext>>`
//! - 原 TS 的 dataProvider 本质是 runtime 引擎；本实现直接拿 `Arc<RuntimeEngine>`

use std::sync::Arc;

use axum::Json;
use qq_farm_core::runtime::engine::RuntimeEngine;

/// Admin 共享上下文
#[derive(Clone)]
pub struct AdminContext {
    /// Runtime 引擎
    pub engine: Arc<RuntimeEngine>,
}

impl AdminContext {
    /// 构造 context
    #[must_use]
    pub fn new(engine: Arc<RuntimeEngine>) -> Self {
        Self { engine }
    }
}

impl std::fmt::Debug for AdminContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminContext").finish_non_exhaustive()
    }
}

/// HTTP 错误（统一响应）
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("not implemented")]
    NotImplemented,
    #[error("internal: {0}")]
    Internal(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        use axum::Json;
        let (status, msg) = match &self {
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m.as_str()),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m.as_str()),
            Self::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.as_str()),
            Self::NotImplemented => (StatusCode::NOT_IMPLEMENTED, "not implemented"),
            Self::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.as_str()),
        };
        (
            status,
            Json(serde_json::json!({
                "ok": false,
                "error": msg,
            })),
        )
            .into_response()
    }
}

/// HTTP 统一响应包装
pub type ApiResult<T> = Result<Json<T>, ApiError>;

/// 构造 ok 响应
pub fn ok<T: serde::Serialize>(value: T) -> ApiResult<T> {
    Ok(Json(value))
}

/// 构造 ok 响应（无 data）
pub fn ok_empty() -> ApiResult<serde_json::Value> {
    Ok(Json(serde_json::json!({ "ok": true })))
}
