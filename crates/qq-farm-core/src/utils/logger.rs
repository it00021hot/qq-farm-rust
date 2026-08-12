//! 模块化日志（1:1 对应原 `services/logger.ts` 180 行）。
//!
//! - `redact_string`：脱敏 code / token / password / cookie 等敏感字段
//! - `sanitize_meta`：递归脱敏对象（含字符串里的 token / `Bearer xxx`）
//! - `create_module_logger`：返回带 `info/warn/error/debug` 的模块 logger
//! - 文件 fallback：写 `<dataDir>/logs/combined.log` + `error.log`
//!
//! Rust 端用 `tracing` crate 作为 root logger（替代 winston），fallback 用
//! `tracing-subscriber` JSON 输出 + 自管文件 append。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;

use parking_lot::Mutex;
use serde::Serialize;

/// 敏感 key（注释保留）
#[allow(dead_code)]
const SENSITIVE_KEY_HINT: &str = "(code|token|password|passwd|auth|ticket|cookie|session)";

/// 脱敏字符串中的 code/token/Bearer（state machine 扫描版）
pub fn redact_string(input: &str) -> String {
    redact_string_v2(input)
}

/// 递归脱敏对象（key 含敏感词 → `[REDACTED]`；string value 走 redact_string）
pub fn sanitize_meta(value: serde_json::Value, depth: usize) -> serde_json::Value {
    if depth > 4 {
        return serde_json::Value::String("[Truncated]".to_string());
    }
    match value {
        serde_json::Value::Null => serde_json::Value::Null,
        serde_json::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_json::Value::Number(n) => serde_json::Value::Number(n),
        serde_json::Value::String(s) => serde_json::Value::String(redact_string(&s)),
        serde_json::Value::Array(arr) => {
            let new_arr: Vec<serde_json::Value> = arr
                .into_iter()
                .map(|v| sanitize_meta(v, depth + 1))
                .collect();
            serde_json::Value::Array(new_arr)
        }
        serde_json::Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                if is_sensitive_key(&k) {
                    new_map.insert(k, serde_json::Value::String("[REDACTED]".to_string()));
                } else {
                    new_map.insert(k, sanitize_meta(v, depth + 1));
                }
            }
            serde_json::Value::Object(new_map)
        }
    }
}

fn is_sensitive_key(k: &str) -> bool {
    let lower = k.to_lowercase();
    ["code", "token", "password", "passwd", "auth", "ticket", "cookie", "session"]
        .iter()
        .any(|kw| lower.contains(kw))
}

/// 真实 redact（state machine 版）
/// redact state machine 扫描（公共 API）
pub fn redact_string_v2(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        // ?code=xxx / &code=xxx 形式
        if (bytes[i] == b'?' || bytes[i] == b'&') && i + 1 < bytes.len() {
            // 试图读 key
            let key_start = i + 1;
            let key_end = scan_ident_end(bytes, key_start);
            if key_end > key_start && key_end < bytes.len() && bytes[key_end] == b'=' {
                let key = &input[key_start..key_end];
                let lower = key.to_ascii_lowercase();
                if matches!(lower.as_str(), "code" | "token" | "ticket" | "password")
                {
                    // 输出 ?key=
                    out.push(bytes[i] as char);
                    out.push_str(key);
                    out.push('=');
                    // 跳过直到 & 或 空白
                    let val_start = key_end + 1;
                    let val_end = scan_value_end(bytes, val_start);
                    // 跳过已经 redact 的
                    if input[val_start..val_end].starts_with("[REDACTED]") {
                        out.push_str(&input[val_start..val_end]);
                    } else {
                        out.push_str("[REDACTED]");
                    }
                    i = val_end;
                    continue;
                }
            }
        }
        // Bearer xxx
        if bytes[i..].starts_with(b"Bearer ") && (i == 0 || !is_token_continue(bytes[i - 1])) {
            out.push_str("Bearer ");
            let val_start = i + "Bearer ".len();
            let val_end = scan_token_end(bytes, val_start);
            if input[val_start..val_end].starts_with("[REDACTED]") {
                out.push_str(&input[val_start..val_end]);
            } else {
                out.push_str("[REDACTED]");
            }
            i = val_end;
            continue;
        }
        // 复制当前字节（UTF-8 安全按 char 边界推进）
        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn is_id_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn scan_ident_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && is_id_char(bytes[i]) {
        i += 1;
    }
    i
}

fn scan_value_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && bytes[i] != b'&' && bytes[i] != b' ' && bytes[i] != b'\n' && bytes[i] != b'\t' {
        i += 1;
    }
    i
}

fn is_token_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.'
}

fn scan_token_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && is_token_continue(bytes[i]) {
        i += 1;
    }
    i
}

#[derive(Serialize, Clone)]
struct LogEntry<'a> {
    ts: String,
    level: &'a str,
    module: &'a str,
    message: String,
    meta: serde_json::Value,
}

static INIT: Once = Once::new();
static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 初始化日志（必须先调一次）
pub fn init() {
    INIT.call_once(|| {
        // tracing-subscriber：标准 error/warn/info/debug 输出
        // 单测中可能重复调用，Once 保证
    });
}

/// 拿到 log 目录（不存在则创建）
pub fn ensure_log_dir() -> PathBuf {
    let mut guard = LOG_DIR.lock();
    if let Some(p) = guard.as_ref() {
        return p.clone();
    }
    let data_dir = crate::config::paths::ensure_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let dir = data_dir.join("logs");
    let _ = fs::create_dir_all(&dir);
    *guard = Some(dir.clone());
    dir
}

/// 模块 logger
#[derive(Clone)]
pub struct ModuleLogger {
    name: String,
}

impl ModuleLogger {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }

    fn write(&self, level: &str, message: &str, meta: serde_json::Value) {
        let safe_msg = redact_string_v2(message);
        let safe_meta = sanitize_meta(meta, 0);
        let entry = LogEntry {
            ts: chrono_like_now_iso(),
            level,
            module: &self.name,
            message: safe_msg,
            meta: safe_meta,
        };
        if let Ok(line) = serde_json::to_string(&entry) {
            append_fallback_log(level, &format!("{line}\n"));
        }
        // tracing 输出
        match level {
            "error" => tracing::error!(module = %self.name, "{}", message),
            "warn" => tracing::warn!(module = %self.name, "{}", message),
            "debug" => tracing::debug!(module = %self.name, "{}", message),
            _ => tracing::info!(module = %self.name, "{}", message),
        }
    }

    pub fn info<M: Serialize>(&self, message: &str, meta: M) {
        let value = serde_json::to_value(meta).unwrap_or(serde_json::Value::Null);
        self.write("info", message, value);
    }

    pub fn warn<M: Serialize>(&self, message: &str, meta: M) {
        let value = serde_json::to_value(meta).unwrap_or(serde_json::Value::Null);
        self.write("warn", message, value);
    }

    pub fn error<M: Serialize>(&self, message: &str, meta: M) {
        let value = serde_json::to_value(meta).unwrap_or(serde_json::Value::Null);
        self.write("error", message, value);
    }

    pub fn debug<M: Serialize>(&self, message: &str, meta: M) {
        let value = serde_json::to_value(meta).unwrap_or(serde_json::Value::Null);
        self.write("debug", message, value);
    }
}

/// 创建模块 logger
pub fn create_module_logger(name: &str) -> ModuleLogger {
    ModuleLogger::new(name)
}

/// 文件 fallback 写日志（parking_lot 串行化以避免并发文件锁竞争）
fn append_fallback_log(level: &str, line: &str) {
    let _guard = log_append_lock().lock();
    let dir = ensure_log_dir();
    if let Ok(mut combined) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("combined.log"))
    {
        let _ = combined.write_all(line.as_bytes());
    }
    if level == "error" {
        if let Ok(mut err) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("error.log"))
        {
            let _ = err.write_all(line.as_bytes());
        }
    }
}

/// 串行化 file append 的全局锁（避免多线程写同一文件时被 fs 锁卡住）
fn log_append_lock() -> &'static Mutex<()> {
    use std::sync::OnceLock;
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn chrono_like_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_unix_to_iso8601(now)
}

fn format_unix_to_iso8601(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let h = secs_of_day / 3_600;
    let m = (secs_of_day % 3_600) / 60;
    let s = secs_of_day % 60;
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z",)
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

/// 把 log 写到指定路径（测试用）
pub fn write_to_path(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redact_query_string() {
        let out = redact_string_v2("?code=abc123&other=ok");
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("abc123"));
    }

    #[test]
    fn redact_bearer() {
        let out = redact_string_v2("Authorization: Bearer abc.def-ghi");
        assert!(out.contains("Bearer [REDACTED]"));
        assert!(!out.contains("abc.def-ghi"));
    }

    #[test]
    fn sanitize_meta_key() {
        let v = json!({
            "username": "alice",
            "password": "secret",
            "auth_token": "tok",
        });
        let out = sanitize_meta(v, 0);
        assert_eq!(out["username"], "alice");
        assert_eq!(out["password"], "[REDACTED]");
        assert_eq!(out["auth_token"], "[REDACTED]");
    }

    #[test]
    fn sanitize_meta_nested() {
        let v = json!({
            "data": {
                "code": "123",
                "user": { "token": "abc" }
            }
        });
        let out = sanitize_meta(v, 0);
        assert_eq!(out["data"]["code"], "[REDACTED]");
        assert_eq!(out["data"]["user"]["token"], "[REDACTED]");
    }

    #[test]
    fn create_module_logger_basic() {
        let l = create_module_logger("test");
        l.info("hello", json!({ "k": "v" }));
        l.warn("warning", json!({ "code": "secret" }));
        l.error("error", json!({ "auth": "x" }));
    }

    #[test]
    fn redact_no_match() {
        let out = redact_string_v2("just a normal message");
        assert_eq!(out, "just a normal message");
    }
}
