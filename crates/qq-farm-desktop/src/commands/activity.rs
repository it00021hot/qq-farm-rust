//! 活动中心快照与领取助手。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::activity;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 兼容 stub（提示使用 `activity_snapshot`）。
#[tauri::command]
pub fn activity_state(state: State<'_, DesktopState>, account_id: String) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::activity_state(&state.app, &account_id).map_err(IpcError::from)
}

/// 活动中心快照。
#[tauri::command]
pub async fn activity_snapshot(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::snapshot(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 领取战令奖励。
#[tauri::command]
pub async fn activity_claim_battle_pass(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::claim_battle_pass(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 点亮星座。
#[tauri::command]
pub async fn activity_light_constellation(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::light_constellation(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 星沙兑换。
#[tauri::command]
pub async fn activity_exchange_star_sand(
    state: State<'_, DesktopState>,
    account_id: String,
    goods_id: Value,
    count: Value,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::exchange_star_sand(&state.app, &account_id, &goods_id, &count)
        .await
        .map_err(IpcError::from)
}

/// 领取节气奖励。
#[tauri::command]
pub async fn activity_claim_solar_term(
    state: State<'_, DesktopState>,
    account_id: String,
    term_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::claim_solar_term(&state.app, &account_id, &term_id).await.map_err(IpcError::from)
}

/// 领取青梅每日种子。
#[tauri::command]
pub async fn activity_claim_qingmei_seed(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::claim_qingmei_seed(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 开始青梅酿造。
#[tauri::command]
pub async fn activity_qingmei_brew_start(
    state: State<'_, DesktopState>,
    account_id: String,
    input: Option<Value>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::start_qingmei_brew(&state.app, &account_id, input.unwrap_or(Value::Null))
        .await
        .map_err(IpcError::from)
}

/// 继续青梅酿造。
#[tauri::command]
pub async fn activity_qingmei_brew_continue(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::continue_qingmei_brew(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 结算青梅酿造。
#[tauri::command]
pub async fn activity_qingmei_brew_settle(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::settle_qingmei_brew(&state.app, &account_id).await.map_err(IpcError::from)
}
