//! 好友模块 stub — 后续从 server routes 迁移。

use crate::error::{AppError, AppResult};
use crate::session::AppContext;

/// 占位：好友列表（待迁移）。
pub fn list_friends(_ctx: &AppContext, _account_id: &str) -> AppResult<serde_json::Value> {
    Err(AppError::Internal("friend module not yet implemented".into()))
}
