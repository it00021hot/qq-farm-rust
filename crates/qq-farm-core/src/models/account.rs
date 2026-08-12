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
    /// 平台（qq / wx）
    #[serde(default)]
    pub platform: String,
    /// 登录 code（一次性 auth code）
    #[serde(default)]
    pub code: String,
    /// 账号 UIN（数字 QQ 号）
    #[serde(default)]
    pub uin: String,
    /// 账号 QQ 字符串
    #[serde(default)]
    pub qq: String,
    /// 头像 URL
    #[serde(default)]
    pub avatar: String,
    /// 所属用户名（管理员/普通用户）
    #[serde(default)]
    pub username: String,
    /// 创建时间（ms）
    #[serde(default)]
    pub created_at: i64,
    /// 更新时间（ms）
    #[serde(default)]
    pub updated_at: i64,
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
            platform: String::new(),
            code: String::new(),
            uin: String::new(),
            qq: String::new(),
            avatar: String::new(),
            username: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
}
