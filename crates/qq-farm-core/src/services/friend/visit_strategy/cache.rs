//! 好友列表进程内缓存（Phase 4 按账号隔离）。

pub use crate::services::friend::runtime_state::FriendsListCache;

use crate::services::friend::runtime_state;

/// 清除指定账号的好友列表缓存
pub fn clear_friends_list_cache(account_id: &str) {
    runtime_state::clear_friends_list_cache(account_id);
}
