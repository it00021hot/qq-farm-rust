//! TTL、冷却、时区偏移等时间常量。

pub const BEIJING_UTC_OFFSET_SECONDS: i64 = 8 * 60 * 60;
pub const SECONDS_PER_DAY: i64 = 86_400;

pub const HELP_IN_FLIGHT_TTL_MS: u64 = 15_000;
pub const HELP_RESULT_TTL_MS: u64 = 30_000;
pub const HELP_CACHE_MAX: usize = 2048;

pub const DEFAULT_FRIENDS_LIST_CACHE_TTL_MS: u64 = 60_000;
pub const MIN_FRIENDS_LIST_CACHE_TTL_MS: u64 = 10_000;
pub const INVALID_KNOWN_FRIEND_GID_COOLDOWN_MS: u64 = 24 * 60 * 60 * 1000;

pub const FRIEND_LIST_COALESCE_MS: u64 = 800;
/// 好友 LandsNotify 按 gid 去抖，避免连发气泡打满 GetGameFriends。
pub const FRIEND_LANDS_NOTIFY_DEBOUNCE_MS: u64 = 500;
pub const QQ_FRIEND_LIST_BATCH_SIZE: usize = 35;
/// 网关 in-flight 上限（对齐 bot `MAX_IN_FLIGHT_REQUESTS`）；Heartbeat 不受此限。
pub const MAX_IN_FLIGHT_REQUESTS: usize = 5;
/// 网关等待队列上限（对齐 bot `MAX_QUEUED_REQUESTS`）。
pub const MAX_QUEUED_REQUESTS: usize = 100;
/// 活动窗口缓存 TTL（对齐 bot `activity-windows.ts`）。
pub const ACTIVITY_WINDOWS_CACHE_TTL_MS: u64 = 5 * 60 * 1000;
/// 活动窗口刷新失败日志节流。
pub const ACTIVITY_WINDOWS_RETRY_LOG_INTERVAL_MS: u64 = 60 * 1000;
/// 仅 Login / Heartbeat 使用的短超时；其它游戏 RPC 等到回包或断线。
pub const LOGIN_TIMEOUT_MS: u64 = 20_000;
pub const HEARTBEAT_RPC_TIMEOUT_MS: u64 = 20_000;
/// 业务 RPC 默认超时（对齐 bot sendMsgAsync 的 20s 默认值）。
/// 无超时的话服务端漏回一个包就永久占用并发槽，漏 5 个后业务全堵死。
pub const DEFAULT_RPC_TIMEOUT_MS: u64 = 20_000;
/// 业务 RPC 排队（等并发槽）超时，与默认超时同值。
pub const RPC_QUEUE_TIMEOUT_MS: u64 = 20_000;

/// 探测本机微信 `/api/check-login` 超时（对齐 YYB scan.html）
pub const LOCAL_WECHAT_DETECT_TIMEOUT_MS: u64 = 3_000;
/// 本机微信 `/api/authorize` 等待用户确认超时
pub const LOCAL_WECHAT_AUTHORIZE_TIMEOUT_MS: u64 = 120_000;

/// 微信扫码任务默认存活
pub const WX_LOGIN_TASK_TTL_MS: u64 = 110_000;
/// 扫码换出的 code 尚未绑定账号时，应用宝授权暂存时长
pub const WX_LOGIN_PENDING_AUTH_TTL_MS: u64 = 10 * 60 * 1000;
/// 掉线后用应用宝授权换码重连的最大次数
pub const WX_RECONNECT_MAX_ATTEMPTS: u32 = 3;
/// 掉线后首次用应用宝授权换码重连的等待时间。
/// 心跳超时类的"半死"会话（推送还在、RPC 不应答）服务端释放很慢，且高频重登
/// 本身就是风控信号（历史上 60~103 次/天的登录churn 与持续掉线强相关），
/// 因此拉长到 15 分钟，降低单位时间登录频率。
pub const WX_RECONNECT_FIRST_DELAY_MS: u64 = 15 * 60 * 1000;
/// 掉线后第 2～3 次用应用宝授权换码重连的等待时间
pub const WX_RECONNECT_RETRY_DELAY_MS: u64 = 10 * 60 * 1000;
/// 被踢下线（"已在其他终端登录"）后重登等待时间。
/// 服务端旧 session 释放需要时间，重登过快会连环被踢，因此每次都等满 3 分钟。
pub const WX_KICKOUT_RECONNECT_DELAY_MS: u64 = 3 * 60 * 1000;
/// 进程启动后已授权微信账号首次自动重连的等待时间
pub const WX_STARTUP_RECONNECT_DELAY_MS: u64 = 60 * 1000;

/// 应用宝 accesstoken 后台保活检查间隔
pub const WX_KEEPALIVE_INTERVAL_MS: u64 = 30 * 60 * 1000;
/// 剩余不足该秒数时提前续 token（默认 45 分钟）
pub const WX_KEEPALIVE_AHEAD_SECS: i64 = 45 * 60;
/// 同一 refresh_token 连续使用超过该秒数后建议重扫（25 天）
pub const WX_REFRESH_TOKEN_RESCAN_SECS: i64 = 25 * 24 * 60 * 60;

/// 按掉线重连次数返回等待时间。
#[must_use]
pub const fn wx_reconnect_delay_ms(attempt: u32) -> u64 {
    if attempt <= 1 {
        WX_RECONNECT_FIRST_DELAY_MS
    } else {
        WX_RECONNECT_RETRY_DELAY_MS
    }
}

/// 被踢下线的重登等待（固定 3 分钟，不随次数缩短）。
#[must_use]
pub const fn wx_kickout_reconnect_delay_ms() -> u64 {
    WX_KICKOUT_RECONNECT_DELAY_MS
}

/// 运行日志用的掉线重连等待文案。
#[must_use]
pub fn wx_reconnect_delay_zh(attempt: u32) -> String {
    duration_ms_zh(wx_reconnect_delay_ms(attempt))
}

/// 运行日志用的启动重连等待文案。
#[must_use]
pub fn wx_startup_reconnect_delay_zh() -> String {
    duration_ms_zh(WX_STARTUP_RECONNECT_DELAY_MS)
}

/// 被踢下线的重登等待文案。
#[must_use]
pub fn wx_kickout_reconnect_delay_zh() -> String {
    duration_ms_zh(WX_KICKOUT_RECONNECT_DELAY_MS)
}

fn duration_ms_zh(duration_ms: u64) -> String {
    let secs = duration_ms / 1000;
    if secs >= 60 && secs % 60 == 0 {
        format!("{} 分钟", secs / 60)
    } else {
        format!("{secs} 秒")
    }
}

/// 网关心跳（对齐 Go / 原 TS）
pub const HEARTBEAT_INTERVAL_MS: u64 = 25_000;
pub const HEARTBEAT_SILENCE_MS: u64 = 30_000;
/// 状态广播兜底间隔：无业务变化时也至少每 30s 全量广播一次
pub const STATUS_FALLBACK_BROADCAST_MS: u64 = 30_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kickout_reconnect_delay_is_fixed_three_minutes() {
        assert_eq!(wx_kickout_reconnect_delay_ms(), 3 * 60 * 1000);
        assert_eq!(wx_kickout_reconnect_delay_zh(), "3 分钟");
    }

    #[test]
    fn normal_reconnect_delays_unchanged() {
        assert_eq!(wx_reconnect_delay_ms(1), 15 * 60 * 1000);
        assert_eq!(wx_reconnect_delay_ms(2), 10 * 60 * 1000);
        assert_eq!(wx_reconnect_delay_ms(3), 10 * 60 * 1000);
    }
}
