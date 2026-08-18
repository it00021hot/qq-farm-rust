//! 微信扫码登录门面 — desktop / CLI 共用（与 server `/api/wx-login` 语义对齐）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use qq_farm_core::constants::game_ids::{
    DESKTOP_WECHAT_PORTS, WX_OAUTH_APP_ID, WX_OAUTH_REDIRECT_URI, WX_OAUTH_SCOPE, WX_OAUTH_STATE,
};
use qq_farm_core::constants::{WX_LOGIN_PENDING_AUTH_TTL_MS, WX_LOGIN_TASK_TTL_MS, WX_MINI_APP_ID};
use qq_farm_core::services::wx_login::service::{ScanStatus, WxLoginService, WxLoginSession};
use qq_farm_core::services::wx_login::YybCredentials;

use crate::error::{AppError, AppResult};

struct WxLoginTask {
    session: tokio::sync::Mutex<WxLoginSession>,
    qr_jpeg: Vec<u8>,
    created_at: AtomicI64,
    app_id: String,
    /// HTTP 面板任务归属；桌面端可为空。
    owner: String,
}

struct QuickLoginSession {
    created_at: i64,
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
    pub refresh_token: String,
    pub token_expires_at: i64,
}

impl From<YybCredentials> for WxAuth {
    fn from(c: YybCredentials) -> Self {
        Self {
            openid: c.openid,
            login_buffer: c.login_buffer,
            access_token: c.access_token,
            refresh_token: c.refresh_token,
            token_expires_at: c.expires_at,
        }
    }
}

/// 进程内微信扫码任务仓库。
pub struct WxLoginHub {
    service: Arc<WxLoginService>,
    tasks: Mutex<HashMap<String, Arc<WxLoginTask>>>,
    quick_sessions: Mutex<HashMap<String, QuickLoginSession>>,
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
            quick_sessions: Mutex::new(HashMap::new()),
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

/// 本机微信快速授权会话。
#[derive(Debug, Clone)]
pub struct WxQuickCreateResult {
    pub session_id: String,
    pub app_id: String,
    pub scope: String,
    pub redirect_uri: String,
    pub state: String,
    pub ports: Vec<u16>,
    pub expires_at: i64,
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

fn prune_quick_sessions(hub: &WxLoginHub, now: i64) {
    let ttl = WX_LOGIN_TASK_TTL_MS as i64;
    hub.quick_sessions.lock().retain(|_, v| now - v.created_at <= ttl);
}

fn take_quick_session(hub: &WxLoginHub, session_id: &str, owner: Option<&str>) -> AppResult<()> {
    prune_quick_sessions(hub, now_ms());
    let session = hub
        .quick_sessions
        .lock()
        .remove(session_id)
        .ok_or_else(|| AppError::NotFound("Quick login session expired or not found".into()))?;
    if owner.filter(|o| !o.is_empty()).is_some_and(|o| session.owner != o) {
        return Err(AppError::Forbidden("Quick login session owner mismatch".into()));
    }
    Ok(())
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

/// 创建本机微信快速授权会话（前端 WebView 调 localhost.weixin.qq.com）。
pub async fn create_quick_session(hub: &WxLoginHub) -> AppResult<WxQuickCreateResult> {
    create_quick_session_for(hub, "").await
}

pub async fn create_quick_session_for(
    hub: &WxLoginHub,
    owner: &str,
) -> AppResult<WxQuickCreateResult> {
    let now = now_ms();
    prune_quick_sessions(hub, now);
    let session_id = uuid::Uuid::new_v4().simple().to_string();
    hub.quick_sessions.lock().insert(
        session_id.clone(),
        QuickLoginSession { created_at: now, owner: owner.to_string() },
    );
    Ok(WxQuickCreateResult {
        session_id,
        app_id: WX_OAUTH_APP_ID.to_string(),
        scope: WX_OAUTH_SCOPE.to_string(),
        redirect_uri: WX_OAUTH_REDIRECT_URI.to_string(),
        state: WX_OAUTH_STATE.to_string(),
        ports: DESKTOP_WECHAT_PORTS.to_vec(),
        expires_at: (now + WX_LOGIN_TASK_TTL_MS as i64) / 1000,
    })
}

/// 确认本机微信 fast_login 回调并完成换票。
pub async fn confirm_quick_session(
    hub: &WxLoginHub,
    session_id: &str,
    redirect_url: &str,
) -> AppResult<WxCodeResult> {
    confirm_quick_session_for(hub, session_id, redirect_url, None).await
}

pub async fn confirm_quick_session_for(
    hub: &WxLoginHub,
    session_id: &str,
    redirect_url: &str,
    owner: Option<&str>,
) -> AppResult<WxCodeResult> {
    take_quick_session(hub, session_id, owner)?;
    let oauth_code = WxLoginService::parse_quick_redirect_url(redirect_url)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let creds = hub.service.exchange_oauth_code(&oauth_code).await.map_err(map_wx_auth_err)?;
    let openid = creds.openid.clone();
    let (gateway_code, updated) = hub
        .service
        .mint_gateway_code(&creds, WX_MINI_APP_ID)
        .await
        .map_err(map_wx_auth_err)?;
    store_pending_auth(hub, &gateway_code, WxAuth::from(updated));
    Ok(WxCodeResult { openid, app_id: WX_MINI_APP_ID.to_string(), code: gateway_code })
}

fn map_wx_auth_err(e: qq_farm_core::services::wx_login::WxAuthError) -> AppError {
    AppError::Internal(e.to_string())
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
    let openid = session.openid.clone().unwrap_or_default();
    let login_buffer = session.login_buffer.clone().unwrap_or_default();
    let access_token = session.access_token.clone().unwrap_or_default();
    let refresh_token = session.refresh_token.clone().unwrap_or_default();
    let token_expires_at = session.expires_at.unwrap_or(0);
    let app_id = task.app_id.clone();
    drop(session);

    let creds = YybCredentials {
        openid: openid.clone(),
        login_buffer: login_buffer.clone(),
        access_token,
        refresh_token,
        expires_at: token_expires_at,
        expires_in: 7200,
        ..Default::default()
    };
    let (code, updated) = hub
        .service
        .mint_gateway_code(&creds, &app_id)
        .await
        .map_err(map_wx_auth_err)?;
    if !updated.login_buffer.is_empty() {
        store_pending_auth(&hub, &code, WxAuth::from(updated));
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
        let auth = WxAuth {
            openid: "oid".into(),
            login_buffer: "buf".into(),
            access_token: "tok".into(),
            refresh_token: "rt".into(),
            token_expires_at: 1,
        };
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

    #[tokio::test]
    async fn quick_session_one_shot_confirm() {
        let hub = WxLoginHub::new();
        let created = create_quick_session(&hub).await.unwrap();
        take_quick_session(&hub, &created.session_id, None).unwrap();
        assert!(take_quick_session(&hub, &created.session_id, None).is_err());
    }
}
