//! Tauri 命令：薄适配 → `qq-farm-app`，无领域逻辑。

mod dto;

pub mod account;
pub mod activity;
pub mod commerce;
pub mod config;
pub mod farm;
pub mod friend;
pub mod pets;
pub mod qq_login;
pub mod settings;
pub mod snapshot;
pub mod weather;

#[allow(unused_imports)]
pub use dto::{AccountSummary, DesktopSnapshot};
