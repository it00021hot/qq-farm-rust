//! 桌面就绪与概览快照。

use tauri::State;

use crate::error::IpcResult;
use crate::state::DesktopState;

use super::account::build_accounts;
use super::dto::DesktopSnapshot;

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

pub(crate) fn build_snapshot(state: &DesktopState) -> DesktopSnapshot {
    let accounts = build_accounts(state);
    let worker_count = state.app.engine.list_workers().len();
    DesktopSnapshot {
        ready: true,
        worker_count,
        account_count: accounts.len(),
        accounts,
    }
}
