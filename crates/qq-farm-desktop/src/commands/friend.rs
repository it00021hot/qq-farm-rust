//! 好友列表、操作、黑名单、已知 GID。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
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

/// 强制同步好友列表。
#[tauri::command]
pub async fn friend_sync(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Vec<qq_farm_app::dto::FriendSummary>> {
    ensure(&state, &account_id)?;
    friend::list_friends(&state.app, &account_id, true).await.map_err(IpcError::from)
}

/// 清空好友列表缓存。
#[tauri::command]
pub fn friend_clear_cache(state: State<'_, DesktopState>, account_id: String) -> IpcResult<()> {
    ensure(&state, &account_id)?;
    friend::clear_friends_cache(&state.app, &account_id).map_err(IpcError::from)
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

/// 已知好友 GID 设置。
#[tauri::command]
pub fn friend_known_gids(state: State<'_, DesktopState>, account_id: String) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    Ok(friend::known_gid_settings(&account_id))
}

/// 覆盖设置已知好友 GID。
#[tauri::command]
pub fn friend_set_known_gids(
    state: State<'_, DesktopState>,
    account_id: String,
    gids: Vec<i64>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    Ok(friend::set_known_gids(&account_id, gids))
}
