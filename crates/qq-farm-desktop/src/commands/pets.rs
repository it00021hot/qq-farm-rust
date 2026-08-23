//! 宠物 / 同气连枝礼包。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::pets;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 宠物快照。
#[tauri::command]
pub async fn pet_info(state: State<'_, DesktopState>, account_id: String) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    pets::pet_info(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 上场宠物。
#[tauri::command]
pub async fn pet_deploy(
    state: State<'_, DesktopState>,
    account_id: String,
    dog_id: i64,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    pets::pet_deploy(&state.app, &account_id, dog_id).await.map_err(IpcError::from)
}

/// 收回宠物。
#[tauri::command]
pub async fn pet_withdraw(state: State<'_, DesktopState>, account_id: String) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    pets::pet_withdraw(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 使用狗粮。
#[tauri::command]
pub async fn pet_food_use(
    state: State<'_, DesktopState>,
    account_id: String,
    item_id: i64,
    count: Option<i64>,
    uid: Option<i64>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    pets::pet_food_use(&state.app, &account_id, item_id, count.unwrap_or(1), uid.unwrap_or(0))
        .await
        .map_err(IpcError::from)
}

/// 守护记录。
#[tauri::command]
pub async fn pet_protect_logs(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    pets::pet_protect_logs(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 同气连枝礼包状态。
#[tauri::command]
pub async fn dog_skill_gifts_status(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    pets::dog_skill_gifts_status(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 手动领取同气连枝礼包。
#[tauri::command]
pub async fn dog_skill_gifts_claim(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    pets::dog_skill_gifts_claim(&state.app, &account_id).await.map_err(IpcError::from)
}
