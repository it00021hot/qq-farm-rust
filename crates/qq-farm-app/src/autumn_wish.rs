//! 秋祈良愿门面。只读当前账号的 worker 网关，不另开定时任务。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::autumn_wish::AutumnWishService::new(loop_.gateway().clone())
        .snapshot()
        .await
        .map_err(AppError::from_core)
}

pub async fn operate(
    ctx: &AppContext,
    account_id: &str,
    action: &str,
    choose_id: i64,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::autumn_wish::AutumnWishService::new(loop_.gateway().clone())
        .operate(action, choose_id)
        .await
        .map_err(AppError::from_core)
}
