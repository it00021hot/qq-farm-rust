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
    let stolen = ret.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
    if matches!(op, qq_farm_core::models::types::FriendOperation::Steal) && stolen > 0 {
        let _ = loop_.warehouse().sell_all_fruits().await;
    }
    Ok(ret)
}

/// 好友黑名单。
#[must_use]
pub fn friend_blacklist(account_id: &str) -> Value {
    json!(qq_farm_core::models::store::account_config::get_friend_blacklist(Some(account_id)))
}

pub fn toggle_friend_blacklist(account_id: &str, gid: i64) -> Value {
    json!(qq_farm_core::models::store::account_config::toggle_friend_blacklist(account_id, gid))
}

#[must_use]
pub fn known_gid_settings(account_id: &str) -> Value {
    json!({
        "knownFriendGids": qq_farm_core::models::store::account_config::get_known_friend_gids(Some(account_id)),
        "knownFriendGidSyncCooldownSec": qq_farm_core::models::store::account_config::get_known_friend_gid_sync_cooldown_sec(Some(account_id)),
        "friendsListCacheTtlSec": qq_farm_core::models::store::account_config::get_friends_list_cache_ttl_sec(Some(account_id)),
    })
}

pub fn set_known_gids(account_id: &str, gids: Vec<i64>) -> Value {
    qq_farm_core::models::store::account_config::set_known_friend_gids(account_id, gids);
    known_gid_settings(account_id)
}

pub fn add_known_gid(account_id: &str, gid: i64) -> Value {
    qq_farm_core::models::store::account_config::add_known_friend_gid(account_id, gid);
    known_gid_settings(account_id)
}

pub fn remove_known_gid(account_id: &str, gid: i64) -> Value {
    let _ = qq_farm_core::models::store::account_config::remove_known_friend_gid(account_id, gid);
    known_gid_settings(account_id)
}

pub fn batch_add_known_gids(account_id: &str, gids: &[i64]) -> Value {
    let _ = qq_farm_core::models::store::account_config::add_known_friend_gids(account_id, gids);
    known_gid_settings(account_id)
}

pub fn batch_remove_known_gids(account_id: &str, gids: &[i64]) -> Value {
    let _ = qq_farm_core::models::store::account_config::remove_known_friend_gids(account_id, gids);
    known_gid_settings(account_id)
}

pub fn set_known_gid_cooldowns(
    account_id: &str,
    sync_cooldown_sec: Option<i64>,
    cache_ttl_sec: Option<i64>,
) -> Value {
    if let Some(sec) = sync_cooldown_sec {
        qq_farm_core::models::store::account_config::set_known_friend_gid_sync_cooldown_sec(
            account_id, sec,
        );
    }
    if let Some(sec) = cache_ttl_sec {
        qq_farm_core::models::store::account_config::set_friends_list_cache_ttl_sec(
            account_id, sec,
        );
    }
    known_gid_settings(account_id)
}
