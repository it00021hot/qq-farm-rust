//! 天气活动「雨落成诗」IPC 命令。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::error::AppError;
use qq_farm_app::weather;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 天气活动快照。
#[tauri::command]
pub async fn weather_snapshot(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    weather::snapshot(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 好友基础名单。
#[tauri::command]
pub async fn weather_friends(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    weather::friends(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 扫描好友现场天气（批上限 5）。
#[tauri::command]
pub async fn weather_friends_scan(
    state: State<'_, DesktopState>,
    account_id: String,
    friend_gids: Vec<String>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let gids: Vec<i64> = friend_gids
        .iter()
        .filter_map(|g| g.trim().parse::<i64>().ok())
        .collect();
    weather::scan_friends(&state.app, &account_id, &gids).await.map_err(IpcError::from)
}

/// 兑换采集瓶。
#[tauri::command]
pub async fn weather_exchange_collector(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    weather::exchange_collector(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 采雨。
#[tauri::command]
pub async fn weather_collect(
    state: State<'_, DesktopState>,
    account_id: String,
    friend_gid: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let gid: i64 = friend_gid.trim().parse().map_err(|_| {
        IpcError::from(AppError::BadRequest("无效的好友 GID".to_string()))
    })?;
    weather::collect(&state.app, &account_id, gid).await.map_err(IpcError::from)
}

/// 召唤雷雨。
#[tauri::command]
pub async fn weather_summon(state: State<'_, DesktopState>, account_id: String) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    weather::summon(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 青蛙使坏。
#[tauri::command]
pub async fn weather_mischief_frog(
    state: State<'_, DesktopState>,
    account_id: String,
    friend_gid: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let gid: i64 = friend_gid.trim().parse().map_err(|_| {
        IpcError::from(AppError::BadRequest("无效的好友 GID".to_string()))
    })?;
    weather::mischief_frog(&state.app, &account_id, gid).await.map_err(IpcError::from)
}

/// 乌云使坏（不传 landId 自动选第一块合格地块）。
#[tauri::command]
pub async fn weather_mischief_cloud(
    state: State<'_, DesktopState>,
    account_id: String,
    friend_gid: String,
    land_id: Option<String>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let gid: i64 = friend_gid.trim().parse().map_err(|_| {
        IpcError::from(AppError::BadRequest("无效的好友 GID".to_string()))
    })?;
    let land = match land_id.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(text) => Some(text.parse::<i64>().map_err(|_| {
            IpcError::from(AppError::BadRequest("无效的地块 ID".to_string()))
        })?),
    };
    weather::mischief_cloud(&state.app, &account_id, gid, land)
        .await
        .map_err(IpcError::from)
}

/// 推进气象研究节点。
#[tauri::command]
pub async fn weather_advance_research(
    state: State<'_, DesktopState>,
    account_id: String,
    node_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let node: i64 = node_id.trim().parse().map_err(|_| {
        IpcError::from(AppError::BadRequest("无效的气象研究节点".to_string()))
    })?;
    weather::advance_research(&state.app, &account_id, node)
        .await
        .map_err(IpcError::from)
}
