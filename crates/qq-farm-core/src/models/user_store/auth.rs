//! 鉴权 + 密码哈希 + 登录尝试记录。
//!
//! 1:1 翻译原 `core/src/models/user-store/auth.ts`（316 行）。
//!
//! ## 密码哈希
//!
//! - 标准格式：`$pbkdf2$<salt>$<iterations>$<hash>`（PBKDF2-SHA512，1:1 对齐原 TS `security.ts`）
//! - 兼容格式：`salt:hash`（早期 Rust 移植格式）+ 纯 SHA256（老格式）
//!
//! ## 登录限流
//!
//! - 单 IP 60s 内最多 10 次
//! - 单账号 5 次失败后锁定 5 分钟（对齐 TS `lockoutDuration: 300000`）

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};

use crate::config::paths::get_data_file;

const SALT_LENGTH: usize = 32;
const ITERATIONS: u32 = 100_000;
const KEY_LENGTH: usize = 64;

const MAX_LOGIN_ATTEMPTS: u32 = 5;
const LOCKOUT_DURATION_MS: i64 = 5 * 60 * 1000;
const RATE_LIMIT_WINDOW_MS: i64 = 60 * 1000;
const MAX_ATTEMPTS_PER_IP: u32 = 10;

const MAX_LOGS: usize = 1000;

// =====================================================================
// 文件路径
// =====================================================================

#[must_use]
pub fn login_attempts_file() -> PathBuf {
    get_data_file("login-attempts.json")
}

#[must_use]
pub fn login_logs_file() -> PathBuf {
    get_data_file("login-logs.json")
}

// =====================================================================
// 类型
// =====================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginAttempt {
    pub count: u32,
    pub window_start: Option<i64>,
    pub first_attempt: Option<i64>,
    pub last_attempt: Option<i64>,
    pub locked_until: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginLogEntry {
    pub id: String,
    pub timestamp: i64,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining_ms: Option<i64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LockoutResult {
    pub locked: bool,
    pub remaining_ms: Option<i64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FailedAttemptResult {
    pub locked: bool,
    pub message: Option<String>,
    pub remaining_attempts: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct PasswordStrengthResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

// =====================================================================
// 全局状态
// =====================================================================

static LOGIN_ATTEMPTS: once_cell::sync::Lazy<parking_lot::RwLock<HashMap<String, LoginAttempt>>> =
    once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(HashMap::new()));

static LOGIN_LOGS: once_cell::sync::Lazy<parking_lot::RwLock<Vec<LoginLogEntry>>> =
    once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(Vec::new()));

// =====================================================================
// 加载 / 保存
// =====================================================================

pub fn load_login_attempts() {
    let path = login_attempts_file();
    if !path.exists() {
        return;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => return,
    };
    let data: HashMap<String, LoginAttempt> = serde_json::from_str(&raw).unwrap_or_default();
    *LOGIN_ATTEMPTS.write() = data;
}

pub fn save_login_attempts() {
    let _ = crate::config::paths::ensure_data_dir();
    let data = LOGIN_ATTEMPTS.read().clone();
    if let Ok(body) = serde_json::to_string_pretty(&data) {
        let path = login_attempts_file();
        let tmp = path.with_extension("json.tmp");
        let _ = fs::write(&tmp, body);
        let _ = fs::rename(&tmp, &path);
    }
}

pub fn load_login_logs() {
    let path = login_logs_file();
    if !path.exists() {
        return;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => return,
    };
    let data: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
    let logs = data.get("logs").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| serde_json::from_value::<LoginLogEntry>(v.clone()).ok())
            .collect()
    });
    *LOGIN_LOGS.write() = logs.unwrap_or_default();
}

pub fn save_login_logs() {
    let _ = crate::config::paths::ensure_data_dir();
    let logs = LOGIN_LOGS.read().clone();
    let to_save: Vec<_> = logs.into_iter().rev().take(MAX_LOGS).collect::<Vec<_>>().into_iter().rev().collect();
    let body = serde_json::json!({ "logs": to_save });
    if let Ok(s) = serde_json::to_string_pretty(&body) {
        let path = login_logs_file();
        let tmp = path.with_extension("json.tmp");
        let _ = fs::write(&tmp, s);
        let _ = fs::rename(&tmp, &path);
    }
}

// =====================================================================
// 登录日志
// =====================================================================

/// 添加登录日志
pub fn add_login_log(entry: serde_json::Value) -> LoginLogEntry {
    load_login_logs();
    let id = format!(
        "{}-{}",
        crate::utils::time::now_ms(),
        random_id_suffix()
    );
    let log_entry = LoginLogEntry {
        id,
        timestamp: crate::utils::time::now_secs(),
        extra: entry,
    };
    let mut logs = LOGIN_LOGS.write();
    logs.push(log_entry.clone());
    if logs.len() > MAX_LOGS {
        let drop_n = logs.len() - MAX_LOGS;
        logs.drain(0..drop_n);
    }
    drop(logs);
    save_login_logs();
    log_entry
}

/// 获取登录日志
#[must_use]
pub fn get_login_logs(limit: usize, offset: usize) -> (Vec<LoginLogEntry>, usize) {
    load_login_logs();
    let logs = LOGIN_LOGS.read();
    let mut sorted: Vec<LoginLogEntry> = logs.clone();
    sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    let total = sorted.len();
    let end = (offset + limit).min(total);
    let sliced = if offset < sorted.len() {
        sorted[offset..end].to_vec()
    } else {
        vec![]
    };
    (sliced, total)
}

/// 清空登录日志
pub fn clear_login_logs() {
    LOGIN_LOGS.write().clear();
    save_login_logs();
}

fn random_id_suffix() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..9)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

// =====================================================================
// 限流 / 锁定
// =====================================================================

fn clean_expired_attempts() {
    let now = crate::utils::time::now_ms();
    let mut guard = LOGIN_ATTEMPTS.write();
    let before = guard.len();
    guard.retain(|_, attempt| {
        if let Some(until) = attempt.locked_until {
            if until < now {
                return false;
            }
        }
        if let Some(start) = attempt.window_start {
            if (now - start) > RATE_LIMIT_WINDOW_MS {
                return false;
            }
        }
        true
    });
    let _ = before;
}

/// 重置所有登录尝试（E2E 测试用）
pub fn reset_all_login_attempts() {
    LOGIN_ATTEMPTS.write().clear();
}

/// IP 速率限制
pub fn check_rate_limit(ip: &str) -> RateLimitResult {
    clean_expired_attempts();
    let ip_key = format!("ip:{ip}");
    let now = crate::utils::time::now_ms();
    let mut guard = LOGIN_ATTEMPTS.write();

    if !guard.contains_key(&ip_key) {
        guard.insert(
            ip_key.clone(),
            LoginAttempt {
                count: 1,
                window_start: Some(now),
                ..Default::default()
            },
        );
        drop(guard);
        save_login_attempts();
        return RateLimitResult {
            allowed: true,
            remaining_ms: None,
            message: None,
        };
    }

    let attempt = guard.get(&ip_key).cloned().unwrap();
    let window_start = attempt.window_start.unwrap_or(now);
    if (now - window_start) > RATE_LIMIT_WINDOW_MS {
        guard.insert(
            ip_key,
            LoginAttempt {
                count: 1,
                window_start: Some(now),
                ..Default::default()
            },
        );
        drop(guard);
        save_login_attempts();
        return RateLimitResult {
            allowed: true,
            remaining_ms: None,
            message: None,
        };
    }

    if attempt.count >= MAX_ATTEMPTS_PER_IP {
        let remaining = RATE_LIMIT_WINDOW_MS - (now - window_start);
        let secs = (remaining + 999) / 1000;
        return RateLimitResult {
            allowed: false,
            remaining_ms: Some(remaining),
            message: Some(format!("请求过于频繁，请 {secs} 秒后重试")),
        };
    }

    if let Some(a) = guard.get_mut(&ip_key) {
        a.count += 1;
    }
    drop(guard);
    save_login_attempts();
    RateLimitResult {
        allowed: true,
        remaining_ms: None,
        message: None,
    }
}

/// 账号锁定检查
pub fn check_account_lockout(username: &str) -> LockoutResult {
    clean_expired_attempts();
    let user_key = format!("user:{username}");
    let now = crate::utils::time::now_ms();

    let mut guard = LOGIN_ATTEMPTS.write();
    if let Some(attempt) = guard.get(&user_key).cloned() {
        if let Some(until) = attempt.locked_until {
            if until > now {
                let remaining = until - now;
                let minutes = (remaining + 60_000 - 1) / 60_000;
                return LockoutResult {
                    locked: true,
                    remaining_ms: Some(remaining),
                    message: Some(format!("账户已被锁定，请 {minutes} 分钟后重试")),
                };
            }
            // 已过期，删
            guard.remove(&user_key);
            drop(guard);
            save_login_attempts();
        }
    }
    LockoutResult {
        locked: false,
        remaining_ms: None,
        message: None,
    }
}

/// 记录一次失败尝试
pub fn record_failed_attempt(username: &str) -> FailedAttemptResult {
    let user_key = format!("user:{username}");
    let now = crate::utils::time::now_ms();

    let mut guard = LOGIN_ATTEMPTS.write();
    let attempt = guard.entry(user_key.clone()).or_insert(LoginAttempt::default());
    if attempt.count == 0 {
        attempt.first_attempt = Some(now);
    }
    attempt.count += 1;
    attempt.last_attempt = Some(now);

    if attempt.count >= MAX_LOGIN_ATTEMPTS {
        attempt.locked_until = Some(now + LOCKOUT_DURATION_MS);
        let minutes = LOCKOUT_DURATION_MS / 60_000;
        drop(guard);
        save_login_attempts();
        return FailedAttemptResult {
            locked: true,
            message: Some(format!("登录失败次数过多，账户已被锁定 {minutes} 分钟")),
            remaining_attempts: None,
        };
    }

    let remaining = MAX_LOGIN_ATTEMPTS - attempt.count;
    drop(guard);
    save_login_attempts();
    FailedAttemptResult {
        locked: false,
        message: None,
        remaining_attempts: Some(remaining),
    }
}

/// 清除失败计数
pub fn clear_failed_attempts(username: &str) {
    let user_key = format!("user:{username}");
    let mut guard = LOGIN_ATTEMPTS.write();
    if guard.remove(&user_key).is_some() {
        drop(guard);
        save_login_attempts();
    }
}

// =====================================================================
// 密码强度 + 哈希
// =====================================================================

/// 校验密码强度
pub fn validate_password_strength(password: &str) -> PasswordStrengthResult {
    let mut errors = Vec::new();

    if password.len() < 6 {
        errors.push("密码长度至少6位".to_string());
    }
    if password.len() > 128 {
        errors.push("密码长度不能超过128位".to_string());
    }

    let mut type_count = 0;
    if password.chars().any(|c| c.is_ascii_lowercase()) {
        type_count += 1;
    }
    if password.chars().any(|c| c.is_ascii_uppercase()) {
        type_count += 1;
    }
    if password.chars().any(|c| c.is_ascii_digit()) {
        type_count += 1;
    }
    if password
        .chars()
        .any(|c| "!@#$%^&*(),.?\":{}|<>_-+=[]\\;'/`~".contains(c))
    {
        type_count += 1;
    }
    if type_count < 2 {
        errors.push("密码必须包含大写字母、小写字母、数字、特殊符号中的至少两种".to_string());
    }

    const COMMON: &[&str] = &[
        "password", "123456", "qwerty", "abc123", "111111", "000000",
    ];
    if COMMON.contains(&password.to_ascii_lowercase().as_str()) {
        errors.push("密码过于简单，请使用更复杂的密码".to_string());
    }

    PasswordStrengthResult {
        valid: errors.is_empty(),
        errors,
    }
}

/// 生成密码 hash（PBKDF2-SHA512，`$pbkdf2$<salt>$<iterations>$<hash>`，对齐原 TS）
#[must_use]
pub fn hash_password(password: &str, salt: Option<&str>) -> String {
    let salt_owned;
    let salt = if let Some(s) = salt {
        s
    } else {
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..SALT_LENGTH).map(|_| rng.gen()).collect();
        salt_owned = hex_encode(&bytes);
        &salt_owned
    };

    let mut hasher = Pbkdf2Hasher::new(salt);
    let hash = hasher.hash(password);
    format!("$pbkdf2${salt}${ITERATIONS}${hash}")
}

/// 验证密码
#[must_use]
pub fn verify_password(password: &str, stored: &str) -> bool {
    if stored.starts_with("$pbkdf2$") {
        // 标准格式：$pbkdf2$<salt>$<iterations>$<hash>
        let parts: Vec<&str> = stored.split('$').collect();
        if parts.len() != 5 {
            return false;
        }
        let salt = parts[2];
        let iterations: u32 = parts[3].parse().unwrap_or(0);
        let expected = parts[4];
        let mut hasher = Pbkdf2Hasher::new(salt);
        let actual = hasher.hash_with_iterations(password, iterations);
        return constant_time_eq(expected.as_bytes(), actual.as_bytes());
    }
    if stored.contains(':') {
        // 兼容早期 Rust 格式：salt:hash
        let mut parts = stored.splitn(2, ':');
        let salt = parts.next().unwrap_or("");
        let expected = parts.next().unwrap_or("");
        let mut hasher = Pbkdf2Hasher::new(salt);
        let actual = hasher.hash(password);
        return constant_time_eq(expected.as_bytes(), actual.as_bytes());
    }
    // 老格式：纯 SHA256
    if stored.len() == 64 {
        let legacy = {
            let mut h = Sha256::new();
            h.update(password.as_bytes());
            hex_encode(&h.finalize())
        };
        return constant_time_eq(stored.as_bytes(), legacy.as_bytes());
    }
    false
}

/// 是否需要 rehash（非标准 `$pbkdf2$` 格式）
#[must_use]
pub fn needs_rehash(stored: &str) -> bool {
    !stored.starts_with("$pbkdf2$")
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// 简单 PBKDF2 包装（避免直接拉 pbkdf2 crate）
struct Pbkdf2Hasher<'a> {
    salt: &'a str,
}

impl<'a> Pbkdf2Hasher<'a> {
    fn new(salt: &'a str) -> Self {
        Self { salt }
    }

    fn hash(&mut self, password: &str) -> String {
        pbkdf2_sha512(password.as_bytes(), self.salt.as_bytes(), ITERATIONS, KEY_LENGTH)
    }

    fn hash_with_iterations(&mut self, password: &str, iterations: u32) -> String {
        pbkdf2_sha512(password.as_bytes(), self.salt.as_bytes(), iterations, KEY_LENGTH)
    }
}

fn pbkdf2_sha512(password: &[u8], salt: &[u8], iterations: u32, key_len: usize) -> String {
    let mut okm = [0u8; 64];
    pbkdf2::pbkdf2_hmac::<Sha512>(password, salt, iterations, &mut okm);
    hex_encode(&okm[..key_len.min(okm.len())])
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn password_strength_too_short() {
        let r = validate_password_strength("abc");
        assert!(!r.valid);
        assert!(r.errors.iter().any(|e| e.contains("6")));
    }

    #[test]
    fn password_strength_no_type_diversity() {
        let r = validate_password_strength("abcdef");
        assert!(!r.valid);
    }

    #[test]
    fn password_strength_too_common() {
        let r = validate_password_strength("password");
        assert!(!r.valid);
    }

    #[test]
    fn password_strength_valid() {
        let r = validate_password_strength("Strong1Pass");
        assert!(r.valid, "errors = {:?}", r.errors);
    }

    #[test]
    fn hash_and_verify_password() {
        let h = hash_password("hello123", None);
        assert!(h.starts_with("$pbkdf2$"));
        assert!(verify_password("hello123", &h));
        assert!(!verify_password("wrong", &h));
    }

    #[test]
    fn hash_with_salt_deterministic() {
        let h1 = hash_password("test", Some("salt123"));
        let h2 = hash_password("test", Some("salt123"));
        assert_eq!(h1, h2);
        assert!(verify_password("test", &h1));
    }

    #[test]
    fn verify_legacy_salt_colon_format() {
        // 兼容早期 Rust 格式：salt:hash
        let mut hasher = Pbkdf2Hasher::new("salt123");
        let hash = hasher.hash("admin");
        let stored = format!("salt123:{hash}");
        assert!(verify_password("admin", &stored));
        assert!(!verify_password("wrong", &stored));
    }

    #[test]
    fn verify_legacy_sha256() {
        // 模拟老格式：纯 SHA256
        let mut h = Sha256::new();
        h.update(b"admin");
        let legacy = hex_encode(&h.finalize());
        assert!(verify_password("admin", &legacy));
        assert!(!verify_password("wrong", &legacy));
    }

    #[test]
    fn needs_rehash_logic() {
        assert!(needs_rehash("plainhash"));
        let h = hash_password("test", None);
        assert!(!needs_rehash(&h));
    }

    #[test]
    #[serial(user_store)]
    fn rate_limit_first_call_allowed() {
        // 清空状态
        LOGIN_ATTEMPTS.write().clear();
        let _ = fs::remove_file(login_attempts_file());
        let r = check_rate_limit("127.0.0.1");
        assert!(r.allowed);
    }

    #[test]
    #[serial(user_store)]
    fn rate_limit_blocks_after_max() {
        LOGIN_ATTEMPTS.write().clear();
        let _ = fs::remove_file(login_attempts_file());
        for _ in 0..MAX_ATTEMPTS_PER_IP {
            let r = check_rate_limit("10.0.0.1");
            assert!(r.allowed);
        }
        let r = check_rate_limit("10.0.0.1");
        assert!(!r.allowed);
        assert!(r.message.is_some());
    }

    #[test]
    #[serial(user_store)]
    fn account_lockout_after_max_attempts() {
        LOGIN_ATTEMPTS.write().clear();
        let _ = fs::remove_file(login_attempts_file());
        for _ in 0..MAX_LOGIN_ATTEMPTS {
            record_failed_attempt("alice");
        }
        let r = check_account_lockout("alice");
        assert!(r.locked);
    }

    #[test]
    #[serial(user_store)]
    fn clear_failed_attempts_unlocks() {
        LOGIN_ATTEMPTS.write().clear();
        let _ = fs::remove_file(login_attempts_file());
        for _ in 0..MAX_LOGIN_ATTEMPTS {
            record_failed_attempt("bob");
        }
        assert!(check_account_lockout("bob").locked);
        clear_failed_attempts("bob");
        assert!(!check_account_lockout("bob").locked);
    }

    #[test]
    #[serial(user_store)]
    fn add_login_log_keeps_recent() {
        LOGIN_LOGS.write().clear();
        let _ = fs::remove_file(login_logs_file());
        for i in 0..5 {
            add_login_log(serde_json::json!({"i": i}));
        }
        let (logs, total) = get_login_logs(100, 0);
        assert_eq!(total, 5);
        assert_eq!(logs.len(), 5);
        // 清理
        let _ = fs::remove_file(login_logs_file());
    }
}
