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

use crate::routes::wx_login::WxLoginState;
use crate::sessions::SessionStore;

/// Admin 共享上下文
#[derive(Clone)]
pub struct AdminContext {
    /// Runtime 引擎
    pub engine: Arc<RuntimeEngine>,
    /// Session 存储（token → user）
    pub sessions: SessionStore,
    /// 微信扫码登录 state
    pub wx: WxLoginState,
}

impl AdminContext {
    /// 构造 context
    #[must_use]
    pub fn new(engine: Arc<RuntimeEngine>) -> Self {
        Self {
            engine,
            sessions: SessionStore::new(),
            wx: WxLoginState::new(),
        }
    }

    /// 转为 qq-farm-app 上下文。
    #[must_use]
    pub fn app_context(&self) -> qq_farm_app::AppContext {
        qq_farm_app::AppContext::new(self.engine.clone())
    }

    /// 构造 context（带 sessions）
    #[must_use]
    pub fn with_sessions(engine: Arc<RuntimeEngine>, sessions: SessionStore) -> Self {
        Self {
            engine,
            sessions,
            wx: WxLoginState::new(),
        }
    }
}

impl std::fmt::Debug for AdminContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminContext").finish_non_exhaustive()
    }
}

/// 占位 AdminContext（仅给 route_layer 中间件用）
impl AdminContext {
    /// 空 ctx（仅给 from_fn_with_state 用；实际请求用真实 ctx）
    pub fn dummy() -> Arc<Self> {
        Arc::new(Self {
            engine: Arc::new(qq_farm_core::runtime::engine::RuntimeEngine::assemble(
                qq_farm_core::runtime::engine::EngineConfig::default(),
            )),
            sessions: crate::sessions::SessionStore::new(),
            wx: WxLoginState::new(),
        })
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
    #[error("{0}")]
    Forbidden(String),
    #[error("bad gateway: {0}")]
    BadGateway(String),
    #[error("internal: {0}")]
    Internal(String),
    /// 对齐原 bot `handleApiError`：worker 未运行时 HTTP 200 + `{ok:false, error:"账号未运行"}`，避免前端 4xx 连弹 toast
    #[error("账号未运行")]
    AccountNotRunning,
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        use axum::Json;
        let (status, msg) = match &self {
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m.as_str()),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m.as_str()),
            Self::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.as_str()),
            Self::Forbidden(m) => (StatusCode::FORBIDDEN, m.as_str()),
            Self::BadGateway(m) => (StatusCode::BAD_GATEWAY, m.as_str()),
            Self::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.as_str()),
            Self::AccountNotRunning => (StatusCode::OK, "账号未运行"),
        };
        (
            status,
            Json(if matches!(self, Self::AccountNotRunning) {
                serde_json::json!({
                    "ok": false,
                    "error": msg,
                    "errorCode": "ACCOUNT_OFFLINE",
                })
            } else {
                serde_json::json!({
                    "ok": false,
                    "error": msg,
                })
            }),
        )
            .into_response()
    }
}

impl From<qq_farm_core::Error> for ApiError {
    fn from(e: qq_farm_core::Error) -> Self {
        match e {
            qq_farm_core::Error::NotFound(m) => Self::NotFound(m),
            qq_farm_core::Error::Business(m) => Self::BadRequest(m),
            qq_farm_core::Error::Network(n) => Self::BadGateway(n.to_string()),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<qq_farm_app::AppError> for ApiError {
    fn from(e: qq_farm_app::AppError) -> Self {
        match e {
            qq_farm_app::AppError::NotFound(m) => Self::NotFound(m),
            qq_farm_app::AppError::BadRequest(m) => Self::BadRequest(m),
            qq_farm_app::AppError::Forbidden(m) => Self::Forbidden(m),
            qq_farm_app::AppError::Internal(m) => Self::Internal(m),
            qq_farm_app::AppError::AccountNotRunning => Self::AccountNotRunning,
            qq_farm_app::AppError::Core(core) => Self::Internal(core.to_string()),
        }
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

/// 对齐原 bot：`{ ok: true, data }`
pub fn ok_data<T: serde::Serialize>(data: T) -> ApiResult<serde_json::Value> {
    Ok(Json(serde_json::json!({ "ok": true, "data": data })))
}
