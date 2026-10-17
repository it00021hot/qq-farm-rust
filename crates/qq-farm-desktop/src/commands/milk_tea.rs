//! 奶茶活动（GetGroup 2026110100 / Operate 53-76，1.15 新活动）。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::milk_tea;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 活动快照（page 决定子页面：shop/settings/upgrade/workshop/hireList/exchange/fund/work/rank/logs）。
#[tauri::command]
pub async fn milk_tea_snapshot(
    state: State<'_, DesktopState>,
    account_id: String,
    page: String,
    input: Option<Value>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let input = input.unwrap_or_else(|| serde_json::json!({}));
    milk_tea::snapshot(&state.app, &account_id, &page, &input).await.map_err(IpcError::from)
}

/// 写操作（action + input）。
#[tauri::command]
pub async fn milk_tea_operate(
    state: State<'_, DesktopState>,
    account_id: String,
    action: String,
    input: Option<Value>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let input = input.unwrap_or_else(|| serde_json::json!({}));
    milk_tea::operate(&state.app, &account_id, &action, &input).await.map_err(IpcError::from)
}
