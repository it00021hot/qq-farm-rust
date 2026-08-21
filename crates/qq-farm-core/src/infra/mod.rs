//! 基础设施层 — JSON 持久化、限流、自动化开关、面板日志、统计与状态。
//!
//! 自 `services/` 迁出，供业务服务与 runtime 共用。

pub mod automation;
pub mod fs_async;
pub mod json_db;
pub mod panel_log;
pub mod rate_limiter;
pub mod stats;
pub mod status;

pub use fs_async::{spawn_blocking, spawn_blocking_detached, spawn_write_file};
pub use json_db::{
    ensure_parent_dir, file_exists, file_size, list_files_with_ext, read_json_or,
    read_json_with_default, read_text_file, write_json_file_atomic, write_text_file_atomic,
};
pub use rate_limiter::{
    get_farm_optimizer, get_friend_optimizer, get_service_config, get_service_queue, PriorityQueue,
    QueueStatus, RateLimiterConfig, RequestQueue, ServiceConfig, TaskEntry, TokenBucket,
};
