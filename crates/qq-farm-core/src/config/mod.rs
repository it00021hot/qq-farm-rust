//! 配置层。
//!
//! - [`app`] — 应用级配置（端口、日志、数据目录等）
//! - [`game`] — 游戏配置（作物、道具、等级等静态数据）

pub mod app;
pub mod game;

pub use app::AppConfig;
pub use game::GameConfig;
