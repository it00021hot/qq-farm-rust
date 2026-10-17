//! 快乐不独享（分享奖励）门面（bot `share-reward-activity.ts`）。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 实例快照（activity_id：2026092501 / 2026110104）。
pub async fn snapshot(ctx: &AppContext, account_id: &str, activity_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::share_reward::ShareRewardService::new(loop_.gateway().clone())
        .get_activity(activity_id)
        .await
        .map_err(AppError::from_core)
}

/// 写操作（share/milestones/logs/daily）。
pub async fn operate(
    ctx: &AppContext,
    account_id: &str,
    activity_id: &str,
    action: &str,
    input: &Value,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::share_reward::ShareRewardService::new(loop_.gateway().clone())
        .operate(activity_id, action, input)
        .await
        .map_err(AppError::from_core)
}
