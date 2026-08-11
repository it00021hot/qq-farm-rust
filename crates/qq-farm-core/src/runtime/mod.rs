//! 运行时引擎。
//!
//! ## 模块
//!
//! - [`engine`] — 顶层 [`RuntimeEngine`]，管理所有 worker
//! - [`worker`] — 单账号 [`Worker`]（tokio task 跑挂机逻辑）
//! - [`worker_handle`] — 外部控制句柄 [`WorkerHandle`]
//! - [`worker_message`] — 控制消息 [`WorkerMessage`]
//! - [`scheduler`] — 任务调度 [`Scheduler`]
//! - [`events`] — 生命周期事件 [`WorkerEvent`]

pub mod engine;
pub mod events;
pub mod scheduler;
pub mod worker;
pub mod worker_handle;
pub mod worker_message;
