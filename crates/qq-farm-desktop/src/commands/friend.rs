//! 好友列表、操作、黑名单。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::error::AppError;
use qq_farm_app::friend;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 好友列表。
#[tauri::command]
pub async fn friend_list(
    state: State<'_, DesktopState>,
    account_id: String,
    force: Option<bool>,
) -> IpcResult<Vec<qq_farm_app::dto::FriendSummary>> {
    ensure(&state, &account_id)?;
    friend::list_friends(&state.app, &account_id, force.unwrap_or(false))
        .await
        .map_err(IpcError::from)
}

/// 好友地块。
#[tauri::command]
pub async fn friend_lands(
    state: State<'_, DesktopState>,
    account_id: String,
    gid: i64,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    friend::friend_lands(&state.app, &account_id, gid).await.map_err(IpcError::from)
}

/// 好友操作。
#[tauri::command]
pub async fn friend_op(
    state: State<'_, DesktopState>,
    account_id: String,
    gid: i64,
    op: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    friend::friend_op(&state.app, &account_id, gid, &op).await.map_err(IpcError::from)
}

/// 互动记录。
#[tauri::command]
pub async fn friend_interact_records(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    friend::interact_records(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 切换好友黑名单。
#[tauri::command]
pub fn friend_blacklist_toggle(
    state: State<'_, DesktopState>,
    account_id: String,
    gid: i64,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    Ok(friend::toggle_friend_blacklist(&account_id, gid))
}

/// 好友互动道具库存。
#[tauri::command]
pub async fn friend_interaction_items(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    friend::friend_interaction_items(&state.app, &account_id)
        .await
        .map_err(IpcError::from)
}

/// 对好友农场批量使用互动道具。
#[tauri::command]
pub async fn friend_interaction_items_use(
    state: State<'_, DesktopState>,
    account_id: String,
    friend_gid: i64,
    item_id: i64,
    land_ids: Vec<i64>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    friend::friend_interaction_items_use(&state.app, &account_id, friend_gid, item_id, land_ids)
        .await
        .map_err(IpcError::from)
}

/// 自用互动道具库存。
#[tauri::command]
pub async fn farm_interaction_items(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    friend::farm_interaction_items(&state.app, &account_id)
        .await
        .map_err(IpcError::from)
}

/// 对自己农场批量使用互动道具。
#[tauri::command]
pub async fn farm_interaction_items_use(
    state: State<'_, DesktopState>,
    account_id: String,
    item_id: i64,
    land_ids: Vec<i64>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    friend::farm_interaction_items_use(&state.app, &account_id, item_id, land_ids)
        .await
        .map_err(IpcError::from)
}

/// 图鉴快照。
#[tauri::command]
pub async fn illustrated_snapshot(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    friend::illustrated_snapshot(&state.app, &account_id)
        .await
        .map_err(IpcError::from)
}

/// 游戏内删除好友（成功后加入本地黑名单）。
#[tauri::command]
pub async fn friend_delete(
    state: State<'_, DesktopState>,
    account_id: String,
    gid: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let gid_num: i64 = gid.trim().parse().map_err(|_| {
        IpcError::from(AppError::BadRequest("无效的好友 GID".to_string()))
    })?;
    qq_farm_app::friend::delete_friend(&state.app, &account_id, gid_num)
        .await
        .map_err(IpcError::from)?;
    Ok(serde_json::json!({ "ok": true, "gid": gid }))
}
