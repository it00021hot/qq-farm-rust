//! 时间工具。
//!
//! 1:1 翻译原 `core/src/utils/utils.ts` 中的时间相关函数。
//!
//! - `server_time` 全局状态：用于和服务器时间对齐
//! - `to_time_sec` 归一化（毫秒/秒自动判断）
//! - `now_ms` / `now_secs`：本地时间

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

static SERVER_TIME_MS: AtomicI64 = AtomicI64::new(0);
static LOCAL_TIME_AT_SYNC: AtomicI64 = AtomicI64::new(0);
/// 标记是否已同步（避免启动时 get_server_time_ms 退化为本地时间）
static SYNCED: AtomicU64 = AtomicU64::new(0);

static SYNC_MUTEX: Mutex<()> = Mutex::new(());

/// 当前本地时间（毫秒）
#[must_use]
pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// 当前本地时间（秒）
#[must_use]
pub fn now_secs() -> i64 {
    now_ms() / 1000
}

/// 推算的服务器时间（毫秒）。未同步时返回本地时间。
#[must_use]
pub fn get_server_time_ms() -> i64 {
    if SYNCED.load(Ordering::Acquire) == 0 {
        return now_ms();
    }
    let server = SERVER_TIME_MS.load(Ordering::Acquire);
    let local_at_sync = LOCAL_TIME_AT_SYNC.load(Ordering::Acquire);
    let elapsed = now_ms() - local_at_sync;
    server + elapsed
}

/// 推算的服务器时间（秒）
#[must_use]
pub fn get_server_time_secs() -> i64 {
    get_server_time_ms() / 1000
}

/// 同步服务器时间（毫秒）
pub fn sync_server_time(server_ms: i64) {
    let _guard = SYNC_MUTEX.lock();
    SERVER_TIME_MS.store(server_ms, Ordering::Release);
    LOCAL_TIME_AT_SYNC.store(now_ms(), Ordering::Release);
    SYNCED.store(1, Ordering::Release);
}

/// 同步服务器时间（秒）
pub fn sync_server_time_secs(server_secs: i64) {
    sync_server_time(server_secs * 1000);
}

/// 是否已同步
#[must_use]
pub fn is_server_time_synced() -> bool {
    SYNCED.load(Ordering::Acquire) != 0
}

/// 时间戳归一化（毫秒/秒自动判断）。
///
/// - 0 / 负数 → 0
/// - > 1e12 → 毫秒，转秒
/// - 其它 → 当作秒
#[must_use]
pub fn to_time_secs(val: i64) -> i64 {
    if val <= 0 {
        return 0;
    }
    if val > 1_000_000_000_000 {
        val / 1000
    } else {
        val
    }
}

/// 格式化为 HH:MM
#[must_use]
pub fn format_hhmm(secs: i64) -> String {
    let total = secs.rem_euclid(86_400);
    let h = total / 3600;
    let m = (total % 3600) / 60;
    format!("{h:02}:{m:02}")
}

/// 判断当前时间是否在 [start, end) 区间内（HH:MM 字符串）
///
/// 区间跨午夜也算。
#[must_use]
pub fn is_in_time_window(now_secs: i64, start_hhmm: &str, end_hhmm: &str) -> bool {
    let Some((start_sec, end_sec)) = parse_window(start_hhmm, end_hhmm) else {
        return false;
    };
    let cur = now_secs.rem_euclid(86_400);
    if start_sec <= end_sec {
        cur >= start_sec && cur < end_sec
    } else {
        // 跨午夜（如 22:00 - 06:00）
        cur >= start_sec || cur < end_sec
    }
}

fn parse_window(start: &str, end: &str) -> Option<(i64, i64)> {
    let s = parse_hhmm(start)?;
    let e = parse_hhmm(end)?;
    Some((s, e))
}

fn parse_hhmm(s: &str) -> Option<i64> {
    let (h, m) = s.split_once(':')?;
    let h: i64 = h.trim().parse().ok()?;
    let m: i64 = m.trim().parse().ok()?;
    if !(0..24).contains(&h) || !(0..60).contains(&m) {
        return None;
    }
    Some(h * 3600 + m * 60)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ms_reasonable() {
        let n = now_ms();
        // 2026-08-11 ≈ 1776000000000 ms，远大于此值
        assert!(n > 1_700_000_000_000, "n={n}");
    }

    #[test]
    fn to_time_secs_handles_mixed_units() {
        assert_eq!(to_time_secs(0), 0);
        assert_eq!(to_time_secs(-1), 0);
        assert_eq!(to_time_secs(100), 100); // 秒
        assert_eq!(to_time_secs(1_776_000_000), 1_776_000_000); // 秒
        assert_eq!(to_time_secs(1_776_000_000_000), 1_776_000_000); // 毫秒 → 秒
        assert_eq!(to_time_secs(60_000), 60_000); // 60_000 < 1e12 当作秒
        assert_eq!(to_time_secs(1_700_000_000_000), 1_700_000_000); // 毫秒 → 秒
    }

    #[test]
    fn server_time_default_to_local_when_not_synced() {
        // 确保未同步时返回本地
        let n = get_server_time_ms();
        let local = now_ms();
        // 误差应该 < 100ms
        assert!((n - local).abs() < 100, "diff={}", (n - local).abs());
    }

    #[test]
    fn server_time_sync_and_read() {
        let target = 1_700_000_000_000_i64;
        sync_server_time(target);
        assert!(is_server_time_synced());
        // 推算的服务器时间应该接近 target
        let n = get_server_time_ms();
        assert!((n - target).abs() < 100, "diff={}", (n - target).abs());
    }

    #[test]
    fn format_hhmm_basic() {
        assert_eq!(format_hhmm(0), "00:00");
        assert_eq!(format_hhmm(3600), "01:00");
        assert_eq!(format_hhmm(3661), "01:01");
        assert_eq!(format_hhmm(86399), "23:59");
    }

    #[test]
    fn is_in_time_window_same_day() {
        // 10:00 - 12:00
        let t = 10 * 3600 + 30 * 60; // 10:30
        assert!(is_in_time_window(t, "10:00", "12:00"));
        assert!(!is_in_time_window(9 * 3600, "10:00", "12:00"));
        assert!(!is_in_time_window(12 * 3600, "10:00", "12:00"));
    }

    #[test]
    fn is_in_time_window_cross_midnight() {
        // 22:00 - 06:00 跨午夜
        assert!(is_in_time_window(23 * 3600, "22:00", "06:00"));
        assert!(is_in_time_window(2 * 3600, "22:00", "06:00"));
        assert!(!is_in_time_window(10 * 3600, "22:00", "06:00"));
        assert!(!is_in_time_window(7 * 3600, "22:00", "06:00"));
    }

    #[test]
    fn is_in_time_window_invalid_returns_false() {
        assert!(!is_in_time_window(0, "bad", "06:00"));
        assert!(!is_in_time_window(0, "25:00", "26:00"));
    }
}
