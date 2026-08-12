//! 微信扫码登录路由 — 6 端点（占位骨架）。
//!
//! 1:1 对应原 `controllers/admin/wx-login-routes.ts`（55 行）。
//!
//! 注：真实网络协议层 `services/wx_login/native_protocol.rs` 已完成。
//! 本路由只暴露 HTTP 入口；真实请求留到 2B 联调阶段。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_empty, AdminContext, ApiError, ApiResult};

/// 构造 wx-login 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/wx-login/tasks", post(create_task))
        .route("/api/wx-login/tasks/{task_id}/qr", get(get_qr))
        .route("/api/wx-login/tasks/{task_id}", delete(delete_task))
        .route("/api/wx-login/tasks/{task_id}/status", get(get_status))
        .route("/api/wx-login/tasks/{task_id}/confirm", post(confirm_task))
        .route("/api/wx-login/tasks/{task_id}/code", post(consume_code))
}

#[derive(Debug, Deserialize)]
struct CreateTaskBody {
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    owner: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfirmBody {
    #[serde(default)]
    action: Option<String>,
}

async fn create_task(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<CreateTaskBody>,
) -> ApiResult<serde_json::Value> {
    let app_id = body.app_id.unwrap_or_else(|| "1112386029".to_string());
    // 占位：阶段 2A-3 接真实 MiniProgramLoginSession::request_login_code
    let _ = app_id;
    Err(ApiError::NotImplemented)
}

async fn get_qr(
    State(_ctx): State<Arc<AdminContext>>,
    Path(_task_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    Err(ApiError::NotImplemented)
}

async fn delete_task(
    State(_ctx): State<Arc<AdminContext>>,
    Path(_task_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    ok_empty()
}

async fn get_status(
    State(_ctx): State<Arc<AdminContext>>,
    Path(_task_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    ok(json!({ "ok": true, "status": "expired" }))
}

async fn confirm_task(
    State(_ctx): State<Arc<AdminContext>>,
    Path(_task_id): Path<String>,
    Json(_body): Json<ConfirmBody>,
) -> ApiResult<serde_json::Value> {
    Err(ApiError::NotImplemented)
}

async fn consume_code(
    State(_ctx): State<Arc<AdminContext>>,
    Path(_task_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    Err(ApiError::NotImplemented)
}
