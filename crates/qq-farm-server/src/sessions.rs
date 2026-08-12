//! Admin sessions — 简单的 token → username 映射。
//!
//! 1:1 对应原 `controllers/admin/admin-sessions.ts`（123 行）的核心部分。

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

/// Admin session store
#[derive(Default, Clone)]
pub struct SessionStore {
    inner: Arc<RwLock<HashMap<String, SessionInfo>>>,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub username: String,
    pub created_at: i64,
    pub last_active: i64,
}

impl SessionStore {
    /// 构造
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建 session
    pub fn create(&self, token: String, username: String) {
        let now = now_ms();
        self.inner.write().insert(
            token,
            SessionInfo { username, created_at: now, last_active: now },
        );
    }

    /// 获取 username
    #[must_use]
    pub fn get_username(&self, token: &str) -> Option<String> {
        self.inner.read().get(token).map(|s| s.username.clone())
    }

    /// 触碰 session（更新 last_active）
    pub fn touch(&self, token: &str) {
        if let Some(s) = self.inner.write().get_mut(token) {
            s.last_active = now_ms();
        }
    }

    /// 删除 session
    pub fn delete(&self, token: &str) {
        self.inner.write().remove(token);
    }

    /// 清理过期 session（> TTL）
    pub fn cleanup_expired(&self, ttl_ms: i64) {
        let now = now_ms();
        self.inner.write().retain(|_, s| now - s.last_active < ttl_ms);
    }

    /// 数量
    #[must_use]
    pub fn count(&self) -> usize {
        self.inner.read().len()
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get() {
        let store = SessionStore::new();
        store.create("tok-1".to_string(), "alice".to_string());
        assert_eq!(store.get_username("tok-1"), Some("alice".to_string()));
    }

    #[test]
    fn get_missing_returns_none() {
        let store = SessionStore::new();
        assert_eq!(store.get_username("nonexistent"), None);
    }

    #[test]
    fn delete_removes() {
        let store = SessionStore::new();
        store.create("tok".to_string(), "u".to_string());
        store.delete("tok");
        assert_eq!(store.get_username("tok"), None);
    }

    #[test]
    fn touch_updates_active() {
        let store = SessionStore::new();
        store.create("tok".to_string(), "u".to_string());
        store.touch("tok");
        assert_eq!(store.get_username("tok"), Some("u".to_string()));
    }

    #[test]
    fn cleanup_expired_removes_old() {
        let store = SessionStore::new();
        store.create("tok".to_string(), "u".to_string());
        // TTL = 0 → 全部过期
        store.cleanup_expired(0);
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn count_increments() {
        let store = SessionStore::new();
        assert_eq!(store.count(), 0);
        store.create("a".into(), "u1".into());
        store.create("b".into(), "u2".into());
        assert_eq!(store.count(), 2);
    }
}
