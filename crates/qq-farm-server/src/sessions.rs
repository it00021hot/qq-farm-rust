//! Admin sessions — token → user 信息。
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
    pub role: String,
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
    pub fn create(&self, token: String, username: String, role: String) {
        let now = now_ms();
        self.inner.write().insert(
            token,
            SessionInfo { username, role, created_at: now, last_active: now },
        );
    }

    /// 获取完整 session
    #[must_use]
    pub fn get(&self, token: &str) -> Option<SessionInfo> {
        self.inner.read().get(token).cloned()
    }

    /// 获取 username
    #[must_use]
    pub fn get_username(&self, token: &str) -> Option<String> {
        self.inner.read().get(token).map(|s| s.username.clone())
    }

    /// 获取 role
    #[must_use]
    pub fn get_role(&self, token: &str) -> Option<String> {
        self.inner.read().get(token).map(|s| s.role.clone())
    }

    /// 是否 admin
    #[must_use]
    pub fn is_admin(&self, token: &str) -> bool {
        self.get_role(token).as_deref() == Some("admin")
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

    /// 替换某 username 的所有 session（用于 edit_user 时让其他 token 失效）
    pub fn invalidate_by_username(&self, username: &str) -> usize {
        let mut guard = self.inner.write();
        let to_remove: Vec<String> = guard
            .iter()
            .filter(|(_, s)| s.username == username)
            .map(|(k, _)| k.clone())
            .collect();
        let n = to_remove.len();
        for k in to_remove {
            guard.remove(&k);
        }
        n
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
        store.create("tok-1".to_string(), "alice".to_string(), "user".to_string());
        let s = store.get("tok-1").unwrap();
        assert_eq!(s.username, "alice");
        assert_eq!(s.role, "user");
    }

    #[test]
    fn get_missing_returns_none() {
        let store = SessionStore::new();
        assert_eq!(store.get_username("nonexistent"), None);
        assert_eq!(store.get_role("nonexistent"), None);
    }

    #[test]
    fn delete_removes() {
        let store = SessionStore::new();
        store.create("tok".to_string(), "u".to_string(), "user".to_string());
        store.delete("tok");
        assert_eq!(store.get_username("tok"), None);
    }

    #[test]
    fn touch_updates_active() {
        let store = SessionStore::new();
        store.create("tok".to_string(), "u".to_string(), "user".to_string());
        store.touch("tok");
        assert_eq!(store.get_username("tok"), Some("u".to_string()));
    }

    #[test]
    fn cleanup_expired_removes_old() {
        let store = SessionStore::new();
        store.create("tok".to_string(), "u".to_string(), "user".to_string());
        store.cleanup_expired(0);
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn count_increments() {
        let store = SessionStore::new();
        assert_eq!(store.count(), 0);
        store.create("a".into(), "u1".into(), "user".into());
        store.create("b".into(), "u2".into(), "admin".into());
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn is_admin_checks_role() {
        let store = SessionStore::new();
        store.create("a".into(), "u1".into(), "user".into());
        store.create("b".into(), "u2".into(), "admin".into());
        assert!(!store.is_admin("a"));
        assert!(store.is_admin("b"));
    }

    #[test]
    fn invalidate_by_username() {
        let store = SessionStore::new();
        store.create("a".into(), "u1".into(), "user".into());
        store.create("b".into(), "u2".into(), "user".into());
        store.create("c".into(), "u1".into(), "user".into());
        let n = store.invalidate_by_username("u1");
        assert_eq!(n, 2);
        assert_eq!(store.count(), 1);
        assert_eq!(store.get_username("b"), Some("u2".to_string()));
    }
}
