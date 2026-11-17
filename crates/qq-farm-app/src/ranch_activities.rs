//! 牧场活动目录门面。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::farm::require_worker_loop;
use crate::session::AppContext;

/// 场景 2 的活动目录。开始和结束时间是服务器秒。
pub async fn snapshot(ctx: &AppContext, account_id: &str) -> AppResult<Value> {
    let loop_ = require_worker_loop(ctx, account_id)?;
    qq_farm_core::services::ranch_activities::RanchActivities::new(loop_.gateway().clone())
        .snapshot()
        .await
        .map_err(AppError::from_core)
}
