//! 好友服务模块。
//!
//! - [`api`] — 底层好友 API（GetFriends / VisitFarm / AcceptApplication；Enter 回包顺手写宠物缓存）
//! - [`gid_manager`] — 好友 GID 缓存管理
//! - [`pet_cache`] — 好友护主犬按天缓存（三态 + 写透落盘）
//! - [`pet_sync`] — 好友宠物每日同步（自适应节奏轮次链）
//! - [`visit_strategy`] — 访问策略（帮 / 偷 / 巡 / 统一巡查 visit_plan + combined）
//! - [`scheduler`] — 调度循环（统一巡查 check_friends_unified + 仅帮/仅偷路径）

pub mod api;
pub mod gid_manager;
pub mod pet_cache;
pub mod pet_sync;
pub mod runtime_state;
pub mod scheduler;
pub mod visit_strategy;

pub use runtime_state::{FriendQuietHours, FriendRuntimeState, FriendsListCache};
