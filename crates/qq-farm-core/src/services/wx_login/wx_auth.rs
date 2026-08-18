//! 应用宝凭据与换票错误分类。

use std::time::{SystemTime, UNIX_EPOCH};

/// 换票 / 续期失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WxAuthErrorKind {
    /// 凭据已失效，需重新扫码。
    CredentialsDead,
    /// 网络或临时错误，可重试。
    Transient,
}

/// 应用宝 OAuth 换票错误。
#[derive(Debug, Clone)]
pub struct WxAuthError {
    pub kind: WxAuthErrorKind,
    pub message: String,
}

impl WxAuthError {
    #[must_use]
    pub fn dead(message: impl Into<String>) -> Self {
        Self { kind: WxAuthErrorKind::CredentialsDead, message: message.into() }
    }

    #[must_use]
    pub fn transient(message: impl Into<String>) -> Self {
        Self { kind: WxAuthErrorKind::Transient, message: message.into() }
    }
}

impl std::fmt::Display for WxAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for WxAuthError {}

/// 应用宝持久化凭据（openid + token 三件套 + login_buffer）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct YybCredentials {
    pub openid: String,
    pub access_token: String,
    pub refresh_token: String,
    pub login_buffer: String,
    /// Unix 秒；0 表示未知。
    pub expires_at: i64,
    pub expires_in: i64,
    /// 当前 refresh_token 首次观察到的 Unix 秒；0 表示尚未记时。
    pub refresh_token_observed_at: i64,
}

impl YybCredentials {
    /// token 是否在 `ahead_secs` 内过期（或已过期）。
    #[must_use]
    pub fn token_due_for_refresh(&self, ahead_secs: i64) -> bool {
        if self.expires_at <= 0 {
            return true;
        }
        now_unix() + ahead_secs.max(0) >= self.expires_at
    }

    /// 仅当微信返回了不同的 refresh_token 时重置观察时间。
    #[must_use]
    pub fn apply_new_refresh_token(mut self, new_refresh: &str, now: i64) -> Self {
        let new_refresh = new_refresh.trim();
        if !new_refresh.is_empty() && new_refresh != self.refresh_token {
            self.refresh_token = new_refresh.to_string();
            self.refresh_token_observed_at = now;
        }
        self.ensure_observed_at(now)
    }

    /// 已有 refresh_token 但尚未记时则从 now 开始。
    #[must_use]
    pub fn ensure_observed_at(mut self, now: i64) -> Self {
        if !self.refresh_token.trim().is_empty() && self.refresh_token_observed_at <= 0 {
            self.refresh_token_observed_at = now;
        }
        self
    }
}

/// 同一 refresh_token 连续使用约 25 天后建议重扫。
#[must_use]
pub fn rescan_recommended(refresh_token: &str, observed_at: i64, now: i64) -> bool {
    if refresh_token.trim().is_empty() || observed_at <= 0 {
        return false;
    }
    now.saturating_sub(observed_at) >= crate::constants::WX_REFRESH_TOKEN_RESCAN_SECS
}

#[must_use]
pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn classify_yyb_message(msg: &str) -> WxAuthErrorKind {
    let m = msg.trim();
    if m.contains("WeChat login buffer response is invalid")
        || m.contains("Missing Yingyongbao authorization")
        || m.contains("WeChat login session has not been confirmed")
        || m.contains("refresh failed")
        || m.contains("refresh response missing")
        || m.contains("invalid quick authorization")
        || m.contains("quick authorization code is missing")
    {
        WxAuthErrorKind::CredentialsDead
    } else {
        WxAuthErrorKind::Transient
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_invalid_buffer_as_dead() {
        assert_eq!(
            classify_yyb_message("WeChat login buffer response is invalid"),
            WxAuthErrorKind::CredentialsDead
        );
    }

    #[test]
    fn classify_http_as_transient() {
        assert_eq!(
            classify_yyb_message("Unable to obtain WeChat login buffer (HTTP 502)"),
            WxAuthErrorKind::Transient
        );
    }

    #[test]
    fn token_due_for_refresh_uses_keepalive_window() {
        let c = YybCredentials {
            openid: String::new(),
            access_token: String::new(),
            refresh_token: String::new(),
            login_buffer: String::new(),
            expires_at: now_unix() + 1800,
            expires_in: 7200,
            refresh_token_observed_at: 0,
        };
        assert!(!c.token_due_for_refresh(0), "token with 30m left should not refresh when ahead=0");
        assert!(
            c.token_due_for_refresh(crate::constants::WX_KEEPALIVE_AHEAD_SECS),
            "token with 30m left should refresh under 45m keepalive window"
        );
        let fresh = YybCredentials { expires_at: now_unix() + 7200, ..c.clone() };
        assert!(!fresh.token_due_for_refresh(crate::constants::WX_KEEPALIVE_AHEAD_SECS));
        let unknown = YybCredentials { expires_at: 0, ..c };
        assert!(unknown.token_due_for_refresh(0));
    }

    #[test]
    fn rescan_recommended_after_twenty_five_days() {
        let now = now_unix();
        assert!(!rescan_recommended("rt", now - 24 * 24 * 60 * 60, now));
        assert!(rescan_recommended("rt", now - 26 * 24 * 60 * 60, now));
        assert!(!rescan_recommended("", now - 26 * 24 * 60 * 60, now));
        assert!(!rescan_recommended("rt", 0, now));
    }

    #[test]
    fn apply_new_refresh_token_only_resets_on_rotate() {
        let now = now_unix();
        let base = YybCredentials {
            refresh_token: "rt".into(),
            refresh_token_observed_at: 50,
            ..Default::default()
        };
        let same = base.clone().apply_new_refresh_token("rt", now);
        assert_eq!(same.refresh_token_observed_at, 50);
        let rotated = base.apply_new_refresh_token("rt2", now);
        assert_eq!(rotated.refresh_token, "rt2");
        assert_eq!(rotated.refresh_token_observed_at, now);
    }
}
