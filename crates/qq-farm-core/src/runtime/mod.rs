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
//! - [`runtime_state`] — 全局日志 / 账号日志 / 配置版本号 / 事件总线
//! - [`relogin_reminder`] — 离线提醒 + 重登录监听

pub mod engine;
pub mod events;
pub mod relogin_reminder;
pub mod runtime_state;
pub mod scheduler;
pub mod worker;
pub mod worker_handle;
pub mod worker_loop;
pub mod worker_message;
