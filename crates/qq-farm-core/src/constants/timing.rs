//! TTL、冷却、时区偏移等时间常量。

pub const BEIJING_UTC_OFFSET_SECONDS: i64 = 8 * 60 * 60;
pub const SECONDS_PER_DAY: i64 = 86_400;

pub const HELP_IN_FLIGHT_TTL_MS: u64 = 15_000;
pub const HELP_RESULT_TTL_MS: u64 = 30_000;
pub const HELP_CACHE_MAX: usize = 2048;

pub const DEFAULT_FRIENDS_LIST_CACHE_TTL_MS: u64 = 60_000;
pub const MIN_FRIENDS_LIST_CACHE_TTL_MS: u64 = 10_000;
pub const INVALID_KNOWN_FRIEND_GID_COOLDOWN_MS: u64 = 24 * 60 * 60 * 1000;

pub const DEFAULT_TIMEOUT_MS: u64 = 20_000;
/// 微信 GetAll 回包带头像/图鉴，20s 经常读不完；对齐「大包等完整帧」而不是空等超时。
pub const FRIEND_GET_ALL_TIMEOUT_MS: u64 = 60_000;
pub const FRIEND_LIST_COALESCE_MS: u64 = 800;
pub const QQ_FRIEND_LIST_BATCH_SIZE: usize = 35;
/// 普通 RPC 排队上限；Heartbeat 不受此限（Go 无 QueueFull）。
pub const MAX_PENDING_RPC: usize = 32;

/// 微信扫码任务默认存活
pub const WX_LOGIN_TASK_TTL_MS: u64 = 110_000;
/// 扫码换出的 code 尚未绑定账号时，应用宝授权暂存时长
pub const WX_LOGIN_PENDING_AUTH_TTL_MS: u64 = 10 * 60 * 1000;
/// 掉线后用应用宝授权换码重连的最大次数
pub const WX_RECONNECT_MAX_ATTEMPTS: u32 = 3;
/// 换码重连等待（踢号 / 断线 / 进程重启后均先等再连）
pub const WX_RECONNECT_DELAY_MS: u64 = 5 * 60 * 1000;

/// 运行日志用的等待文案（「5 分钟」）。
#[must_use]
pub fn wx_reconnect_delay_zh() -> String {
    let secs = WX_RECONNECT_DELAY_MS / 1000;
    if secs >= 60 && secs % 60 == 0 {
        format!("{} 分钟", secs / 60)
    } else {
        format!("{secs} 秒")
    }
}

/// 网关心跳（对齐 Go / 原 TS）
pub const HEARTBEAT_INTERVAL_MS: u64 = 25_000;
pub const HEARTBEAT_SILENCE_MS: u64 = 30_000;
