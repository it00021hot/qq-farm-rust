//! 面板认证 stub — 桌面端可跳过，server 仍用 SessionStore。

use crate::error::{AppError, AppResult};

/// 面板用户会话（最小 stub，供 desktop 后续扩展）。
#[derive(Debug, Clone)]
pub struct PanelSession {
    pub username: String,
    pub role: String,
    pub token: String,
}

/// 占位：验证 token（待 server sessions 迁移）。
pub fn validate_token(_token: &str) -> AppResult<PanelSession> {
    Err(AppError::Internal("auth module not yet implemented".into()))
}
