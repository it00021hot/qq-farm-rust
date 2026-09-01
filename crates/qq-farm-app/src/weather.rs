//! 天气活动「雨落成诗」门面。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 活动快照。
pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().get_current_activity().await.map_err(AppError::from_core)
}

/// 好友基础名单（不进农场）。
pub async fn friends(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let list = loop_.weather().get_weather_friends().await.map_err(AppError::from_core)?;
    serde_json::to_value(list).map_err(|e| AppError::Internal(e.to_string()))
}

/// 扫描好友现场天气。
pub async fn scan_friends(ctx: &AppContext, account_id: &str, gids: &[i64]) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().scan_weather_friends(gids).await.map_err(AppError::from_core)
}

/// 兑换采集瓶。
pub async fn exchange_collector(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().exchange_collector_bottle().await.map_err(AppError::from_core)
}

/// 采雨。
pub async fn collect(ctx: &AppContext, account_id: &str, friend_gid: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().collect_weather(friend_gid).await.map_err(AppError::from_core)
}

/// 召唤雷雨。
pub async fn summon(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().summon_thunderstorm().await.map_err(AppError::from_core)
}

/// 青蛙使坏。
pub async fn mischief_frog(ctx: &AppContext, account_id: &str, friend_gid: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().frog_mischief(friend_gid).await.map_err(AppError::from_core)
}

/// 乌云使坏（不指定地块时自动选第一块合格地块）。
pub async fn mischief_cloud(
    ctx: &AppContext,
    account_id: &str,
    friend_gid: i64,
    land_id: Option<i64>,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().cloud_mischief(friend_gid, land_id).await.map_err(AppError::from_core)
}

/// 推进气象研究节点。
pub async fn advance_research(ctx: &AppContext, account_id: &str, node_id: i64) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.weather().advance_research(node_id).await.map_err(AppError::from_core)
}
