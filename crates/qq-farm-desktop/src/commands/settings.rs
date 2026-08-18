//! 设置面板。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::settings;
use qq_farm_core::models::store::account_config as cfg;
use qq_farm_core::models::store::global_config::OfflineReminder;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

use super::dto::SettingsSummary;

/// 设置只读摘要。
#[tauri::command]
pub fn get_settings(
    state: State<'_, DesktopState>,
    account_id: Option<String>,
) -> IpcResult<SettingsSummary> {
    let id = account_id.unwrap_or_default();
    if !id.is_empty() {
        accounts::ensure_account_access(&state.acl, &id).map_err(IpcError::from)?;
    } else {
        let _ = &state.acl;
    }
    let opt = if id.is_empty() { None } else { Some(id.as_str()) };
    let intervals = cfg::get_intervals(opt);
    let automation = cfg::get_automation(opt);
    let strategy = cfg::get_planting_strategy(opt);
    let preferred_seed = cfg::get_preferred_seed(opt);
    Ok(SettingsSummary {
        account_id: id,
        strategy,
        preferred_seed,
        farm_interval_sec: intervals.farm,
        farm_min_sec: intervals.farm_min,
        farm_max_sec: intervals.farm_max,
        automation_farm: automation.farm,
        automation_friend: automation.friend,
        automation_task: automation.task,
        automation_sell: automation.sell,
    })
}

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
