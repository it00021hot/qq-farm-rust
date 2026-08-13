//! 微信扫码登录路由 — 6 端点（1:1 对应原 `controllers/admin/wx-login-routes.ts` 55 行）。
//!
//! 真实接通 `services::wx_login::WxLoginService`：
//! 1. `POST /api/wx-login/tasks` → create_qr_session（生成 QR）
//! 2. `GET  /api/wx-login/tasks/:id/qr` → 返回 QR 二进制
//! 3. `GET  /api/wx-login/tasks/:id/status` → poll 状态
//! 4. `POST /api/wx-login/tasks/:id/confirm` → confirm 扫码
//! 5. `POST /api/wx-login/tasks/:id/code` → issue_code 拿到 game auth code
//! 6. `DELETE /api/wx-login/tasks/:id` → destroy

use std::collections::HashMap;
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
use tokio::sync::oneshot;

use crate::context::{ok, ok_empty, AdminContext, ApiError, ApiResult};
use qq_farm_core::services::wx_login::service::{ScanStatus, WxLoginService, WxLoginSession};

/// 微信登录任务
struct WxLoginTask {
    /// tokio::Mutex 让 lock().await 可跨 await 持有
    session: tokio::sync::Mutex<WxLoginSession>,
    qr_png: Vec<u8>,
    last_status: tokio::sync::Mutex<ScanStatus>,
    /// auth code（confirm + issue 后填充）
    auth_code: tokio::sync::Mutex<Option<String>>,
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

    fn create(&self, app_id: String) -> Result<String, ApiError> {
        // async block → 用 oneshot 转
        let (tx, rx) = oneshot::channel();
        let service = self.service.clone();
        let tasks = self.tasks.clone();
        let app_id_for_task = app_id.clone();
        tokio::spawn(async move {
            let result = service.create_qr_session().await;
            let _ = tx.send(result);
        });
        // 实际不能这样 — 创建是 async。改成在 handler 内部 await。
        let _ = (rx, tasks, app_id_for_task);
        Err(ApiError::Internal("should be called from create_task handler".to_string()))
    }
}

impl Default for WxLoginState {
    fn default() -> Self {
        Self::new()
    }
}

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

const DEFAULT_APP_ID: &str = "1112386029";

fn wx_state_from_ctx(ctx: &AdminContext) -> Result<WxLoginState, ApiError> {
    // wx-login state 现在是 lazy：每个 request 共享 AdminContext.wx
    // AdminContext 加 wx 字段
    Ok(ctx.wx.clone())
}

async fn create_task(
    State(ctx): State<Arc<AdminContext>>,
    Json(body): Json<CreateTaskBody>,
) -> ApiResult<serde_json::Value> {
    let app_id = body.app_id.unwrap_or_else(|| DEFAULT_APP_ID.to_string());
    let wx = wx_state_from_ctx(&ctx)?;
    let service = wx.service.clone();
    let tasks = wx.tasks.clone();
    let app_id_for_task = app_id.clone();

    let (session, qr_png) = service
        .create_qr_session()
        .await
        .map_err(ApiError::Internal)?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let task = Arc::new(WxLoginTask {
        session: tokio::sync::Mutex::new(session),
        qr_png,
        last_status: tokio::sync::Mutex::new(ScanStatus::Waiting),
        auth_code: tokio::sync::Mutex::new(None),
        app_id: app_id_for_task,
    });
    tasks.lock().insert(task_id.clone(), task);

    Ok(Json(json!({
        "ok": true,
        "task_id": task_id,
        "app_id": app_id,
        "status": "waiting",
    })))
}

async fn get_qr(
    State(ctx): State<Arc<AdminContext>>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let wx = wx_state_from_ctx(&ctx)?;
    let task = wx
        .tasks
        .lock()
        .get(&task_id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("task not found: {task_id}")))?;
    Ok((
        [(header::CONTENT_TYPE, "image/png")],
        task.qr_png.clone(),
    ))
}

async fn delete_task(
    State(ctx): State<Arc<AdminContext>>,
    Path(task_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let wx = wx_state_from_ctx(&ctx)?;
    let task = wx.tasks.lock().remove(&task_id);
    if let Some(task) = task {
        let mut session = task.session.lock().await;
        wx.service.destroy(&mut *session);
    }
    ok_empty()
}

#[allow(dead_code)]
async fn get_status(
    State(ctx): State<Arc<AdminContext>>,
    Path(task_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let wx = wx_state_from_ctx(&ctx)?;
    let task = wx
        .tasks
        .lock()
        .get(&task_id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("task not found: {task_id}")))?;
    let service = wx.service.clone();
    // poll 内部需要 &mut session，session 在另一个线程持有不安全。
    // 用 tokio::sync::Mutex 替换 parkink_lot::Mutex 给 session
    let mut session = task.session.lock().await;
    let status = {
        let s: &mut WxLoginSession = &mut *session;
        service.poll(s).await.map_err(ApiError::Internal)?
    };
    *task.last_status.lock().await = status;
    Ok(Json(json!({
        "ok": true,
        "status": status_name(status),
    })))
}

async fn confirm_task(
    State(ctx): State<Arc<AdminContext>>,
    Path(task_id): Path<String>,
    Json(_body): Json<ConfirmBody>,
) -> ApiResult<serde_json::Value> {
    let wx = wx_state_from_ctx(&ctx)?;
    let task = wx
        .tasks
        .lock()
        .get(&task_id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("task not found: {task_id}")))?;
    let service = wx.service.clone();
    let mut session = task.session.lock().await;
    let (openid, _) = {
        let s: &mut WxLoginSession = &mut *session;
        service.confirm(s).await.map_err(ApiError::Internal)?
    };
    Ok(Json(json!({
        "ok": true,
        "openid": openid,
        "status": "authorized",
    })))
}

async fn consume_code(
    State(ctx): State<Arc<AdminContext>>,
    Path(task_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let wx = wx_state_from_ctx(&ctx)?;
    let task = wx
        .tasks
        .lock()
        .get(&task_id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("task not found: {task_id}")))?;
    let service = wx.service.clone();
    let app_id = task.app_id.clone();
    let session = task.session.lock().await;
    let code = service
        .issue_code(&*session, &app_id)
        .await
        .map_err(ApiError::Internal)?;
    *task.auth_code.lock().await = Some(code.clone());

    // === 全链路打通：auth_code → 创建 store Account → 启动 worker ===
    let account_id = persist_and_start(&ctx, &code, &app_id, &task_id);

    Ok(Json(json!({
        "ok": true,
        "code": code,
        "account_id": account_id,
    })))
}

/// 把 auth_code 落到 store Account + 启动 worker
fn persist_and_start(
    ctx: &AdminContext,
    auth_code: &str,
    app_id: &str,
    task_id: &str,
) -> String {
    use qq_farm_core::models::Account;
    use qq_farm_core::models::store::accounts as accounts_store;

    // 1. 构造 store Account（code 字段就是 auth_code）
    let display_name = format!(
        "wx-{}",
        &app_id.chars().take(8).collect::<String>()
    );
    let mut store_acc = accounts_store::Account {
        id: String::new(), // 让 add_or_update_account 自动生成
        name: display_name.clone(),
        code: auth_code.to_string(),
        platform: "wx".to_string(),
        qq: String::new(),
        uin: String::new(),
        avatar: String::new(),
        username: String::new(),
        created_at: 0,
        updated_at: 0,
    };
    let saved = accounts_store::add_or_update_account(store_acc.clone());
    let account_id = saved.id.clone();
    store_acc.id = account_id.clone();

    // 2. 转 models::Account + start_worker
    let models_acc = Account::new(
        account_id.clone(),
        auth_code.to_string(), // open_id 字段实际存 auth_code
        display_name.clone(),
    );
    let eng = ctx.engine.clone();
    match eng.start_worker(models_acc) {
        Ok(()) => {
            tracing::info!(
                account_id = %account_id,
                task_id = %task_id,
                "微信扫码全链路打通：worker 已启动"
            );
            // 写日志
            ctx.engine
                .runtime_state()
                .add_account_log("wx-login", "扫码登录成功，worker 已启动", Some(&account_id), Some(&display_name), None);
        }
        Err(e) => {
            tracing::warn!(account_id = %account_id, "启动 worker 失败: {e}");
            ctx.engine.runtime_state().add_account_log(
                "wx-login",
                &format!("worker 启动失败: {e}"),
                Some(&account_id),
                Some(&display_name),
                None,
            );
        }
    }
    account_id
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
