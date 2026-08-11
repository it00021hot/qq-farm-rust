//! 业务服务层。
//!
//! - [`json_db`] — 原子 JSON 文件读写
//! - [`rate_limiter`] — 令牌桶 + 优先级队列 + 服务队列
//! - [`farm`] — 农场服务（1C 阶段：1:1 翻译原 TS services/farm/）
//! - [`friend`] — 好友服务（1D 阶段：1:1 翻译原 TS services/friend/）

pub mod farm;
pub mod friend;
pub mod json_db;
pub mod rate_limiter;

pub use json_db::{
    ensure_parent_dir, file_exists, file_size, list_files_with_ext, read_json_or,
    read_json_with_default, read_text_file, write_json_file_atomic, write_text_file_atomic,
};
pub use rate_limiter::{
    get_farm_optimizer, get_friend_optimizer, get_service_config, get_service_queue,
    PriorityQueue, QueueStatus, RateLimiterConfig, RequestQueue, ServiceConfig, TaskEntry,
    TokenBucket,
};
