//! 活动中心 stub — 后续从 server routes 迁移。

use crate::error::{AppError, AppResult};
use crate::session::AppContext;

/// 占位：活动中心状态（待迁移）。
pub fn activity_state(_ctx: &AppContext, _account_id: &str) -> AppResult<serde_json::Value> {
    Err(AppError::Internal("activity module not yet implemented".into()))
}
