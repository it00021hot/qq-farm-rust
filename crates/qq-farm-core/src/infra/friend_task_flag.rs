//! 好友巡查任务标记（L2 全局，统一入口）。
//!
//! 好友巡查（`FriendService::visit_batch*`）执行期间置位；天气扫描 / 宠物
//! 同步等同样要进出好友农场的后台任务据此让路（对齐 bot
//! `isFriendCheckRunning` / `waitForFriendTaskIdle`）。

use std::collections::HashMap;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

static FLAGS: Lazy<Mutex<HashMap<String, bool>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 置位 / 复位某账号的好友巡查标记
pub fn set_friend_checking(account_id: &str, running: bool) {
    let mut flags = FLAGS.lock();
    if running {
        flags.insert(account_id.to_string(), true);
    } else {
        flags.remove(account_id);
    }
}

/// 查询某账号的好友巡查是否在跑
#[must_use]
pub fn is_friend_checking(account_id: &str) -> bool {
    FLAGS.lock().get(account_id).copied().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_clear_flag() {
        let account = "test-friend-task-flag-account";
        assert!(!is_friend_checking(account));
        set_friend_checking(account, true);
        assert!(is_friend_checking(account));
        set_friend_checking(account, false);
        assert!(!is_friend_checking(account));
    }
}
