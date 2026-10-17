//! 快乐不独享（分享奖励）。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::share_reward;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 实例快照（activity_id：2026110104 等）。
#[tauri::command]
pub async fn share_reward_snapshot(
    state: State<'_, DesktopState>,
    account_id: String,
    activity_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    share_reward::snapshot(&state.app, &account_id, &activity_id).await.map_err(IpcError::from)
}

/// 写操作（share/milestones/logs/daily）。
#[tauri::command]
pub async fn share_reward_operate(
    state: State<'_, DesktopState>,
    account_id: String,
    activity_id: String,
    action: String,
    input: Option<Value>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let input = input.unwrap_or_else(|| serde_json::json!({}));
    share_reward::operate(&state.app, &account_id, &activity_id, &action, &input)
        .await
        .map_err(IpcError::from)
}
