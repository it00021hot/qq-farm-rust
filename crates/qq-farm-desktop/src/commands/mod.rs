//! Tauri 命令：薄适配 → `qq-farm-app`，无领域逻辑。

mod dto;

use tauri::State;

use qq_farm_app::accounts;
use qq_farm_core::models::store::account_config as cfg;
use qq_farm_core::models::store::accounts as account_store;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

pub use dto::{AccountSummary, DesktopSnapshot, SettingsSummary};

/// 桌面端就绪探测。
#[tauri::command]
pub fn desktop_ready(state: State<'_, DesktopState>) -> IpcResult<bool> {
    let _ = state.app.as_ref();
    Ok(true)
}

/// 概览快照：账号列表 + worker 数量。
#[tauri::command]
pub fn get_snapshot(state: State<'_, DesktopState>) -> IpcResult<DesktopSnapshot> {
    Ok(build_snapshot(&state))
}

/// 账号列表（LocalOwner：全部本地账号）。
#[tauri::command]
pub fn list_accounts(state: State<'_, DesktopState>) -> IpcResult<Vec<AccountSummary>> {
    Ok(build_accounts(&state))
}

/// 设置只读摘要。
#[tauri::command]
pub fn get_settings(
    state: State<'_, DesktopState>,
    account_id: Option<String>,
) -> IpcResult<SettingsSummary> {
    let _ = &state.acl;
    let id = account_id.unwrap_or_default();
    if !id.is_empty() {
        accounts::ensure_account_access(&state.acl, &id).map_err(IpcError::from)?;
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

fn build_snapshot(state: &DesktopState) -> DesktopSnapshot {
    let accounts = build_accounts(state);
    let worker_count = state.app.engine.list_workers().len();
    DesktopSnapshot {
        ready: true,
        worker_count,
        account_count: accounts.len(),
        accounts,
    }
}

fn build_accounts(state: &DesktopState) -> Vec<AccountSummary> {
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
