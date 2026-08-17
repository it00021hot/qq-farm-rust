//! 账号生命周期与微信扫码登录。

use base64::Engine as _;
use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts::{self, UpsertAccountRequest};
use qq_farm_app::wx_login;
use qq_farm_core::models::store::accounts as account_store;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

use super::dto::{AccountSummary, WxLoginCreateDto, WxLoginStatusDto};

/// 账号列表（LocalOwner：全部本地账号）。
#[tauri::command]
pub fn list_accounts(state: State<'_, DesktopState>) -> IpcResult<Vec<AccountSummary>> {
    let _ = &state.acl;
    Ok(build_accounts(&state))
}

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
    let acc = accounts::start_account(&state.app, &state.acl, &account_id).map_err(IpcError::from)?;
    serde_json::to_value(acc).map_err(|e| IpcError::from(qq_farm_app::AppError::Internal(e.to_string())))
}

/// 停止账号 worker。
#[tauri::command]
pub fn stop_account(state: State<'_, DesktopState>, account_id: String) -> IpcResult<()> {
    accounts::stop_account(&state.app, &state.acl, &account_id).map_err(IpcError::from)
}

/// 更新账号备注。
#[tauri::command]
pub fn remark_account(
    state: State<'_, DesktopState>,
    account_id: String,
    name: String,
) -> IpcResult<Value> {
    let acc = accounts::remark_account(&state.acl, &account_id, name).map_err(IpcError::from)?;
    serde_json::to_value(acc).map_err(|e| IpcError::from(qq_farm_app::AppError::Internal(e.to_string())))
}

/// 创建微信扫码登录任务。
#[tauri::command]
pub async fn wx_login_create(state: State<'_, DesktopState>) -> IpcResult<WxLoginCreateDto> {
    let _ = &state.acl;
    let r = wx_login::create_task(&state.app.wx_login)
        .await
        .map_err(IpcError::from)?;
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
    let r = wx_login::poll_status(&state.app.wx_login, &task_id)
        .await
        .map_err(IpcError::from)?;
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
    let r = wx_login::confirm(&state.app.wx_login, &task_id)
        .await
        .map_err(IpcError::from)?;
    Ok(WxLoginStatusDto {
        task_id: r.task_id,
        app_id: r.app_id,
        status: r.status,
        expires_at: r.expires_at,
    })
}

/// 换取微信登录 code。
#[tauri::command]
pub async fn wx_login_code(
    state: State<'_, DesktopState>,
    task_id: String,
) -> IpcResult<Value> {
    let _ = &state.acl;
    let r = wx_login::issue_code(&state.app.wx_login, &task_id)
        .await
        .map_err(IpcError::from)?;
    Ok(serde_json::json!({ "code": r.code, "openid": r.openid, "appId": r.app_id }))
}

/// 销毁微信扫码任务。
#[tauri::command]
pub fn wx_login_destroy(state: State<'_, DesktopState>, task_id: String) -> IpcResult<()> {
    let _ = &state.acl;
    wx_login::destroy_task(&state.app.wx_login, &task_id);
    Ok(())
}

pub(crate) fn build_accounts(state: &DesktopState) -> Vec<AccountSummary> {
    let running: std::collections::HashSet<String> = state
        .app
        .engine
        .list_workers()
        .into_iter()
        .map(|w| w.account_id)
        .collect();
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
            AccountSummary {
                id: a.id.clone(),
                name: a.name,
                nick,
                platform: a.platform,
                qq: a.qq,
                avatar: a.avatar,
                running: running.contains(&a.id),
            }
        })
        .collect()
}
