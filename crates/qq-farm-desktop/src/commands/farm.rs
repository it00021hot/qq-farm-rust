//! 农场操作、背包、日志。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::farm;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

use super::dto::BagSellItem;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 面板状态详情（含等级进度 + 扁平字段）。
#[tauri::command]
pub fn farm_status_detail(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<qq_farm_app::dto::PanelStatus> {
    ensure(&state, &account_id)?;
    Ok(farm::panel_status_with_progress(&state.app, &account_id))
}

/// 钻石余额。
#[tauri::command]
pub async fn farm_diamond(state: State<'_, DesktopState>, account_id: String) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let diamond = farm::diamond_balance(&state.app, &account_id).await.map_err(IpcError::from)?;
    Ok(serde_json::json!({ "diamond": diamond }))
}

/// 地块详情。
#[tauri::command]
pub async fn farm_lands(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<qq_farm_app::dto::LandsPayload> {
    ensure(&state, &account_id)?;
    farm::lands(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 农场手动操作。
#[tauri::command]
pub async fn farm_operate(
    state: State<'_, DesktopState>,
    account_id: String,
    op: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    farm::operate(&state.app, &account_id, &op).await.map_err(IpcError::from)
}

/// 背包详情。
#[tauri::command]
pub async fn farm_bag(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<qq_farm_core::services::warehouse::BagDetail> {
    ensure(&state, &account_id)?;
    farm::bag(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 出售背包物品。
#[tauri::command]
pub async fn farm_bag_sell(
    state: State<'_, DesktopState>,
    account_id: String,
    items: Vec<BagSellItem>,
) -> IpcResult<()> {
    ensure(&state, &account_id)?;
    let tuples: Vec<(i64, i64, i64)> =
        items.into_iter().map(|i| (i.item_id, i.count, i.uid)).collect();
    farm::bag_sell(&state.app, &account_id, &tuples).await.map_err(IpcError::from)
}

/// 使用背包物品。
#[tauri::command]
pub async fn farm_bag_use(
    state: State<'_, DesktopState>,
    account_id: String,
    item_id: i64,
    count: i64,
    uid: Option<i64>,
) -> IpcResult<()> {
    ensure(&state, &account_id)?;
    farm::bag_use(&state.app, &account_id, item_id, count, uid.unwrap_or(0))
        .await
        .map_err(IpcError::from)
}

/// 背包种子 / 种子目录。
#[tauri::command]
pub async fn farm_seeds(
    state: State<'_, DesktopState>,
    account_id: Option<String>,
) -> IpcResult<Value> {
    let id = account_id.unwrap_or_default();
    if id.is_empty() {
        let _ = &state.acl;
        return Ok(farm::seeds_catalog());
    }
    ensure(&state, &id)?;
    farm::bag_seeds(&state.app, &id).await.map_err(IpcError::from)
}

/// 每日礼包概览。
#[tauri::command]
pub async fn farm_daily_gifts(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    farm::daily_gift_overview(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 全局 / 账号日志。
#[tauri::command]
pub fn farm_get_logs(
    state: State<'_, DesktopState>,
    account_id: Option<String>,
    limit: Option<usize>,
) -> IpcResult<Vec<qq_farm_core::runtime::runtime_state::LogEntry>> {
    let id = account_id.unwrap_or_default();
    if !id.is_empty() {
        ensure(&state, &id)?;
    } else {
        let _ = &state.acl;
    }
    let lim = limit.unwrap_or(200);
    let opt = if id.is_empty() { None } else { Some(id.as_str()) };
    Ok(farm::engine_global_logs(&state.app, opt, lim))
}

/// 清空全局日志。
#[tauri::command]
pub fn farm_clear_logs(
    state: State<'_, DesktopState>,
    account_id: Option<String>,
) -> IpcResult<()> {
    let id = account_id.unwrap_or_default();
    if !id.is_empty() {
        ensure(&state, &id)?;
    } else {
        let _ = &state.acl;
    }
    let opt = if id.is_empty() { None } else { Some(id.as_str()) };
    farm::clear_global_logs(&state.app, opt);
    Ok(())
}

/// 种植分析排名。
#[tauri::command]
pub fn farm_analytics(state: State<'_, DesktopState>, sort_by: Option<String>) -> IpcResult<Value> {
    let _ = &state.acl;
    Ok(farm::analytics(sort_by.as_deref()))
}

/// 偷菜作物黑名单。
#[tauri::command]
pub fn farm_get_plant_blacklist(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    Ok(farm::plant_blacklist(&account_id))
}

/// 设置偷菜作物黑名单。
#[tauri::command]
pub fn farm_set_plant_blacklist(
    state: State<'_, DesktopState>,
    account_id: String,
    seed_ids: Vec<i64>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    Ok(farm::set_plant_blacklist(&account_id, seed_ids))
}

/// 保存设置后立即检测并购买化肥。
#[tauri::command]
pub async fn farm_fertilizer_check_and_buy(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    farm::fertilizer_check_and_buy(&state.app, &account_id).await.map_err(IpcError::from)
}
