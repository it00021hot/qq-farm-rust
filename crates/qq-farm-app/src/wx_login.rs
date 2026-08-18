//! 微信扫码登录门面 — desktop / CLI 共用（与 server `/api/wx-login` 语义对齐）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use qq_farm_core::constants::{WX_LOGIN_PENDING_AUTH_TTL_MS, WX_LOGIN_TASK_TTL_MS, WX_MINI_APP_ID};
use qq_farm_core::services::wx_login::service::{ScanStatus, WxLoginService, WxLoginSession};

use crate::error::{AppError, AppResult};

struct WxLoginTask {
    session: tokio::sync::Mutex<WxLoginSession>,
    qr_jpeg: Vec<u8>,
    created_at: AtomicI64,
    app_id: String,
    /// HTTP 面板任务归属；桌面端可为空。
    owner: String,
}

#[derive(Debug, Clone)]
struct PendingWxAuth {
    auth: WxAuth,
    created_at: i64,
}

/// 应用宝授权（可多次换取一次性网关 code）。不经过 HTTP/IPC 返回给前端。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WxAuth {
    pub openid: String,
    pub login_buffer: String,
    pub access_token: String,
}

/// 进程内微信扫码任务仓库。
pub struct WxLoginHub {
    service: Arc<WxLoginService>,
    tasks: Mutex<HashMap<String, Arc<WxLoginTask>>>,
    pending_auth: Mutex<HashMap<String, PendingWxAuth>>,
}

impl Default for WxLoginHub {
    fn default() -> Self {
        Self::new()
    }
}

impl WxLoginHub {
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: Arc::new(WxLoginService::new()),
            tasks: Mutex::new(HashMap::new()),
            pending_auth: Mutex::new(HashMap::new()),
        }
    }
}

/// 创建任务结果。
#[derive(Debug, Clone)]
pub struct WxCreateResult {
    pub task_id: String,
    pub app_id: String,
    pub status: String,
    pub expires_at: i64,
    pub qr_jpeg: Vec<u8>,
}

/// 状态轮询结果。
#[derive(Debug, Clone)]
pub struct WxStatusResult {
    pub task_id: String,
    pub app_id: String,
    pub status: String,
    pub expires_at: i64,
}

/// 换 code 结果。
#[derive(Debug, Clone)]
pub struct WxCodeResult {
    pub openid: String,
    pub app_id: String,
    pub code: String,
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
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

fn find_task(hub: &WxLoginHub, task_id: &str, owner: Option<&str>) -> AppResult<Arc<WxLoginTask>> {
    let task = hub
        .tasks
        .lock()
        .get(task_id)
        .cloned()
        .ok_or_else(|| AppError::NotFound("Login task not found or expired".into()))?;
    let expired = now_ms() - task.created_at.load(Ordering::Relaxed) > WX_LOGIN_TASK_TTL_MS as i64;
    let owner_mismatch = owner.filter(|o| !o.is_empty()).is_some_and(|o| task.owner != o);
    if expired || owner_mismatch {
        let _ = hub.tasks.lock().remove(task_id);
        return Err(AppError::NotFound("Login task not found or expired".into()));
    }
    Ok(task)
}

/// 创建扫码任务并返回 JPEG 二维码。
pub async fn create_task(hub: &WxLoginHub) -> AppResult<WxCreateResult> {
    create_task_for(hub, "").await
}

/// 创建扫码任务（HTTP 面板带 owner，防越权）。
pub async fn create_task_for(hub: &WxLoginHub, owner: &str) -> AppResult<WxCreateResult> {
    let (session, qr_jpeg) = hub.service.create_qr_session().await.map_err(AppError::Internal)?;
    let task_id = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let created_at = now_ms();
    let task = Arc::new(WxLoginTask {
        session: tokio::sync::Mutex::new(session),
        qr_jpeg: qr_jpeg.clone(),
        created_at: AtomicI64::new(created_at),
        app_id: WX_MINI_APP_ID.to_string(),
        owner: owner.to_string(),
    });
    hub.tasks.lock().insert(task_id.clone(), task);
    Ok(WxCreateResult {
        task_id,
        app_id: WX_MINI_APP_ID.to_string(),
        status: "waiting".into(),
        expires_at: (created_at + WX_LOGIN_TASK_TTL_MS as i64) / 1000,
        qr_jpeg,
    })
}

/// 读取任务二维码 JPEG。
pub fn qr_jpeg(hub: &WxLoginHub, task_id: &str, owner: Option<&str>) -> AppResult<Vec<u8>> {
    Ok(find_task(hub, task_id, owner)?.qr_jpeg.clone())
}

/// 轮询扫码状态。
pub async fn poll_status(hub: &WxLoginHub, task_id: &str) -> AppResult<WxStatusResult> {
    poll_status_for(hub, task_id, None).await
}

pub async fn poll_status_for(
    hub: &WxLoginHub,
    task_id: &str,
    owner: Option<&str>,
) -> AppResult<WxStatusResult> {
    let task = find_task(hub, task_id, owner)?;
    let mut session = task.session.lock().await;
    let status = hub.service.poll(&mut *session).await.map_err(AppError::Internal)?;
    let result = WxStatusResult {
        task_id: task_id.to_string(),
        app_id: task.app_id.clone(),
        status: status_name(status).to_string(),
        expires_at: (task.created_at.load(Ordering::Relaxed) + WX_LOGIN_TASK_TTL_MS as i64) / 1000,
    };
    if matches!(status, ScanStatus::Cancelled | ScanStatus::Expired) {
        drop(session);
        destroy_task(hub, task_id);
    }
    Ok(result)
}

/// 确认授权（建立 login_buffer）。
pub async fn confirm(hub: &WxLoginHub, task_id: &str) -> AppResult<WxStatusResult> {
    confirm_for(hub, task_id, None).await
}

pub async fn confirm_for(
    hub: &WxLoginHub,
    task_id: &str,
    owner: Option<&str>,
) -> AppResult<WxStatusResult> {
    let task = find_task(hub, task_id, owner)?;
    let mut session = task.session.lock().await;
    hub.service.confirm(&mut *session).await.map_err(AppError::Internal)?;
    drop(session);
    task.created_at.store(now_ms(), Ordering::Relaxed);
    Ok(WxStatusResult {
        task_id: task_id.to_string(),
        app_id: task.app_id.clone(),
        status: "ready_for_code".into(),
        expires_at: (task.created_at.load(Ordering::Relaxed) + WX_LOGIN_TASK_TTL_MS as i64) / 1000,
    })
}

/// 换取 wx.login code（会销毁任务）。
pub async fn issue_code(hub: &WxLoginHub, task_id: &str) -> AppResult<WxCodeResult> {
    issue_code_for(hub, task_id, None).await
}

pub async fn issue_code_for(
    hub: &WxLoginHub,
    task_id: &str,
    owner: Option<&str>,
) -> AppResult<WxCodeResult> {
    let task = find_task(hub, task_id, owner)?;
    let session = task.session.lock().await;
    let code = hub.service.issue_code(&*session, &task.app_id).await.map_err(AppError::Internal)?;
    let openid = session.openid.clone().unwrap_or_default();
    let login_buffer = session.login_buffer.clone().unwrap_or_default();
    let access_token = session.access_token.clone().unwrap_or_default();
    let app_id = task.app_id.clone();
    drop(session);
    if !login_buffer.is_empty() {
        store_pending_auth(
            hub,
            &code,
            WxAuth { openid: openid.clone(), login_buffer, access_token },
        );
    }
    destroy_task(hub, task_id);
    Ok(WxCodeResult { openid, app_id, code })
}

fn prune_pending_auth(hub: &WxLoginHub, now: i64) {
    let ttl = WX_LOGIN_PENDING_AUTH_TTL_MS as i64;
    hub.pending_auth.lock().retain(|_, v| now - v.created_at <= ttl);
}

fn store_pending_auth(hub: &WxLoginHub, code: &str, auth: WxAuth) {
    let code = code.trim();
    if code.is_empty() || auth.login_buffer.trim().is_empty() {
        return;
    }
    let now = now_ms();
    prune_pending_auth(hub, now);
    hub.pending_auth.lock().insert(code.to_string(), PendingWxAuth { auth, created_at: now });
}

/// 取出扫码换码后暂存的应用宝授权（一次性）。
pub fn take_pending_auth(hub: &WxLoginHub, code: &str) -> Option<WxAuth> {
    let code = code.trim();
    if code.is_empty() {
        return None;
    }
    let now = now_ms();
    prune_pending_auth(hub, now);
    hub.pending_auth.lock().remove(code).map(|p| p.auth)
}

/// 取消 / 清理任务。
pub fn destroy_task(hub: &WxLoginHub, task_id: &str) {
    destroy_task_for(hub, task_id, None);
}

pub fn destroy_task_for(hub: &WxLoginHub, task_id: &str, owner: Option<&str>) {
    let task = hub.tasks.lock().get(task_id).cloned();
    let Some(t) = task else {
        return;
    };
    if owner.filter(|o| !o.is_empty()).is_some_and(|o| t.owner != o) {
        return;
    }
    let removed = hub.tasks.lock().remove(task_id);
    if let Some(t) = removed {
        if let Ok(mut session) = t.session.try_lock() {
            hub.service.destroy(&mut *session);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_auth_claim_is_one_shot() {
        let hub = WxLoginHub::new();
        let auth =
            WxAuth { openid: "oid".into(), login_buffer: "buf".into(), access_token: "tok".into() };
        store_pending_auth(&hub, "wx-code", auth.clone());
        assert_eq!(take_pending_auth(&hub, "wx-code"), Some(auth));
        assert!(take_pending_auth(&hub, "wx-code").is_none());
    }

    #[test]
    fn pending_auth_ignores_empty_code_or_buffer() {
        let hub = WxLoginHub::new();
        store_pending_auth(&hub, "", WxAuth { login_buffer: "buf".into(), ..Default::default() });
        store_pending_auth(
            &hub,
            "code",
            WxAuth { login_buffer: String::new(), ..Default::default() },
        );
        assert!(take_pending_auth(&hub, "code").is_none());
        assert!(take_pending_auth(&hub, "").is_none());
        assert!(take_pending_auth(&hub, "   ").is_none());
    }

    #[test]
    fn pending_auth_expired_is_dropped() {
        let hub = WxLoginHub::new();
        store_pending_auth(
            &hub,
            "old",
            WxAuth { login_buffer: "buf".into(), ..Default::default() },
        );
        {
            let mut map = hub.pending_auth.lock();
            if let Some(p) = map.get_mut("old") {
                p.created_at = now_ms() - WX_LOGIN_PENDING_AUTH_TTL_MS as i64 - 1;
            }
        }
        assert!(take_pending_auth(&hub, "old").is_none());
    }
}
