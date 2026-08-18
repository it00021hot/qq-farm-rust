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
}
