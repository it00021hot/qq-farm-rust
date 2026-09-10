//! 好友模块门面。

use serde_json::{json, Value};

use crate::dto::friend_summaries_from_values;
use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 好友列表。
pub async fn list_friends(
    ctx: &AppContext,
    account_id: &str,
    force: bool,
) -> AppResult<Vec<qq_farm_core::services::friend::visit_strategy::FriendSummary>> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let friends = loop_.friend().get_friends_list(force).await.map_err(AppError::from_core)?;
    Ok(friend_summaries_from_values(friends))
}

/// 清空好友列表缓存。
pub fn clear_friends_cache(ctx: &AppContext, account_id: &str) -> AppResult<()> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::friend::scheduler::FriendService::clear_friends_list_cache(
        loop_.friend().as_ref(),
    );
    Ok(())
}

/// 互动记录。
pub async fn interact_records(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let records = qq_farm_core::services::interact::InteractService::new(loop_.gateway().clone())
        .get_interact_records()
        .await
        .map_err(AppError::from_core)?;
    serde_json::to_value(records).map_err(|e| AppError::Internal(e.to_string()))
}

/// 好友地块。
pub async fn friend_lands(ctx: &AppContext, account_id: &str, gid: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let lands = loop_.friend().get_friend_lands_detail(gid).await.map_err(AppError::from_core)?;
    serde_json::to_value(lands).map_err(|e| AppError::Internal(e.to_string()))
}

/// 好友操作（偷菜成功后自动卖果实）。
pub async fn friend_op(ctx: &AppContext, account_id: &str, gid: i64, op: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let op = qq_farm_core::models::types::FriendOperation::from_str_opt(op)
        .ok_or_else(|| AppError::BadRequest(format!("unknown op: {op}")))?;
    let ret = loop_.friend().do_friend_operation(op, gid).await.map_err(AppError::from_core)?;
    let stolen = ret.get("count").and_then(serde_json::Value::as_u64).unwrap_or(0);
    if matches!(op, qq_farm_core::models::types::FriendOperation::Steal) && stolen > 0 {
        let _ = loop_.warehouse().sell_all_fruits().await;
    }
    Ok(ret)
}

/// 游戏内删除好友（成功后加入本地黑名单，自动互动/自动通过申请都跳过）。
pub async fn delete_friend(ctx: &AppContext, account_id: &str, gid: i64) -> AppResult<()> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.friend().delete_friend(gid).await.map_err(AppError::from_core)
}

/// 好友黑名单。
#[must_use]
pub fn friend_blacklist(account_id: &str) -> Value {
    json!(qq_farm_core::models::store::account_config::get_friend_blacklist(Some(account_id)))
}

pub fn toggle_friend_blacklist(account_id: &str, gid: i64) -> Value {
    json!(qq_farm_core::models::store::account_config::toggle_friend_blacklist(account_id, gid))
}

/// 好友互动道具库存。
pub async fn friend_interaction_items(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::friend_interaction_items::get_friend_interaction_items(loop_.gateway())
        .await
        .map_err(AppError::from_core)
}

/// 对好友农场批量使用互动道具。
pub async fn friend_interaction_items_use(
    ctx: &AppContext,
    account_id: &str,
    friend_gid: i64,
    item_id: i64,
    land_ids: Vec<i64>,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::friend_interaction_items::use_friend_interaction_item_batch(
        loop_.gateway(),
        friend_gid,
        item_id,
        &land_ids,
    )
    .await
    .map_err(AppError::from_core)
}

/// 自用互动道具库存。
pub async fn farm_interaction_items(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::friend_interaction_items::get_self_interaction_items(loop_.gateway())
        .await
        .map_err(AppError::from_core)
}

/// 对自己农场批量使用互动道具。
pub async fn farm_interaction_items_use(
    ctx: &AppContext,
    account_id: &str,
    item_id: i64,
    land_ids: Vec<i64>,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let host_gid = loop_.current_gid();
    if host_gid <= 0 {
        return Err(AppError::Internal("当前账号未在线".into()));
    }
    qq_farm_core::services::friend_interaction_items::use_self_interaction_item_batch(
        loop_.gateway(),
        host_gid,
        item_id,
        &land_ids,
    )
    .await
    .map_err(AppError::from_core)
}

/// 图鉴快照（作物 + 变异）。
pub async fn illustrated_snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::illustrated::IllustratedService::new(loop_.gateway().clone())
        .get_snapshot()
        .await
        .map_err(AppError::from_core)
}
