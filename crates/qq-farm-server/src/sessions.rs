//! Admin sessions — token → user 信息。
//!
//! 1:1 对应原 `controllers/admin/admin-sessions.ts`（123 行）的核心部分。
//!
//! 持久化：admin-sessions.json（启动时 `load_persisted_sessions` 恢复 token）

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

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

impl SessionInfo {
    /// 序列化为 JSON Value
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "username": self.username,
            "role": self.role,
            "created_at": self.created_at,
            "last_active": self.last_active,
        })
    }

    /// 从 JSON 反序列化
    fn from_json(v: &serde_json::Value) -> Option<Self> {
        Some(Self {
            username: v.get("username")?.as_str()?.to_string(),
            role: v.get("role")?.as_str()?.to_string(),
            created_at: v.get("created_at")?.as_i64()?,
            last_active: v.get("last_active")?.as_i64()?,
        })
    }
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

    /// 用已有 info 创建 session（持久化恢复用）
    pub fn create_with_info(&self, token: String, info: SessionInfo) {
        self.inner.write().insert(token, info);
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

    /// 列出所有 session（用于持久化）
    #[must_use]
    pub fn dump(&self) -> Vec<(String, SessionInfo)> {
        self.inner
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

/// 持久化路径：`<dataDir>/sessions/admin-sessions.json`
#[must_use]
pub fn admin_sessions_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("sessions").join("admin-sessions.json")
}

/// 加载持久化的 sessions（启动时调用）
pub fn load_persisted_sessions(store: &SessionStore, data_dir: &Path) -> usize {
    let path = admin_sessions_path(data_dir);
    let Ok(content) = fs::read_to_string(&path) else {
        return 0;
    };
    let parsed: HashMap<String, serde_json::Value> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(_) => return 0,
    };
    let mut n = 0;
    for (token, v) in parsed {
        if let Some(info) = SessionInfo::from_json(&v) {
            store.create_with_info(token, info);
            n += 1;
        }
    }
    n
}

/// 持久化 sessions（admin 增删后调用）
pub fn persist_sessions(store: &SessionStore, data_dir: &Path) -> std::io::Result<()> {
    let path = admin_sessions_path(data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let dump = store.dump();
    let mut map = serde_json::Map::new();
    for (k, v) in dump {
        map.insert(k, v.to_json());
    }
    let content = serde_json::to_string_pretty(&serde_json::Value::Object(map))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(&path, content)
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

    // ===== 阶段 2E：持久化测试 =====

    #[test]
    fn dump_and_persist_roundtrip() {
        let tmp = std::env::temp_dir().join(format!(
            "qq-farm-test-sessions-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let store = SessionStore::new();
        store.create("tok-a".into(), "alice".into(), "admin".into());
        store.create("tok-b".into(), "bob".into(), "user".into());

        persist_sessions(&store, &tmp).expect("persist 成功");
        assert!(admin_sessions_path(&tmp).exists());

        // 重新加载
        let store2 = SessionStore::new();
        let n = load_persisted_sessions(&store2, &tmp);
        assert_eq!(n, 2);
        assert_eq!(store2.get_username("tok-a"), Some("alice".to_string()));
        assert_eq!(store2.get_role("tok-a"), Some("admin".to_string()));
        assert_eq!(store2.get_role("tok-b"), Some("user".to_string()));

        // 清理
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn load_persisted_missing_file_returns_zero() {
        let store = SessionStore::new();
        let tmp = std::env::temp_dir().join("qq-farm-nonexistent-dir-xyz");
        let n = load_persisted_sessions(&store, &tmp);
        assert_eq!(n, 0);
    }

    #[test]
    fn session_info_json_roundtrip() {
        let info = SessionInfo {
            username: "alice".into(),
            role: "admin".into(),
            created_at: 1000,
            last_active: 2000,
        };
        let json = info.to_json();
        let info2 = SessionInfo::from_json(&json).unwrap();
        assert_eq!(info2.username, "alice");
        assert_eq!(info2.role, "admin");
        assert_eq!(info2.created_at, 1000);
        assert_eq!(info2.last_active, 2000);
    }

    #[test]
    fn dump_returns_all_sessions() {
        let store = SessionStore::new();
        store.create("a".into(), "u1".into(), "user".into());
        store.create("b".into(), "u2".into(), "admin".into());
        let dump = store.dump();
        assert_eq!(dump.len(), 2);
    }
}
