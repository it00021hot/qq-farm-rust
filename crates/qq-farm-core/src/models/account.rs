//! 账号领域模型（运行时登录会话）。

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

/// 运行时账号会话（worker / engine 使用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSession {
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

/// 兼容旧名；请改用 [`AccountSession`]。
#[deprecated(note = "use AccountSession")]
pub type Account = AccountSession;

impl AccountSession {
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

    /// 从持久化账号构造运行时账号，保留 platform / code（对齐 TS worker `platform: account.platform`）。
    #[must_use]
    pub fn from_store(acc: &crate::models::store::accounts::AccountRecord) -> Self {
        let code = acc.code.clone();
        let display_name = if acc.name.trim().is_empty() {
            acc.nick.clone()
        } else {
            acc.name.clone()
        };
        Self {
            id: acc.id.clone(),
            open_id: code.clone(),
            display_name,
            status: AccountStatus::Idle,
            platform: acc.platform.clone(),
            code,
            uin: acc.uin.clone(),
            qq: acc.qq.clone(),
            avatar: acc.avatar.clone(),
            username: acc.username.clone(),
            created_at: acc.created_at,
            updated_at: acc.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::store::accounts::AccountRecord;

    #[test]
    fn from_store_keeps_wx_platform_and_code() {
        let store = AccountRecord {
            id: "2".into(),
            name: "账号2".into(),
            nick: String::new(),
            code: "wx-one-time".into(),
            platform: "wx".into(),
            uin: String::new(),
            qq: String::new(),
            avatar: String::new(),
            username: "user".into(),
            created_at: 1,
            updated_at: 2,
        };
        let acc = AccountSession::from_store(&store);
        assert_eq!(acc.platform, "wx");
        assert_eq!(acc.code, "wx-one-time");
        assert_eq!(acc.open_id, "wx-one-time");
        assert_eq!(acc.display_name, "账号2");
        assert_eq!(acc.username, "user");
    }
}
