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
    loop_.activity_center().gift_qixi_sachet(friend_gid, count).await.map_err(AppError::from_core)
}

pub async fn charity(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let dto = loop_
        .activity_center()
        .get_current_charity_red_flower_activity()
        .await
        .map_err(AppError::from_core)?;
    serde_json::to_value(dto).map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn claim_charity_seeds(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().claim_charity_red_flower_seeds().await.map_err(AppError::from_core)
}

pub async fn donate_charity_love(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().donate_charity_red_flower_love().await.map_err(AppError::from_core)
}

pub async fn claim_charity_daily_gift(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().claim_charity_red_flower_daily_gift().await.map_err(AppError::from_core)
}

pub async fn claim_charity_progress_reward(
    ctx: &AppContext,
    account_id: &str,
    target: &str,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_
        .activity_center()
        .claim_charity_red_flower_progress_reward(target)
        .await
        .map_err(AppError::from_core)
}

/// 萌宠成长日记快照。
pub async fn pet_diary(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().get_pet_diary().await.map_err(AppError::from_core)
}

/// 萌宠成长日记写操作（feed/draw/battle/exchange/… 共 15 个动作 + solar）。
pub async fn operate_pet_diary(
    ctx: &AppContext,
    account_id: &str,
    action: &str,
    params: &Value,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().operate_pet_diary(action, params).await.map_err(AppError::from_core)
}

/// 萌宠成长日记互动 / 被夺日志。
pub async fn pet_diary_records(ctx: &AppContext, account_id: &str, kind: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    let records =
        loop_.activity_center().get_pet_diary_records(kind).await.map_err(AppError::from_core)?;
    serde_json::to_value(records).map_err(|e| AppError::Internal(e.to_string()))
}

/// 好友的萌宠活动信息（夺宝前置查询）。
pub async fn pet_diary_friend(ctx: &AppContext, account_id: &str, gid: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    loop_.activity_center().get_pet_diary_friend(gid).await.map_err(AppError::from_core)
}
