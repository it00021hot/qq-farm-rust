//! 好友安静时段配置与判断。

pub use crate::services::friend::runtime_state::FriendQuietHours;

use crate::services::friend::runtime_state;

/// 测试 / 无账号上下文时使用的全局 shim 键
pub const GLOBAL_QUIET_HOURS_ACCOUNT: &str = "";

/// 解析 "HH:MM" 格式为分钟数（0-1439）；无效返回 None
#[must_use]
pub fn parse_time_to_minutes(time_str: &str) -> Option<u32> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let h: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

/// 设置账号 quiet hours（线程安全）
pub fn set_friend_quiet_hours(account_id: &str, cfg: Option<FriendQuietHours>) {
    runtime_state::set_friend_quiet_hours(account_id, cfg);
}

/// 当前是否在好友安静时段（测试 / 未指定账号时读全局 shim）
#[must_use]
pub fn in_friend_quiet_hours(now_hhmm: Option<(u32, u32)>) -> bool {
    in_friend_quiet_hours_for(None, now_hhmm)
}

/// 按账号配置判断安静时段（对齐 TS `getFriendQuietHours(accountId)`）
#[must_use]
pub fn in_friend_quiet_hours_for(account_id: Option<&str>, now_hhmm: Option<(u32, u32)>) -> bool {
    let cfg = if let Some(id) = account_id.filter(|s| !s.is_empty()) {
        let snap = crate::models::store::account_config::get_friend_quiet_hours(Some(id));
        if !snap.enabled {
            return false;
        }
        FriendQuietHours {
            enabled: true,
            start: snap.start,
            end: snap.end,
        }
    } else {
        let cfg = runtime_state::get_friend_quiet_hours(GLOBAL_QUIET_HOURS_ACCOUNT);
        match cfg {
            Some(c) if c.enabled => c,
            _ => return false,
        }
    };
    let (h, m) = now_hhmm.unwrap_or_else(|| {
        let t = chrono::Local::now();
        (
            t.format("%H").to_string().parse().unwrap_or(0),
            t.format("%M").to_string().parse().unwrap_or(0),
        )
    });
    let cur = h * 60 + m;
    let start = match parse_time_to_minutes(&cfg.start) {
        Some(s) => s,
        None => return false,
    };
    let end = match parse_time_to_minutes(&cfg.end) {
        Some(e) => e,
        None => return false,
    };
    if start == end {
        return true;
    }
    if start < end {
        cur >= start && cur < end
    } else {
        cur >= start || cur < end
    }
}
