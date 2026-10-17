//! 节令日历（GetSolarTermsV2 / 答题 / 季度奖励，bot 0533264）。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::solar_calendar;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 节令日历快照。
#[tauri::command]
pub async fn solar_calendar_snapshot(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    solar_calendar::snapshot(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 节令操作：claim / answer / season。
#[tauri::command]
pub async fn solar_calendar_operate(
    state: State<'_, DesktopState>,
    account_id: String,
    action: String,
    input: Option<Value>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let input = input.unwrap_or_else(|| serde_json::json!({}));
    solar_calendar::operate(&state.app, &account_id, &action, &input).await.map_err(IpcError::from)
}
