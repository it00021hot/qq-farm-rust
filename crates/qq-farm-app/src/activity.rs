//! 活动中心门面。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

fn json_to_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// 活动中心快照。
pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().get_activity_center_snapshot().await.map_err(AppError::from_core)
}

pub async fn season(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let dto =
        loop_.activity_center().get_current_season_event().await.map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn shop(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let dto = loop_
        .activity_center()
        .get_current_star_sand_shop(Some(loop_.warehouse().as_ref()))
        .await
        .map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn solar_terms(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let dto =
        loop_.activity_center().get_current_solar_terms().await.map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn qingmei(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let dto = loop_
        .activity_center()
        .get_current_qingmei_activity()
        .await
        .map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn claim_battle_pass(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().claim_battle_pass_rewards().await.map_err(AppError::from_core)
}

pub async fn light_constellation(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().light_constellation().await.map_err(AppError::from_core)
}

pub async fn exchange_star_sand(
    ctx: &AppContext,
    account_id: &str,
    goods_id: &Value,
    count: &Value,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let dto = loop_
        .activity_center()
        .exchange_star_sand_goods(
            loop_.warehouse().as_ref(),
            &json_to_text(goods_id),
            &json_to_text(count),
        )
        .await
        .map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn claim_solar_term(
    ctx: &AppContext,
    account_id: &str,
    term_id: &str,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().claim_solar_term(term_id).await.map_err(AppError::from_core)
}

pub async fn claim_qingmei_seed(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().claim_qingmei_daily_seed().await.map_err(AppError::from_core)
}

pub async fn start_qingmei_brew(
    ctx: &AppContext,
    account_id: &str,
    input: Value,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().start_qingmei_brew(input).await.map_err(AppError::from_core)
}

pub async fn continue_qingmei_brew(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().continue_qingmei_brew().await.map_err(AppError::from_core)
}

pub async fn settle_qingmei_brew(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().settle_qingmei_brew().await.map_err(AppError::from_core)
}

pub async fn qixi(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().get_current_qixi_activity().await.map_err(AppError::from_core)
}

pub async fn claim_qixi_bridge(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().claim_qixi_bridge_rewards().await.map_err(AppError::from_core)
}

pub async fn gift_qixi_sachet(
    ctx: &AppContext,
    account_id: &str,
    friend_gid: i64,
    count: i64,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_
        .activity_center()
        .gift_qixi_sachet(friend_gid, count)
        .await
        .map_err(AppError::from_core)
}
