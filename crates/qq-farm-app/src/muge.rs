//! 牧歌初启门面。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 活动快照。
pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::muge::MugeService::new(loop_.gateway().clone())
        .snapshot()
        .await
        .map_err(AppError::from_core)
}

/// 领奖、保存答案、结算或领取分享奖励。
pub async fn operate(
    ctx: &AppContext,
    account_id: &str,
    action: &str,
    input: &Value,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::muge::MugeService::new(loop_.gateway().clone())
        .operate(action, input)
        .await
        .map_err(AppError::from_core)
}
