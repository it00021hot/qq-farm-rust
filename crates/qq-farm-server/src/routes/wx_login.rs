//! 微信扫码登录路由 — 6 端点（1:1 对应原 `controllers/admin/wx-login-routes.ts` 55 行）。
//!
//! 真实接通 `services::wx_login::WxLoginService`：
//! 1. `POST /api/wx-login/tasks` → create_qr_session（生成 QR）
//! 2. `GET  /api/wx-login/tasks/:id/qr` → 返回 QR jpeg 二进制
//! 3. `GET  /api/wx-login/tasks/:id/status` → poll 状态
//! 4. `POST /api/wx-login/tasks/:id/confirm` → confirm 扫码
//! 5. `POST /api/wx-login/tasks/:id/code` → issue_code 拿到 game auth code
//! 6. `DELETE /api/wx-login/tasks/:id` → destroy
//!
//! ## 响应结构（1:1 对齐原 bot `wx-login-routes.ts`）
//! - 成功：`{ok: true, data: {task_id, app_id, status, expires_at, qr_url, ...}}`
//! - 失败：`{ok: false, error: "..."}`（502 / 404 / 400 / 401）
//!
//! ## 鉴权
//! - 6 端点都强制 `x-admin-token`（由 `routes::build()` 的 `auth_check` 中间件统一覆盖）
//! - task owner 校验（防越权访问别人的 task）
//! - `app_id` 必须匹配 `TARGET_APP_ID`，否则 400

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::json;

use crate::context::{AdminContext, ApiError};
use qq_farm_core::services::wx_login::service::{ScanStatus, WxLoginService, WxLoginSession};

/// 微信登录任务（1:1 对应原 bot 的 Task interface）
struct WxLoginTask {
    /// tokio::Mutex 让 lock().await 可跨 await 持有
    session: tokio::sync::Mutex<WxLoginSession>,
    /// QR jpeg 二进制
    qr_jpeg: Vec<u8>,
    /// task owner（admin username）— 防越权
    owner: String,
    /// 创建时间（毫秒），confirm 后会刷新以给 /code 足够 TTL
    created_at: AtomicI64,
    /// app_id
    app_id: String,
}

/// wx-login state（独立放在 AdminContext 之外的类型）
#[derive(Clone)]
pub struct WxLoginState {
    service: Arc<WxLoginService>,
    tasks: Arc<Mutex<HashMap<String, Arc<WxLoginTask>>>>,
}

impl WxLoginState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: Arc::new(WxLoginService::new()),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for WxLoginState {
    fn default() -> Self {
        Self::new()
    }
}

/// 原 bot 强制使用 `wx5306c5978fdb76e4`（前端 AccountModal 写死）
pub const TARGET_APP_ID: &str = qq_farm_core::constants::WX_MINI_APP_ID;
/// Task TTL（毫秒）— 与原 bot 一致
pub const TASK_TTL_MS: i64 = qq_farm_core::constants::WX_LOGIN_TASK_TTL_MS as i64;

fn now_ms_i64() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 构造 wx-login 路由
/// 鉴权由 `routes::build()` 的 `auth_check` layer 统一覆盖
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/wx-login/tasks", post(create_task))
        .route("/api/wx-login/tasks/{task_id}/qr", get(get_qr))
        .route("/api/wx-login/tasks/{task_id}", delete(delete_task))
        .route("/api/wx-login/tasks/{task_id}/status", get(get_status))
        .route("/api/wx-login/tasks/{task_id}/confirm", post(confirm_task))
        .route("/api/wx-login/tasks/{task_id}/code", post(consume_code))
}

#[derive(Debug, Deserialize, Default)]
struct CreateTaskBody {
    #[serde(default)]
    app_id: Option<String>,
}

fn wx_state_from_ctx(ctx: &AdminContext) -> WxLoginState {
    ctx.wx.clone()
}

/// 从请求头解析 task owner（对齐 TS `owner(req)`：优先 username，回退 token）
fn owner_from_headers(ctx: &AdminContext, headers: &axum::http::HeaderMap) -> String {
    let token = headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    ctx.sessions
        .get_username(token)
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| token.to_string())
}

/// 找 task + 校验 TTL + 校验 owner（删过期/越权 task）
fn find_task(
    ctx: &AdminContext,
    task_id: &str,
    owner: &str,
) -> Result<Arc<WxLoginTask>, ApiError> {
    let wx = wx_state_from_ctx(ctx);
    let task = wx
        .tasks
        .lock()
        .get(task_id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound("Login task not found or expired".to_string()))?;
    let expired = now_ms_i64() - task.created_at.load(Ordering::Relaxed) > TASK_TTL_MS;
    if expired || task.owner != owner {
        let _ = wx.tasks.lock().remove(task_id);
        return Err(ApiError::NotFound(
            "Login task not found or expired".to_string(),
        ));
    }
    Ok(task)
}

async fn create_task(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    body: Option<Json<CreateTaskBody>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let body = body.map(|Json(b)| b).unwrap_or_default();

    // 校验 app_id（如果传了）
    if let Some(ref app) = body.app_id {
        if app != TARGET_APP_ID {
            return Err(ApiError::BadRequest("Unsupported app_id".to_string()));
        }
    }

    let wx = wx_state_from_ctx(&ctx);
    let service = wx.service.clone();

    // 调 service 拿 session + QR
    let (session, qr_jpeg) = service
        .create_qr_session()
        .await
        .map_err(ApiError::BadGateway)?;

    // task_id：原 bot 用 `crypto.randomBytes(32).to_string('hex')` = 64 hex
    // 我们用两个 uuid v4 拼成 64 hex char
    let task_id = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let created_at = now_ms_i64();

    let task = Arc::new(WxLoginTask {
        session: tokio::sync::Mutex::new(session),
        qr_jpeg,
        owner: owner_from_headers(&ctx, &headers),
        created_at: AtomicI64::new(created_at),
        app_id: TARGET_APP_ID.to_string(),
    });
    wx.tasks.lock().insert(task_id.clone(), task);

    let qr_url = format!("/api/wx-login/tasks/{task_id}/qr");
    Ok(Json(json!({
        "ok": true,
        "data": {
            "task_id": task_id,
            "app_id": TARGET_APP_ID,
            "status": "waiting",
            "expires_at": (created_at + TASK_TTL_MS) / 1000,
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
    let task = find_task(&ctx, &task_id, &owner)?;
    Ok((
        [(header::CONTENT_TYPE, "image/jpeg")],
        task.qr_jpeg.clone(),
    )
        .into_response())
}

async fn delete_task(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let wx = wx_state_from_ctx(&ctx);
    let task = wx
        .tasks
        .lock()
        .get(&task_id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound("Login task not found or expired".to_string()))?;
    if task.owner != owner {
        return Err(ApiError::NotFound("Login task not found or expired".to_string()));
    }
    let removed = wx.tasks.lock().remove(&task_id);
    if let Some(t) = removed {
        let mut session = t.session.lock().await;
        wx.service.destroy(&mut *session);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn get_status(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let task = find_task(&ctx, &task_id, &owner)?;
    let wx = wx_state_from_ctx(&ctx);
    let service = wx.service.clone();
    let mut session = task.session.lock().await;
    let status = service.poll(&mut *session).await.map_err(ApiError::BadGateway)?;
    let data = json!({
        "task_id": task_id,
        "app_id": task.app_id,
        "status": status_name(status),
        "expires_at": (task.created_at.load(Ordering::Relaxed) + TASK_TTL_MS) / 1000,
    });
    // 对齐 TS：cancelled / expired 时销毁 task
    if matches!(status, ScanStatus::Cancelled | ScanStatus::Expired) {
        drop(session);
        let removed = wx.tasks.lock().remove(&task_id);
        if let Some(t) = removed {
            let mut s = t.session.lock().await;
            wx.service.destroy(&mut *s);
        }
    }
    Ok(Json(json!({ "ok": true, "data": data })))
}

async fn confirm_task(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let task = find_task(&ctx, &task_id, &owner)?;
    let wx = wx_state_from_ctx(&ctx);
    let service = wx.service.clone();
    let mut session = task.session.lock().await;
    service.confirm(&mut *session).await.map_err(ApiError::BadGateway)?;
    drop(session);
    task.created_at.store(now_ms_i64(), Ordering::Relaxed);
    let data = json!({
        "task_id": task_id,
        "app_id": task.app_id,
        "status": "ready_for_code",
        "expires_at": (task.created_at.load(Ordering::Relaxed) + TASK_TTL_MS) / 1000,
    });
    Ok(Json(json!({ "ok": true, "data": data })))
}

async fn consume_code(
    State(ctx): State<Arc<AdminContext>>,
    headers: axum::http::HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = owner_from_headers(&ctx, &headers);
    let task = find_task(&ctx, &task_id, &owner)?;
    let wx = wx_state_from_ctx(&ctx);
    let service = wx.service.clone();
    let app_id = task.app_id.clone();
    let session = task.session.lock().await;
    let code = service.issue_code(&*session, &app_id).await.map_err(ApiError::BadGateway)?;
    let openid = session.openid.clone().unwrap_or_default();
    drop(session);

    let removed = wx.tasks.lock().remove(&task_id);
    if let Some(t) = removed {
        let mut s = t.session.lock().await;
        wx.service.destroy(&mut *s);
    }

    Ok(Json(json!({
        "ok": true,
        "data": {
            "openid": openid,
            "app_id": app_id,
            "code": code,
            "err_msg": "login:ok",
        }
    })))
}

fn status_name(s: ScanStatus) -> &'static str {
    match s {
        ScanStatus::Waiting => "waiting",
        ScanStatus::Scanned => "scanned",
        ScanStatus::Authorized => "authorized",
        ScanStatus::Cancelled => "cancelled",
        ScanStatus::Expired => "expired",
    }
}
