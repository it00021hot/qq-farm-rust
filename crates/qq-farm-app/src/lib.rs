//! # qq-farm-app
//!
//! UI 无关的应用门面层，供 server / desktop / CLI 共享业务语义。
//!
//! - [`session::AppContext`] — 持有 `RuntimeEngine`
//! - [`accounts`] — ACL 与账号生命周期
//! - [`farm`] — 农场操作编排
//! - [`events::AppEvent`] — 运行时事件总线

pub mod accounts;
pub mod activity;
pub mod auth;
pub mod commerce;
pub mod error;
pub mod events;
pub mod farm;
pub mod friend;
pub mod session;

pub use error::{AppError, AppResult};
pub use session::AppContext;
