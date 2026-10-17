//! 奶茶活动门面（bot `milk-tea.ts`，1.15 新活动）。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 活动快照（page：shop/settings/upgrade/workshop/hireList/exchange/fund/work/rank/logs）。
pub async fn snapshot(
    ctx: &AppContext,
    account_id: &str,
    page: &str,
    input: &Value,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::milk_tea::MilkTeaService::new(loop_.gateway().clone())
        .get_activity(page, input)
        .await
        .map_err(AppError::from_core)
}

/// 写操作（create/rename/boost/claim/upgrade/craft/hire/exchange/unlock/fundClaim/wages/interact/hireSetting）。
pub async fn operate(
    ctx: &AppContext,
    account_id: &str,
    action: &str,
    input: &Value,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::milk_tea::MilkTeaService::new(loop_.gateway().clone())
        .operate(action, input)
        .await
        .map_err(AppError::from_core)
}
