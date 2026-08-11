//! 配置层。
//!
//! - [`app`] — 应用全局配置（启动参数、端口、间隔等）
//! - [`game_config`] — 游戏静态数据（植物 / 物品 / 土地 / 等级）
//! - [`system_config`] — 系统配置 + 设备预设 + PlantPhase 枚举
//! - [`paths`] — 运行时路径（资源根 / 数据目录 / 分享文件）

pub mod app;
pub mod game_config;
pub mod paths;
pub mod system_config;

pub use app::AppConfig;
pub use game_config::{GameConfig, SeedInfo};
pub use game_config::global as global_game_config;
pub use paths::{
    ensure_data_dir, get_app_root, get_data_dir, get_data_file, get_resource_path,
    get_resource_root, get_share_file_path, IS_PACKAGED,
};
pub use system_config::{
    device_presets, get_default_system_config, get_device_presets, get_runtime_config,
    global as global_system_config, update_runtime_config, DeviceInfo, DevicePreset, PlantPhase,
    RuntimeConfig, SystemConfig, DEFAULT_CLIENT_VERSION, PHASE_NAMES,
};
