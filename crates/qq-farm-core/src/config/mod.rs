//! 配置层。
//!
//! - [`app`] — 应用级配置（端口、日志、数据目录等）
//! - [`game_config`] — 游戏静态数据加载与查询（1:1 翻译原 TS `core/src/gameConfig/`）

pub mod app;
pub mod game_config;

pub use app::AppConfig;
pub use game_config::{GameConfig, SeedInfo};
