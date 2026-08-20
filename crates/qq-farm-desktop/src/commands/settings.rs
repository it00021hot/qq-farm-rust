//! 设置面板。

use serde_json::{json, Value};
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::admin;
use qq_farm_app::qq_bot_bind;
use qq_farm_app::settings;
use qq_farm_core::services::qq_bot::{BindPollResult, BindStartResult};

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

/// 设置面板聚合。
#[tauri::command]
pub fn get_settings_panel(state: State<'_, DesktopState>, account_id: String) -> IpcResult<Value> {
    accounts::ensure_account_access(&state.acl, &account_id).map_err(IpcError::from)?;
    Ok(settings::settings_panel(&account_id, "local"))
}

/// 保存设置快照。
#[tauri::command]
pub fn save_settings(
    state: State<'_, DesktopState>,
    account_id: String,
    snapshot: Value,
) -> IpcResult<Value> {
    accounts::ensure_account_access(&state.acl, &account_id).map_err(IpcError::from)?;
    settings::save_settings(&state.app, &account_id, snapshot).map_err(IpcError::from)
}

const DESKTOP_USERNAME: &str = "local";

/// 读取下线提醒（桌面单用户）。
#[tauri::command]
pub fn get_offline_reminder() -> IpcResult<Value> {
    Ok(settings::offline_reminder_view(Some(DESKTOP_USERNAME)))
}

/// 保存下线提醒。
#[tauri::command]
pub fn set_offline_reminder(state: State<'_, DesktopState>, cfg: Value) -> IpcResult<Value> {
    settings::set_offline_reminder(Some(DESKTOP_USERNAME), cfg);
    state
        .app
        .engine
        .qq_bot()
        .reconcile_background(qq_farm_core::models::store::global_config::gateway_qq_bot_config());
    Ok(settings::offline_reminder_view(Some(DESKTOP_USERNAME)))
}

/// 测试下线提醒推送（不落盘）。
#[tauri::command]
pub async fn test_offline_reminder(state: State<'_, DesktopState>, cfg: Value) -> IpcResult<Value> {
    settings::test_offline_reminder(&state.app, Some(DESKTOP_USERNAME), cfg)
        .await
        .map_err(IpcError::from)
}

/// QQ Bot 绑定状态。
#[tauri::command]
pub fn get_qq_bot_bind_status() -> IpcResult<Value> {
    Ok(qq_bot_bind::qq_bot_bind_status(Some(DESKTOP_USERNAME)))
}

/// 启动 QQ Bot 扫码绑定。
#[tauri::command]
pub fn start_qq_bot_bind(state: State<'_, DesktopState>) -> IpcResult<BindStartResult> {
    qq_bot_bind::start_qq_bot_bind(&state.app, DESKTOP_USERNAME).map_err(IpcError::from)
}

/// 轮询 QQ Bot 绑定状态。
#[tauri::command]
pub fn poll_qq_bot_bind(state: State<'_, DesktopState>, session_id: String) -> IpcResult<BindPollResult> {
    Ok(qq_bot_bind::poll_qq_bot_bind(&state.app, &session_id))
}

/// 解绑 QQ Bot 通知。
#[tauri::command]
pub fn unbind_qq_bot(state: State<'_, DesktopState>) -> IpcResult<Value> {
    qq_bot_bind::unbind_qq_bot(DESKTOP_USERNAME);
    state
        .app
        .engine
        .qq_bot()
        .bind_sessions()
        .clear_user(DESKTOP_USERNAME);
    Ok(settings::offline_reminder_view(Some(DESKTOP_USERNAME)))
}

/// 设备预设列表。
#[tauri::command]
pub fn get_device_presets() -> IpcResult<Value> {
    Ok(admin::device_presets())
}

/// 系统配置（saved / default / current）。
#[tauri::command]
pub fn get_system_config() -> IpcResult<Value> {
    let saved = qq_farm_core::models::store::global_config::get_system_config();
    let default = qq_farm_core::config::get_default_system_config();
    let current = qq_farm_core::config::get_runtime_config();
    Ok(json!({
        "saved": saved,
        "default": default,
        "current": current,
    }))
}

/// 保存系统配置并立即生效。
#[tauri::command]
pub fn set_system_config(cfg: Value) -> IpcResult<Value> {
    let saved = admin::set_system_config(cfg).map_err(IpcError::from)?;
    let current = qq_farm_core::config::get_runtime_config();
    Ok(json!({ "saved": saved, "current": current }))
}

/// 重置系统配置为默认值。
#[tauri::command]
pub fn reset_system_config() -> IpcResult<Value> {
    Ok(admin::reset_system_config())
}
