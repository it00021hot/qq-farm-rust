//! 商城 / 神秘商人。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::commerce;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 商城概览 stub（提示使用 catalog / mystery）。
#[tauri::command]
pub fn commerce_overview(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    commerce::commerce_overview(&state.app, &account_id).map_err(IpcError::from)
}

/// 游戏商城目录。
#[tauri::command]
pub async fn commerce_mall_catalog(
    state: State<'_, DesktopState>,
    account_id: String,
    slot_type: Option<i32>,
    sub_slot_type: Option<i32>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    commerce::mall_catalog(&state.app, &account_id, slot_type, sub_slot_type)
        .await
        .map_err(IpcError::from)
}

/// 购买商城商品。
#[tauri::command]
pub async fn commerce_mall_purchase(
    state: State<'_, DesktopState>,
    account_id: String,
    goods_id: i32,
    count: Option<i32>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    commerce::purchase_mall(&state.app, &account_id, goods_id, count.unwrap_or(1))
        .await
        .map_err(IpcError::from)
}

/// 神秘商人。
#[tauri::command]
pub async fn commerce_mystery_shop(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    commerce::mystery_shop(&state.app, &account_id)
        .await
        .map_err(IpcError::from)
}

/// 购买神秘商人商品。
#[tauri::command]
pub async fn commerce_mystery_purchase(
    state: State<'_, DesktopState>,
    account_id: String,
    offer_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    commerce::purchase_mystery(&state.app, &account_id, &offer_id)
        .await
        .map_err(IpcError::from)
}
