//! 节令日历门面。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 节令日历快照（含事件 / 季度 / 服务器时间）。
pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::solar_calendar::SolarCalendarService::new(loop_.gateway().clone())
        .get_calendar()
        .await
        .map_err(AppError::from_core)
}

/// 节令操作：claim / answer / season。
pub async fn operate(
    ctx: &AppContext,
    account_id: &str,
    action: &str,
    input: &Value,
) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::solar_calendar::SolarCalendarService::new(loop_.gateway().clone())
        .operate(action, input)
        .await
        .map_err(AppError::from_core)
}
