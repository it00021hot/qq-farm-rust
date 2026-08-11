//! 账号领域模型。

use serde::{Deserialize, Serialize};

/// 账号登录态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    /// 未登录
    Idle,
    /// 登录中
    Logging,
    /// 已登录，挂机运行中
    Running,
    /// 掉线，等待重连
    Reconnecting,
    /// 已停止
    Stopped,
}

/// 账号实体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// 账号唯一 ID
    pub id: String,
    /// 登录用 openid（游戏会话）
    pub open_id: String,
    /// 显示名（备注/昵称）
    pub display_name: String,
    /// 当前状态
    pub status: AccountStatus,
}

impl Account {
    /// 创建新账号
    #[must_use]
    pub fn new(id: impl Into<String>, open_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            open_id: open_id.into(),
            display_name: display_name.into(),
            status: AccountStatus::Idle,
        }
    }
}
