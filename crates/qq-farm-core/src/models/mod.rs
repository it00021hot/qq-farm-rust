//! 领域模型。
//!
//! - [`account`] — 账号（QQ/微信登录凭据 + 运行时状态）
//! - [`land`] — 土地（农场内一块地的运行时数据）
//! - [`friend`] — 好友（用于好友互动）

pub mod account;
pub mod friend;
pub mod land;

pub use account::Account;
pub use friend::Friend;
pub use land::Land;
