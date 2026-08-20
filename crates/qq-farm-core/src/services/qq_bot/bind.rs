//! QQ Bot 扫码绑定会话。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::store::global_config::QqBotBinding;

const BIND_SESSION_TTL_SECS: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindPollStatus {
    Pending,
    Bound,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindStartResult {
    pub session_id: String,
    pub bot_invite_url: String,
    pub qr_data_url: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindPollResult {
    pub status: BindPollStatus,
    pub binding: Option<QqBotBinding>,
}

#[derive(Debug, Clone)]
struct BindSession {
    session_id: String,
    username: String,
    created_at: i64,
    expires_at: i64,
    binding: Option<QqBotBinding>,
}

#[derive(Debug, Default)]
pub struct BindSessionManager {
    sessions: Mutex<HashMap<String, BindSession>>,
    username_to_session: Mutex<HashMap<String, String>>,
    openid_to_username: Mutex<HashMap<String, String>>,
}

impl BindSessionManager {
    pub fn start_session(&self, username: &str, bot_invite_url: &str, qr_data_url: &str) -> BindStartResult {
        let now = now_secs();
        let session_id = Uuid::new_v4().to_string();
        let expires_at = now + BIND_SESSION_TTL_SECS;
        let session = BindSession {
            session_id: session_id.clone(),
            username: username.trim().to_string(),
            created_at: now,
            expires_at,
            binding: None,
        };
        {
            let mut sessions = self.sessions.lock();
            sessions.retain(|_, s| s.username != session.username || s.expires_at > now);
            sessions.insert(session_id.clone(), session);
        }
        self.username_to_session.lock().insert(username.trim().to_string(), session_id.clone());
        BindStartResult {
            session_id,
            bot_invite_url: bot_invite_url.trim().to_string(),
            qr_data_url: qr_data_url.to_string(),
            expires_at,
        }
    }

    pub fn poll(&self, session_id: &str) -> BindPollResult {
        let now = now_secs();
        let mut sessions = self.sessions.lock();
        let Some(session) = sessions.get(session_id) else {
            return BindPollResult { status: BindPollStatus::Expired, binding: None };
        };
        if session.expires_at <= now && session.binding.is_none() {
            sessions.remove(session_id);
            return BindPollResult { status: BindPollStatus::Expired, binding: None };
        }
        if let Some(binding) = session.binding.clone() {
            return BindPollResult { status: BindPollStatus::Bound, binding: Some(binding) };
        }
        BindPollResult { status: BindPollStatus::Pending, binding: None }
    }

    /// 收到单聊消息后尝试完成绑定；返回 `(username, binding)`。
    pub fn complete_from_message(
        &self,
        user_openid: &str,
        nickname: &str,
        content: &str,
    ) -> Option<(String, QqBotBinding)> {
        let openid = user_openid.trim();
        if openid.is_empty() {
            return None;
        }
        if content.trim() == "解绑" {
            if let Some(username) = self.openid_to_username.lock().remove(openid) {
                self.username_to_session.lock().remove(&username);
                return Some((username, QqBotBinding::default()));
            }
            return None;
        }

        let now = now_secs();
        let session_id = {
            let sessions = self.sessions.lock();
            sessions
                .values()
                .filter(|s| s.binding.is_none() && s.expires_at > now)
                .max_by_key(|s| s.created_at)
                .map(|s| s.session_id.clone())
        };
        let session_id = session_id?;
        let binding = QqBotBinding {
            user_openid: openid.to_string(),
            bound_at: now,
            nickname: nickname.trim().to_string(),
        };
        let username = {
            let mut sessions = self.sessions.lock();
            let session = sessions.get_mut(&session_id)?;
            session.binding = Some(binding.clone());
            session.username.clone()
        };
        self.openid_to_username.lock().insert(openid.to_string(), username.clone());
        Some((username, binding))
    }

    pub fn username_for_openid(&self, user_openid: &str) -> Option<String> {
        self.openid_to_username.lock().get(user_openid.trim()).cloned()
    }

    pub fn clear_user(&self, username: &str) {
        let openid = {
            let sessions = self.sessions.lock();
            sessions
                .values()
                .find_map(|s| s.binding.as_ref().filter(|_| s.username == username))
                .map(|b| b.user_openid.clone())
        };
        self.username_to_session.lock().remove(username);
        self.sessions.lock().retain(|_, s| s.username != username);
        if let Some(openid) = openid.filter(|v| !v.is_empty()) {
            self.openid_to_username.lock().remove(&openid);
        }
        let to_remove: Vec<String> = self
            .openid_to_username
            .lock()
            .iter()
            .filter_map(|(openid, u)| (u == username).then_some(openid.clone()))
            .collect();
        for openid in to_remove {
            self.openid_to_username.lock().remove(&openid);
        }
    }

    pub fn register_binding(&self, username: &str, binding: &QqBotBinding) {
        if !binding.is_bound() {
            return;
        }
        self.openid_to_username
            .lock()
            .insert(binding.user_openid.clone(), username.trim().to_string());
    }
}

pub type SharedBindSessionManager = Arc<BindSessionManager>;

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_session_lifecycle() {
        let mgr = BindSessionManager::default();
        let start = mgr.start_session("alice", "https://q.qq.com/qqbot/1", "data:image/png;base64,abc");
        assert_eq!(mgr.poll(&start.session_id).status, BindPollStatus::Pending);

        let done = mgr
            .complete_from_message("openid-1", "nick", "hello")
            .expect("bound");
        assert_eq!(done.0, "alice");
        assert_eq!(done.1.user_openid, "openid-1");

        let poll = mgr.poll(&start.session_id);
        assert_eq!(poll.status, BindPollStatus::Bound);
        assert_eq!(poll.binding.unwrap().nickname, "nick");
    }

    #[test]
    fn unbind_clears_mapping() {
        let mgr = BindSessionManager::default();
        let start = mgr.start_session("bob", "https://example.com", "");
        mgr.complete_from_message("openid-bob", "", "hi");
        assert!(mgr.username_for_openid("openid-bob").is_some());
        mgr.complete_from_message("openid-bob", "", "解绑");
        assert!(mgr.username_for_openid("openid-bob").is_none());
        mgr.clear_user("bob");
        assert_eq!(mgr.poll(&start.session_id).status, BindPollStatus::Expired);
    }
}
