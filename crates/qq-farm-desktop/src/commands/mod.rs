//! Tauri 命令：薄适配 → `qq-farm-app`，无领域逻辑。

mod dto;

pub mod account;
pub mod application;
pub mod commerce;
pub mod config;
pub mod farm;
pub mod friend;
pub mod milk_tea;
pub mod pets;
pub mod settings;
pub mod share_reward;
pub mod snapshot;
pub mod solar_calendar;

#[allow(unused_imports)]
pub use dto::{AccountSummary, DesktopSnapshot};
