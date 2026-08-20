//! 微信扫码登录路由 — 6 端点，全部转发 [`qq_farm_app::wx_login::WxLoginHub`]。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{AdminContext, ApiError};
use qq_farm_app::wx_login;
use qq_farm_core::constants::WX_MINI_APP_ID;

pub use qq_farm_app::wx_login::WxLoginHub as WxLoginState;

pub const TARGET_APP_ID: &str = WX_MINI_APP_ID;

/// 构造 wx-login 路由（鉴权由 `routes::build()` 覆盖）
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/wx-login/tasks", post(create_task))
        .route("/api/wx-login/tasks/{task_id}/qr", get(get_qr))
        .route("/api/wx-login/tasks/{task_id}", delete(delete_task))
        .route("/api/wx-login/tasks/{task_id}/status", get(get_status))
        .route("/api/wx-login/tasks/{task_id}/confirm", post(confirm_task))
        .route("/api/wx-login/tasks/{task_id}/code", post(consume_code))
        .route("/api/wx-login/quick-tasks", post(create_quick_task))
        .route("/api/wx-login/quick-tasks/{session_id}/confirm", post(confirm_quick_task))
}

#[derive(Debug, Deserialize, Default)]
struct CreateTaskBody {
    #[serde(default)]
    app_id: Option<String>,
}

fn owner_from_headers(ctx: &AdminContext, headers: &axum::http::HeaderMap) -> String {
    let token = headers.get("x-admin-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    ctx.sessions.get_username(token).filter(|u| !u.is_empty()).unwrap_or_else(|| token.to_string())
}

async fn create_task(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    body: Option<Json<CreateTaskBody>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    if let Some(ref app) = body.app_id {
        if app != TARGET_APP_ID {
            return Err(ApiError::BadRequest("Unsupported app_id".to_string()));
        }
    }
    let owner = owner_from_headers(&ctx, &headers);
    let r = wx_login::create_task_for(&ctx.wx, &owner).await.map_err(ApiError::from)?;
    let qr_url = format!("/api/wx-login/tasks/{}/qr", r.task_id);
    Ok(Json(json!({
        "ok": true,
        "data": {
            "task_id": r.task_id,
            "app_id": r.app_id,
            "status": r.status,
            "expires_at": r.expires_at,
            "qr_url": qr_url,
        }
    })))
}

async fn get_qr(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let jpeg = wx_login::qr_jpeg(&ctx.wx, &task_id, Some(&owner)).map_err(ApiError::from)?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg")], jpeg).into_response())
}

async fn delete_task(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    wx_login::qr_jpeg(&ctx.wx, &task_id, Some(&owner)).map_err(ApiError::from)?;
    wx_login::destroy_task_for(&ctx.wx, &task_id, Some(&owner));
    Ok(Json(json!({ "ok": true })))
}

async fn get_status(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let r =
        wx_login::poll_status_for(&ctx.wx, &task_id, Some(&owner)).await.map_err(ApiError::from)?;
    Ok(Json(json!({
        "ok": true,
        "data": {
            "task_id": r.task_id,
            "app_id": r.app_id,
            "status": r.status,
            "expires_at": r.expires_at,
        }
    })))
}

async fn confirm_task(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let r = wx_login::confirm_for(&ctx.wx, &task_id, Some(&owner)).await.map_err(ApiError::from)?;
    Ok(Json(json!({
        "ok": true,
        "data": {
            "task_id": r.task_id,
            "app_id": r.app_id,
            "status": r.status,
            "expires_at": r.expires_at,
        }
    })))
}

async fn consume_code(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let r =
        wx_login::issue_code_for(&ctx.wx, &task_id, Some(&owner)).await.map_err(ApiError::from)?;
    Ok(Json(json!({
        "ok": true,
        "data": {
            "openid": r.openid,
            "app_id": r.app_id,
            "code": r.code,
            "err_msg": "login:ok",
        }
    })))
}

#[derive(Debug, Deserialize)]
struct QuickConfirmBody {
    redirect_url: String,
}

async fn create_quick_task(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let r = wx_login::create_quick_session_for(&ctx.wx, &owner).await.map_err(ApiError::from)?;
    Ok(Json(json!({
        "ok": true,
        "data": {
            "session_id": r.session_id,
            "appid": r.app_id,
            "scope": r.scope,
            "redirect_uri": r.redirect_uri,
            "state": r.state,
            "ports": r.ports,
            "expires_at": r.expires_at,
        }
    })))
}

async fn confirm_quick_task(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
    Json(body): Json<QuickConfirmBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let r =
        wx_login::confirm_quick_session_for(&ctx.wx, &session_id, &body.redirect_url, Some(&owner))
            .await
            .map_err(ApiError::from)?;
    Ok(Json(json!({
        "ok": true,
        "data": {
            "openid": r.openid,
            "app_id": r.app_id,
            "code": r.code,
            "err_msg": "login:ok",
        }
    })))
}
