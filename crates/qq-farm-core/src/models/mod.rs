//! 领域模型。
//!
//! - [`account`] — 账号（QQ/微信登录凭据 + 运行时状态）
//! - [`land`] — 土地运行时数据（proto 形态）
//! - [`friend`] — 好友（用于好友互动）
//! - [`types`] — 跨模块共享类型（按需提取；非 1:1 全搬原 `types/`）

pub mod account;
pub mod friend;
pub mod land;
pub mod types;

pub use account::Account;
pub use friend::Friend;
pub use land::Land;
