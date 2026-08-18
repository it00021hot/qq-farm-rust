//! 好友模块 L3 进程内状态 — 按账号隔离 quiet hours 与好友列表缓存。

use std::collections::HashMap;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::constants::{DEFAULT_FRIENDS_LIST_CACHE_TTL_MS, MIN_FRIENDS_LIST_CACHE_TTL_MS};

/// 好友安静时段配置
#[derive(Debug, Clone, Default)]
pub struct FriendQuietHours {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

/// 好友列表缓存条目
#[derive(Debug, Clone, Default)]
pub struct FriendsListCache {
    pub friends: Vec<serde_json::Value>,
    pub time_ms: u64,
}

impl FriendsListCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_ttl_ms(&self, configured_ttl_sec: i64) -> u64 {
        if configured_ttl_sec <= 0 {
            return DEFAULT_FRIENDS_LIST_CACHE_TTL_MS;
        }
        let ms = (configured_ttl_sec as u64) * 1000;
        ms.max(MIN_FRIENDS_LIST_CACHE_TTL_MS)
    }
}

/// 单账号好友运行时状态（Phase 4 L3 globals）
#[derive(Debug, Clone, Default)]
pub struct FriendRuntimeState {
    pub quiet_hours: Option<FriendQuietHours>,
    pub friends_list_cache: Option<FriendsListCache>,
}

static FRIEND_STATES: Lazy<Mutex<HashMap<String, FriendRuntimeState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn with_state_mut<R>(account_id: &str, f: impl FnOnce(&mut FriendRuntimeState) -> R) -> R {
    let mut guard = FRIEND_STATES.lock();
    f(guard.entry(account_id.to_string()).or_default())
}

/// 读取账号 quiet hours 配置（进程内覆盖层）
#[must_use]
pub fn get_friend_quiet_hours(account_id: &str) -> Option<FriendQuietHours> {
    FRIEND_STATES.lock().get(account_id).and_then(|s| s.quiet_hours.clone())
}

/// 设置账号 quiet hours（线程安全）
pub fn set_friend_quiet_hours(account_id: &str, cfg: Option<FriendQuietHours>) {
    with_state_mut(account_id, |s| s.quiet_hours = cfg);
}

/// 读取账号好友列表缓存
#[must_use]
pub fn get_friends_list_cache(account_id: &str) -> Option<FriendsListCache> {
    FRIEND_STATES.lock().get(account_id).and_then(|s| s.friends_list_cache.clone())
}

/// 写入账号好友列表缓存
pub fn set_friends_list_cache(account_id: &str, cache: Option<FriendsListCache>) {
    with_state_mut(account_id, |s| s.friends_list_cache = cache);
}

/// 清除指定账号的好友列表缓存
pub fn clear_friends_list_cache(account_id: &str) {
    set_friends_list_cache(account_id, None);
}

/// 清除所有账号的好友列表缓存
pub fn clear_all_friends_list_cache() {
    for state in FRIEND_STATES.lock().values_mut() {
        state.friends_list_cache = None;
    }
}
