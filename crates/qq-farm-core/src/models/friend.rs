//! 好友领域模型。

use serde::{Deserialize, Serialize};

/// 好友实体（用于好友农场互动）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Friend {
    /// 好友 openid
    pub open_id: String,
    /// 显示昵称
    pub display_name: String,
    /// 等级
    pub level: u32,
    /// 是否在黑名单
    pub blacklisted: bool,
}
