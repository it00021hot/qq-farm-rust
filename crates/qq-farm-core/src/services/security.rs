//! 安全服务 — 密码加密 / 验证 / 强度 / 登录限流 / 会话 token。
//!
//! 1:1 翻译原 `core/src/services/security.ts`（339 行）。
//!
//! ## 协议分层
//!
//! - **密码学**（hash/verify） — 重导出 [`crate::models::user_store::auth`]，避免重复
//! - **登录限流**（account lockout / IP rate limit） — 重导出同上
//! - **密码强度** — 重导出同上
//! - **会话 token**（新增） — 本模块提供
//! - **IP 提取 / 客户端风险评估**（新增） — 本模块提供
//!
//! ## 与原 TS 的差异
//!
//! - 原 TS 的 `passwordHashMiddleware` / `rateLimitMiddleware` 是 HTTP 中间件，
//!   本模块只提供底层函数（`get_client_ip_from_headers` 等），由 `qq-farm-server` crate
//!   的 axum 层负责装配
//! - `SECURITY_CONFIG` 字段值与原 TS 完全一致（`saltRounds=12` 在 Rust 里没有用，
//!   因为我们用 PBKDF2 不是 bcrypt；保留 12 仅为字段兼容）

use std::collections::HashMap;

use parking_lot::Mutex;
use serde::Serialize;

// =====================================================================
// 配置
// =====================================================================

/// 安全配置（1:1 对齐原 TS `SECURITY_CONFIG`）
#[derive(Debug, Clone, Serialize)]
pub struct SecurityConfig {
    pub salt_rounds: u32,
    pub min_password_length: usize,
    pub max_password_length: usize,
    pub enable_password_strength_check: bool,
    pub max_login_attempts: u32,
    pub lockout_duration_ms: i64,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            salt_rounds: 12,
            min_password_length: 4,
            max_password_length: 64,
            enable_password_strength_check: true,
            max_login_attempts: 5,
            lockout_duration_ms: 300_000,
        }
    }
}

/// 默认全局配置
pub static SECURITY_CONFIG: SecurityConfig = SecurityConfig {
    salt_rounds: 12,
    min_password_length: 4,
    max_password_length: 64,
    enable_password_strength_check: true,
    max_login_attempts: 5,
    lockout_duration_ms: 300_000,
};

// =====================================================================
// 会话 token
// =====================================================================

/// 会话 token
#[derive(Debug, Clone, Serialize)]
pub struct SessionToken {
    pub token: String,
    pub expires_at: i64,
    pub created_at: i64,
}

/// 生成 32 字节随机 hex token
#[must_use]
pub fn generate_token(length: usize) -> String {
    use rand::RngCore;
    let mut bytes = vec![0u8; length];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 生成 24 小时过期的会话 token
#[must_use]
pub fn generate_session_token() -> SessionToken {
    let now = now_ms();
    SessionToken {
        token: generate_token(32),
        expires_at: now + 24 * 60 * 60 * 1000,
        created_at: now,
    }
}

/// 验证会话 token
#[must_use]
pub fn verify_session_token(token: &str, expires_at: i64) -> bool {
    if token.is_empty() || expires_at <= 0 {
        return false;
    }
    now_ms() <= expires_at
}

// =====================================================================
// 客户端 IP 提取
// =====================================================================

/// 客户端 IP 提取结果
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientIpInfo {
    /// 提取的 IP 字符串；无法识别时为 `"unknown"`
    pub ip: String,
    /// 命中的来源：`cf-connecting-ip` / `x-real-ip` / `x-forwarded-for` / `req_ip` / `socket`
    pub source: &'static str,
}

/// 从 header map 提取客户端 IP
///
/// `headers` 是只读 header map（key 不区分大小写，本函数内部已处理）
///
/// 优先级（1:1 对齐原 TS `getClientIp`）：
/// 1. `cf-connecting-ip`
/// 2. `x-real-ip`
/// 3. `x-forwarded-for`（取第一段）
/// 4. 直连 IP（`req.ip`，过滤 `::1` / `::ffff:127.0.0.1`）
/// 5. `socket.remoteAddress`（剥 `::ffff:` 前缀）
/// 6. `"unknown"`
pub fn extract_client_ip(
    headers: &HashMap<String, String>,
    fallback_ip: Option<&str>,
    fallback_socket: Option<&str>,
) -> ClientIpInfo {
    let get_header = |name: &str| -> Option<String> {
        // 尝试不区分大小写查找
        if let Some(v) = headers.get(name) {
            return Some(v.clone());
        }
        let lower = name.to_ascii_lowercase();
        headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.clone())
    };

    let source_order: &[&str] = &["cf-connecting-ip", "x-real-ip", "x-forwarded-for"];
    for &name in source_order {
        if let Some(v) = get_header(name) {
            let s = v.trim();
            if !s.is_empty() {
                if name == "x-forwarded-for" {
                    let first = s.split(',').next().unwrap_or("").trim();
                    if !first.is_empty() {
                        return ClientIpInfo {
                            ip: first.to_string(),
                            source: "x-forwarded-for",
                        };
                    }
                } else {
                    return ClientIpInfo {
                        ip: s.to_string(),
                        source: match name {
                            "cf-connecting-ip" => "cf-connecting-ip",
                            "x-real-ip" => "x-real-ip",
                            _ => "headers",
                        },
                    };
                }
            }
        }
    }
    if let Some(ip) = fallback_ip {
        if !ip.is_empty() && ip != "::1" && ip != "::ffff:127.0.0.1" {
            return ClientIpInfo {
                ip: ip.to_string(),
                source: "req_ip",
            };
        }
    }
    if let Some(addr) = fallback_socket {
        if let Some(stripped) = addr.strip_prefix("::ffff:") {
            return ClientIpInfo {
                ip: stripped.to_string(),
                source: "socket",
            };
        }
        if !addr.is_empty() {
            return ClientIpInfo {
                ip: addr.to_string(),
                source: "socket",
            };
        }
    }
    ClientIpInfo {
        ip: "unknown".to_string(),
        source: "unknown",
    }
}

// =====================================================================
// 进程内通用限流器
// =====================================================================

/// 限流记录
#[derive(Debug, Clone, Default)]
struct RateLimitRecord {
    count: u32,
    reset_at: i64,
}

/// 进程内通用限流器（滑动窗口近似）
///
/// 与 user_store/auth.rs 的 IP 限流不同：
/// - `user_store::auth::check_rate_limit`：按 IP 持久化（写到文件）
/// - 本类型：纯内存瞬时限流（适合作通用 API 限流中间件底层）
pub struct RateLimiter {
    window_ms: i64,
    max_requests: u32,
    store: Mutex<HashMap<String, RateLimitRecord>>,
}

impl RateLimiter {
    #[must_use]
    pub fn new(window_ms: i64, max_requests: u32) -> Self {
        Self {
            window_ms,
            max_requests,
            store: Mutex::new(HashMap::new()),
        }
    }

    /// 检查并递增计数
    ///
    /// 返回 `RateLimitDecision`：
    /// - `Allowed` — 通过
    /// - `Limited` — 超过 `max_requests`，返回 `retry_after_secs`
    pub fn check(&self, key: &str) -> RateLimitDecision {
        let now = now_ms();
        let mut store = self.store.lock();
        let record = store.entry(key.to_string()).or_insert_with(|| RateLimitRecord {
            count: 0,
            reset_at: now + self.window_ms,
        });
        if now > record.reset_at {
            record.count = 0;
            record.reset_at = now + self.window_ms;
        }
        record.count += 1;
        if record.count > self.max_requests {
            return RateLimitDecision::Limited {
                retry_after_secs: ((record.reset_at - now).max(0) + 999) / 1000,
            };
        }
        RateLimitDecision::Allowed {
            remaining: self.max_requests - record.count,
            reset_at: record.reset_at,
        }
    }

    /// 清理过期记录（建议定时调用）
    pub fn cleanup_expired(&self) {
        let now = now_ms();
        self.store.lock().retain(|_, r| r.reset_at > now);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allowed {
        remaining: u32,
        reset_at: i64,
    },
    Limited {
        retry_after_secs: i64,
    },
}

// =====================================================================
// 工具
// =====================================================================

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// =====================================================================
// 重导出（向后兼容原 TS `require('./security')` 调用）
// =====================================================================

pub use crate::models::user_store::auth::{
    check_account_lockout as _check_account_lockout,
    check_rate_limit as _check_ip_rate_limit, clear_failed_attempts as _clear_login_attempts,
    hash_password, record_failed_attempt as _record_login_attempt, validate_password_strength,
    verify_password, FailedAttemptResult, LockoutResult, PasswordStrengthResult, RateLimitResult,
};

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn security_config_default() {
        let c = SecurityConfig::default();
        assert_eq!(c.salt_rounds, 12);
        assert_eq!(c.min_password_length, 4);
        assert_eq!(c.max_password_length, 64);
        assert!(c.enable_password_strength_check);
        assert_eq!(c.max_login_attempts, 5);
        assert_eq!(c.lockout_duration_ms, 300_000);
    }

    #[test]
    fn generate_token_length() {
        // 32 bytes -> 64 hex chars
        let t = generate_token(32);
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_token_uniqueness() {
        let t1 = generate_token(16);
        let t2 = generate_token(16);
        assert_ne!(t1, t2);
    }

    #[test]
    fn generate_session_token_has_expiry() {
        let s = generate_session_token();
        assert!(!s.token.is_empty());
        assert!(s.expires_at > s.created_at);
        // 24h
        assert_eq!(s.expires_at - s.created_at, 24 * 60 * 60 * 1000);
    }

    #[test]
    fn verify_session_token_valid() {
        let s = generate_session_token();
        assert!(verify_session_token(&s.token, s.expires_at));
    }

    #[test]
    fn verify_session_token_empty_token() {
        assert!(!verify_session_token("", now_ms() + 1000));
    }

    #[test]
    fn verify_session_token_expired() {
        let past = now_ms() - 1000;
        assert!(!verify_session_token("any", past));
    }

    #[test]
    fn verify_session_token_zero_expiry() {
        assert!(!verify_session_token("any", 0));
    }

    #[test]
    fn extract_ip_from_cf_connecting() {
        let mut h = HashMap::new();
        h.insert("cf-connecting-ip".to_string(), "1.2.3.4".to_string());
        let r = extract_client_ip(&h, None, None);
        assert_eq!(r.ip, "1.2.3.4");
        assert_eq!(r.source, "cf-connecting-ip");
    }

    #[test]
    fn extract_ip_from_x_real_ip() {
        let mut h = HashMap::new();
        h.insert("x-real-ip".to_string(), "5.6.7.8".to_string());
        let r = extract_client_ip(&h, None, None);
        assert_eq!(r.ip, "5.6.7.8");
        assert_eq!(r.source, "x-real-ip");
    }

    #[test]
    fn extract_ip_from_x_forwarded_for_first() {
        let mut h = HashMap::new();
        h.insert(
            "x-forwarded-for".to_string(),
            "10.0.0.1, 192.168.1.1, 172.16.0.1".to_string(),
        );
        let r = extract_client_ip(&h, None, None);
        assert_eq!(r.ip, "10.0.0.1");
        assert_eq!(r.source, "x-forwarded-for");
    }

    #[test]
    fn extract_ip_priority_cf_beats_xff() {
        let mut h = HashMap::new();
        h.insert("cf-connecting-ip".to_string(), "1.1.1.1".to_string());
        h.insert("x-forwarded-for".to_string(), "2.2.2.2".to_string());
        let r = extract_client_ip(&h, None, None);
        assert_eq!(r.ip, "1.1.1.1");
        assert_eq!(r.source, "cf-connecting-ip");
    }

    #[test]
    fn extract_ip_case_insensitive() {
        let mut h = HashMap::new();
        h.insert("CF-Connecting-IP".to_string(), "9.9.9.9".to_string());
        let r = extract_client_ip(&h, None, None);
        assert_eq!(r.ip, "9.9.9.9");
    }

    #[test]
    fn extract_ip_from_fallback_when_localhost() {
        // ::1 / ::ffff:127.0.0.1 被过滤
        let h: HashMap<String, String> = HashMap::new();
        let r = extract_client_ip(&h, Some("::1"), None);
        assert_eq!(r.ip, "unknown");
    }

    #[test]
    fn extract_ip_from_fallback_socket_ipv4_mapped() {
        let h: HashMap<String, String> = HashMap::new();
        let r = extract_client_ip(&h, None, Some("::ffff:192.168.1.100"));
        assert_eq!(r.ip, "192.168.1.100");
        assert_eq!(r.source, "socket");
    }

    #[test]
    fn extract_ip_unknown_when_all_missing() {
        let h: HashMap<String, String> = HashMap::new();
        let r = extract_client_ip(&h, None, None);
        assert_eq!(r.ip, "unknown");
    }

    #[test]
    fn rate_limiter_allows_under_limit() {
        let rl = RateLimiter::new(60_000, 3);
        for _ in 0..3 {
            match rl.check("key1") {
                RateLimitDecision::Allowed { .. } => {}
                _ => panic!("should allow"),
            }
        }
    }

    #[test]
    fn rate_limiter_blocks_over_limit() {
        let rl = RateLimiter::new(60_000, 2);
        rl.check("key1");
        rl.check("key1");
        let r = rl.check("key1");
        assert!(matches!(r, RateLimitDecision::Limited { .. }));
    }

    #[test]
    fn rate_limiter_separate_keys() {
        let rl = RateLimiter::new(60_000, 1);
        assert!(matches!(rl.check("a"), RateLimitDecision::Allowed { .. }));
        assert!(matches!(rl.check("b"), RateLimitDecision::Allowed { .. }));
        assert!(matches!(rl.check("a"), RateLimitDecision::Limited { .. }));
    }

    #[test]
    fn rate_limiter_reset_after_window() {
        let rl = RateLimiter::new(50, 1);
        rl.check("k");
        assert!(matches!(rl.check("k"), RateLimitDecision::Limited { .. }));
        // 等过窗口
        std::thread::sleep(std::time::Duration::from_millis(80));
        assert!(matches!(rl.check("k"), RateLimitDecision::Allowed { .. }));
    }

    #[test]
    fn rate_limiter_cleanup_expired() {
        let rl = RateLimiter::new(50, 5);
        rl.check("k");
        std::thread::sleep(std::time::Duration::from_millis(80));
        rl.cleanup_expired();
        // 内部 store 应被清空
        assert!(rl.store.lock().is_empty());
    }

    #[test]
    fn rate_limit_decision_partial_eq() {
        let a = RateLimitDecision::Allowed {
            remaining: 5,
            reset_at: 1000,
        };
        let b = RateLimitDecision::Allowed {
            remaining: 5,
            reset_at: 1000,
        };
        assert_eq!(a, b);
    }
}
