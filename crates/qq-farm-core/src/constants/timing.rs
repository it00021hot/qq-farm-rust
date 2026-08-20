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
/// 掉线后首次用应用宝授权换码重连的等待时间
pub const WX_RECONNECT_FIRST_DELAY_MS: u64 = 3 * 60 * 1000;
/// 掉线后第 2～3 次用应用宝授权换码重连的等待时间
pub const WX_RECONNECT_RETRY_DELAY_MS: u64 = 60 * 1000;
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
