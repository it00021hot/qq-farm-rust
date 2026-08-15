//! 好友服务模块。
//!
//! - [`api`] — 底层好友 API（GetFriends / VisitFarm / AcceptApplication）
//! - [`gid_manager`] — 好友 GID 缓存管理
//! - [`visit_strategy`] — 访问策略（帮 / 偷 / 巡）
//! - [`scheduler`] — 调度循环（巡好友 + 帮 + 偷）

pub mod api;
pub mod gid_manager;
pub mod scheduler;
pub mod runtime_state;
pub mod visit_strategy;

pub use runtime_state::{FriendQuietHours, FriendRuntimeState, FriendsListCache};
