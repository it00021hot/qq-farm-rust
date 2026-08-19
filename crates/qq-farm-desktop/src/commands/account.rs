//! 账号生命周期与微信扫码登录。

use base64::Engine as _;
use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts::{self, UpsertAccountRequest};
use qq_farm_app::wx_login;
use qq_farm_core::models::store::accounts as account_store;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

use super::dto::{
    AccountSummary, WxLoginCreateDto, WxLoginStatusDto, WxQuickAuthorizeDto, WxQuickCreateDto,
    WxQuickDetectDto,
};

/// 面板风格账号列表（含 `nextId` / running / nick）。
#[tauri::command]
pub fn list_accounts_page(state: State<'_, DesktopState>) -> IpcResult<Value> {
    let _ = &state.acl;
    Ok(accounts::list_accounts_enriched(&state.app, None))
}

/// 创建或更新账号。
#[tauri::command]
pub fn upsert_account(
    state: State<'_, DesktopState>,
    req: UpsertAccountRequest,
) -> IpcResult<Value> {
    accounts::upsert_account(&state.app, &state.acl, req).map_err(IpcError::from)
}

/// 删除账号。
#[tauri::command]
pub fn delete_account(state: State<'_, DesktopState>, account_id: String) -> IpcResult<()> {
    accounts::delete_account(&state.app, &state.acl, &account_id).map_err(IpcError::from)
}

/// 启动账号 worker。
#[tauri::command]
pub fn start_account(state: State<'_, DesktopState>, account_id: String) -> IpcResult<Value> {
    let acc =
        accounts::start_account(&state.app, &state.acl, &account_id).map_err(IpcError::from)?;
    Ok(accounts::account_to_public_json(&acc))
}

/// 停止账号 worker。
#[tauri::command]
pub fn stop_account(state: State<'_, DesktopState>, account_id: String) -> IpcResult<()> {
    accounts::stop_account(&state.app, &state.acl, &account_id).map_err(IpcError::from)
}

/// 创建微信扫码登录任务。
#[tauri::command]
pub async fn wx_login_create(state: State<'_, DesktopState>) -> IpcResult<WxLoginCreateDto> {
    let _ = &state.acl;
    let r = wx_login::create_task(&state.app.wx_login).await.map_err(IpcError::from)?;
    Ok(WxLoginCreateDto {
        task_id: r.task_id,
        app_id: r.app_id,
        status: r.status,
        expires_at: r.expires_at,
        qr_jpeg_base64: base64::engine::general_purpose::STANDARD.encode(&r.qr_jpeg),
    })
}

/// 轮询微信扫码状态。
#[tauri::command]
pub async fn wx_login_poll(
    state: State<'_, DesktopState>,
    task_id: String,
) -> IpcResult<WxLoginStatusDto> {
    let _ = &state.acl;
    let r = wx_login::poll_status(&state.app.wx_login, &task_id).await.map_err(IpcError::from)?;
    Ok(WxLoginStatusDto {
        task_id: r.task_id,
        app_id: r.app_id,
        status: r.status,
        expires_at: r.expires_at,
    })
}

/// 确认微信扫码授权。
#[tauri::command]
pub async fn wx_login_confirm(
    state: State<'_, DesktopState>,
    task_id: String,
) -> IpcResult<WxLoginStatusDto> {
    let _ = &state.acl;
    let r = wx_login::confirm(&state.app.wx_login, &task_id).await.map_err(IpcError::from)?;
    Ok(WxLoginStatusDto {
        task_id: r.task_id,
        app_id: r.app_id,
        status: r.status,
        expires_at: r.expires_at,
    })
}

/// 换取微信登录 code。
#[tauri::command]
pub async fn wx_login_code(state: State<'_, DesktopState>, task_id: String) -> IpcResult<Value> {
    let _ = &state.acl;
    let r = wx_login::issue_code(&state.app.wx_login, &task_id).await.map_err(IpcError::from)?;
    Ok(serde_json::json!({ "code": r.code, "openid": r.openid, "appId": r.app_id }))
}

/// 创建本机微信快速授权会话。
#[tauri::command]
pub async fn wx_quick_login_create(state: State<'_, DesktopState>) -> IpcResult<WxQuickCreateDto> {
    let _ = &state.acl;
    let r = wx_login::create_quick_session(&state.app.wx_login).await.map_err(IpcError::from)?;
    Ok(WxQuickCreateDto {
        session_id: r.session_id,
        app_id: r.app_id,
        scope: r.scope,
        redirect_uri: r.redirect_uri,
        state: r.state,
        ports: r.ports,
        expires_at: r.expires_at,
    })
}

/// 探测本机微信（原生代理，不走 WebView）。
#[tauri::command]
pub async fn wx_quick_login_detect(
    state: State<'_, DesktopState>,
    session_id: String,
) -> IpcResult<WxQuickDetectDto> {
    let _ = &state.acl;
    let r = wx_login::detect_quick_session(&state.app.wx_login, &session_id)
        .await
        .map_err(IpcError::from)?;
    Ok(WxQuickDetectDto {
        port: r.port,
        authorize_uuid: r.authorize_uuid,
        nickname: r.nickname,
        headimgurl: r.headimgurl,
    })
}

/// 本机微信确认授权，返回 redirect_url。
#[tauri::command]
pub async fn wx_quick_login_authorize(
    state: State<'_, DesktopState>,
    session_id: String,
    port: u16,
    authorize_uuid: String,
    x: i32,
    y: i32,
) -> IpcResult<WxQuickAuthorizeDto> {
    let _ = &state.acl;
    let redirect_url = wx_login::authorize_quick_session(
        &state.app.wx_login,
        &session_id,
        port,
        &authorize_uuid,
        qq_farm_core::services::wx_login::LocalWechatPosition { x, y },
    )
    .await
    .map_err(IpcError::from)?;
    Ok(WxQuickAuthorizeDto { redirect_url })
}

/// 确认本机微信 fast_login 回调。
#[tauri::command]
pub async fn wx_quick_login_confirm(
    state: State<'_, DesktopState>,
    session_id: String,
    redirect_url: String,
) -> IpcResult<Value> {
    let _ = &state.acl;
    let r = wx_login::confirm_quick_session(&state.app.wx_login, &session_id, &redirect_url)
        .await
        .map_err(IpcError::from)?;
    Ok(serde_json::json!({ "code": r.code, "openid": r.openid, "appId": r.app_id }))
}

pub(crate) fn build_accounts(state: &DesktopState) -> Vec<AccountSummary> {
    let running: std::collections::HashSet<String> =
        state.app.engine.list_workers().into_iter().map(|w| w.account_id).collect();
    let allowed = accounts::accessible_account_ids(&state.acl);
    account_store::get_accounts()
        .into_iter()
        .filter(|a| allowed.iter().any(|id| id == &a.id))
        .map(|a| {
            let nick = {
                let status = state.app.engine.panel_status(&a.id);
                status
                    .pointer("/status/name")
                    .and_then(|n| n.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&a.nick)
                    .to_string()
            };
            let wx_authorized = a.has_wx_auth();
            AccountSummary {
                id: a.id.clone(),
                name: a.name,
                nick,
                platform: a.platform,
                qq: a.qq,
                avatar: a.avatar,
                running: running.contains(&a.id),
                wx_authorized,
            }
        })
        .collect()
}
