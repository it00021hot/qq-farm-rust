//! 设置面板。

use serde_json::{json, Value};
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::admin;
use qq_farm_app::settings;
use qq_farm_core::models::store::global_config::OfflineReminder;

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
pub fn get_offline_reminder() -> IpcResult<OfflineReminder> {
    Ok(settings::get_offline_reminder(Some(DESKTOP_USERNAME)))
}

/// 保存下线提醒。
#[tauri::command]
pub fn set_offline_reminder(cfg: OfflineReminder) -> IpcResult<OfflineReminder> {
    let value = serde_json::to_value(&cfg).unwrap_or_default();
    settings::set_offline_reminder(Some(DESKTOP_USERNAME), value);
    Ok(settings::get_offline_reminder(Some(DESKTOP_USERNAME)))
}

/// 测试下线提醒推送（不落盘）。
#[tauri::command]
pub async fn test_offline_reminder(cfg: OfflineReminder) -> IpcResult<Value> {
    let value = serde_json::to_value(&cfg).unwrap_or_default();
    settings::test_offline_reminder(Some(DESKTOP_USERNAME), value).await.map_err(IpcError::from)
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
