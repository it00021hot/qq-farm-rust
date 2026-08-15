//! 商城模块 stub — 后续从 server routes 迁移。

use crate::error::{AppError, AppResult};
use crate::session::AppContext;

/// 占位：商城概览（待迁移）。
pub fn commerce_overview(_ctx: &AppContext, _account_id: &str) -> AppResult<serde_json::Value> {
    Err(AppError::Internal("commerce module not yet implemented".into()))
}
