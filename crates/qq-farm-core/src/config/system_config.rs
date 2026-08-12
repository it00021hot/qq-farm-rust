//! 系统配置 + 设备预设。
//!
//! 1:1 翻译原 `core/src/config/config.ts`。
//!
//! - `DeviceInfo` / `DevicePreset` / `SystemConfig` / `RuntimeConfig`
//! - 6 个设备预设（Windows PC / iPhone 15 Pro / iPhone 16 Pro / 小米 / 华为 / iPad Pro）
//! - `PlantPhase` 枚举（8 阶段）
//! - 全局可读写 CONFIG（Mutex 保护）
//! - `update_runtime_config` / `get_runtime_config` / `get_device_presets`

use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// 默认客户端版本（与原 TS DEFAULT_CLIENT_VERSION 一致）
pub const DEFAULT_CLIENT_VERSION: &str = "1.13.0.5_20260723";

/// 生长阶段枚举（与原 TS PlantPhase 1:1）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlantPhase {
    Unknown = 0,
    Seed = 1,
    Germination = 2,
    SmallLeaves = 3,
    LargeLeaves = 4,
    Blooming = 5,
    Mature = 6,
    Dead = 7,
}

/// 阶段中文名
pub const PHASE_NAMES: [&str; 8] = ["未知", "种子", "发芽", "小叶", "大叶", "开花", "成熟", "枯死"];

impl PlantPhase {
    /// 从 i32 转换（容错）
    #[must_use]
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Seed,
            2 => Self::Germination,
            3 => Self::SmallLeaves,
            4 => Self::LargeLeaves,
            5 => Self::Blooming,
            6 => Self::Mature,
            7 => Self::Dead,
            _ => Self::Unknown,
        }
    }

    /// 中文名
    #[must_use]
    pub fn name(self) -> &'static str {
        PHASE_NAMES[self as usize]
    }
}

// =====================================================================
// DeviceInfo
// =====================================================================

/// 设备信息（发到游戏服务器的标识）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub os: String,
    pub client_version: String,
    pub sys_software: String,
    pub network: String,
    pub memory: String,
    pub device_id: String,
    pub user_agent: String,
}

impl DeviceInfo {
    /// 构造 Windows PC 默认
    #[must_use]
    pub fn windows_pc() -> Self {
        Self {
            os: "Windows".to_string(),
            client_version: String::new(),
            sys_software: "Windows".to_string(),
            network: "wifi".to_string(),
            memory: "16384".to_string(),
            device_id: "DESKTOP-PC<WPC>".to_string(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 MicroMessenger/7.0.20.1781(0x6700143B) NetType/WIFI MiniProgramEnv/Windows WindowsWechat/WMPF WindowsWechat(0x63090a13)".to_string(),
        }
    }

    /// 构造 iPhone 15 Pro 默认
    #[must_use]
    pub fn iphone_15_pro() -> Self {
        Self {
            os: "iOS".to_string(),
            client_version: String::new(),
            sys_software: "iOS 17.4.1".to_string(),
            network: "wifi".to_string(),
            memory: "7672".to_string(),
            device_id: "iPhone15,2".to_string(),
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 MicroMessenger/8.0.47(0x18002f2c) NetType/WIFI Language/zh_CN".to_string(),
        }
    }

    /// 构造 iPhone 16 Pro 默认
    #[must_use]
    pub fn iphone_16_pro() -> Self {
        Self {
            os: "iOS".to_string(),
            client_version: String::new(),
            sys_software: "iOS 18.2.1".to_string(),
            network: "wifi".to_string(),
            memory: "8192".to_string(),
            device_id: "iPhone17,1".to_string(),
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_2_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/22C161 MicroMessenger/8.0.54(0x1800362c) NetType/WIFI Language/zh_CN".to_string(),
        }
    }

    /// 构造 Android 小米默认
    #[must_use]
    pub fn android_xiaomi() -> Self {
        Self {
            os: "Android".to_string(),
            client_version: String::new(),
            sys_software: "Android 14".to_string(),
            network: "wifi".to_string(),
            memory: "8192".to_string(),
            device_id: "Xiaomi 14".to_string(),
            user_agent: "Mozilla/5.0 (Linux; Android 14; 23127PN0CC Build/UKQ1.231003.002) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/116.0.0.0 Mobile Safari/537.36 XWEB/1165009 MMWEBSDK/20240407 MiniProgramEnv/android MicroMessenger/8.0.49.2680(0x28003137) NetType/WIFI Language/zh_CN ABI/arm64".to_string(),
        }
    }

    /// 构造 Android 华为默认
    #[must_use]
    pub fn android_huawei() -> Self {
        Self {
            os: "Android".to_string(),
            client_version: String::new(),
            sys_software: "Android 14".to_string(),
            network: "wifi".to_string(),
            memory: "12288".to_string(),
            device_id: "HUAWEI Mate 60".to_string(),
            user_agent: "Mozilla/5.0 (Linux; Android 14; ALN-AL10 Build/HUAWEIALN-AL10) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/116.0.0.0 Mobile Safari/537.36 XWEB/1165009 MMWEBSDK/20240407 MiniProgramEnv/android MicroMessenger/8.0.49.2680(0x28003137) NetType/WIFI Language/zh_CN ABI/arm64".to_string(),
        }
    }

    /// 构造 iPad Pro 默认
    #[must_use]
    pub fn ipad_pro() -> Self {
        Self {
            os: "iOS".to_string(),
            client_version: String::new(),
            sys_software: "iPadOS 17.4".to_string(),
            network: "wifi".to_string(),
            memory: "16384".to_string(),
            device_id: "iPad14,6".to_string(),
            user_agent: "Mozilla/5.0 (iPad; CPU OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 MicroMessenger/8.0.47(0x18002f2c) NetType/WIFI Language/zh_CN".to_string(),
        }
    }
}

// =====================================================================
// DevicePreset
// =====================================================================

/// 设备预设（前端可选）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DevicePreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub device_info: DeviceInfo,
}

/// 全部设备预设（顺序与原 TS DEVICE_PRESETS 一致）
#[must_use]
pub fn device_presets() -> Vec<DevicePreset> {
    vec![
        DevicePreset {
            id: "windows_pc",
            name: "Windows PC",
            description: "Windows 微信PC客户端",
            device_info: DeviceInfo::windows_pc(),
        },
        DevicePreset {
            id: "iphone_15_pro",
            name: "iPhone 15 Pro",
            description: "iPhone 15 Pro (iOS 17)",
            device_info: DeviceInfo::iphone_15_pro(),
        },
        DevicePreset {
            id: "iphone_16_pro",
            name: "iPhone 16 Pro",
            description: "iPhone 16 Pro (iOS 18)",
            device_info: DeviceInfo::iphone_16_pro(),
        },
        DevicePreset {
            id: "android_xiaomi",
            name: "小米手机",
            description: "小米/Redmi (Android 14)",
            device_info: DeviceInfo::android_xiaomi(),
        },
        DevicePreset {
            id: "android_huawei",
            name: "华为手机",
            description: "华为 (Android 14)",
            device_info: DeviceInfo::android_huawei(),
        },
        DevicePreset {
            id: "ipad_pro",
            name: "iPad Pro",
            description: "iPad Pro 12.9 (iPadOS 17)",
            device_info: DeviceInfo::ipad_pro(),
        },
    ]
}

// =====================================================================
// SystemConfig / RuntimeConfig
// =====================================================================

/// 系统配置（用户可见部分）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub server_url: String,
    pub client_version: String,
    pub platform: String,
    pub os: String,
    pub device_info: DeviceInfo,
}

impl SystemConfig {
    /// 默认系统配置
    #[must_use]
    pub fn default_with_device(device: DeviceInfo) -> Self {
        let mut d = device;
        if d.client_version.is_empty() {
            d.client_version = DEFAULT_CLIENT_VERSION.to_string();
        }
        let os = d.os.clone();
        let client_version = d.client_version.clone();
        Self {
            server_url: "wss://gate-obt.nqf.qq.com/prod/ws".to_string(),
            client_version,
            platform: "qq".to_string(),
            os,
            device_info: d,
        }
    }

    /// 完全默认
    #[must_use]
    pub fn default_system() -> Self {
        Self::default_with_device(DeviceInfo::windows_pc())
    }
}

/// 运行时配置（SystemConfig + 内部参数）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub server_url: String,
    pub client_version: String,
    pub platform: String,
    pub os: String,
    pub device_info: DeviceInfo,
    /// 心跳间隔（毫秒）
    pub heartbeat_interval_ms: i64,
    /// 农场检查间隔（毫秒）
    pub farm_check_interval_ms: i64,
    /// 好友检查间隔（毫秒）
    pub friend_check_interval_ms: i64,
    /// 农场检查间隔范围（毫秒）
    pub farm_check_interval_min_ms: i64,
    pub farm_check_interval_max_ms: i64,
    /// 好友检查间隔范围（毫秒）
    pub friend_check_interval_min_ms: i64,
    pub friend_check_interval_max_ms: i64,
    /// 管理端口
    pub admin_port: u16,
    /// 管理员密码（None 表示未设）
    pub admin_password: Option<String>,
}

impl RuntimeConfig {
    /// 默认运行时配置
    #[must_use]
    pub fn default_runtime() -> Self {
        let sys = SystemConfig::default_system();
        let admin_port = std::env::var("ADMIN_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3007);
        let admin_password = std::env::var("ADMIN_PASSWORD").ok();
        Self {
            server_url: sys.server_url,
            client_version: sys.client_version,
            platform: sys.platform,
            os: sys.os,
            device_info: sys.device_info,
            heartbeat_interval_ms: 25_000,
            farm_check_interval_ms: 3_000,
            friend_check_interval_ms: 12_000,
            farm_check_interval_min_ms: 3_000,
            farm_check_interval_max_ms: 5_000,
            friend_check_interval_min_ms: 12_000,
            friend_check_interval_max_ms: 15_000,
            admin_port,
            admin_password,
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::default_runtime()
    }
}

// =====================================================================
// 全局 CONFIG（可读写，Mutex 保护）
// =====================================================================

static GLOBAL_CONFIG: once_cell::sync::Lazy<Arc<RwLock<RuntimeConfig>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(RuntimeConfig::default_runtime())));

/// 获取全局 CONFIG 引用
#[must_use]
pub fn global() -> Arc<RwLock<RuntimeConfig>> {
    Arc::clone(&GLOBAL_CONFIG)
}

/// 更新运行时配置（部分覆盖）
pub fn update_runtime_config(new: &SystemConfig) {
    let mut guard = GLOBAL_CONFIG.write();
    guard.server_url = new.server_url.clone();
    guard.client_version = new.client_version.clone();
    guard.platform = new.platform.clone();
    guard.os = new.os.clone();
    let mut dev = new.device_info.clone();
    if dev.client_version.is_empty() {
        dev.client_version = guard.client_version.clone();
    }
    guard.device_info = dev;
    // 同步 os 与 client_version 到顶层
    guard.os = guard.device_info.os.clone();
    guard.client_version = guard.device_info.client_version.clone();
}

/// 获取运行时配置快照
#[must_use]
pub fn get_runtime_config() -> RuntimeConfig {
    GLOBAL_CONFIG.read().clone()
}

/// 获取默认系统配置
#[must_use]
pub fn get_default_system_config() -> SystemConfig {
    SystemConfig::default_system()
}

/// 获取设备预设列表（带当前 clientVersion）
#[must_use]
pub fn get_device_presets() -> Vec<DevicePreset> {
    let cv = GLOBAL_CONFIG.read().client_version.clone();
    device_presets()
        .into_iter()
        .map(|mut p| {
            if p.device_info.client_version.is_empty() {
                p.device_info.client_version = cv.clone();
            }
            p
        })
        .collect()
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn plant_phase_from_i32_and_name() {
        assert_eq!(PlantPhase::from_i32(0), PlantPhase::Unknown);
        assert_eq!(PlantPhase::from_i32(1), PlantPhase::Seed);
        assert_eq!(PlantPhase::from_i32(6), PlantPhase::Mature);
        assert_eq!(PlantPhase::from_i32(99), PlantPhase::Unknown);

        assert_eq!(PlantPhase::Seed.name(), "种子");
        assert_eq!(PlantPhase::Mature.name(), "成熟");
        assert_eq!(PlantPhase::Dead.name(), "枯死");
    }

    #[test]
    fn device_presets_count_and_ids() {
        let presets = device_presets();
        assert_eq!(presets.len(), 6);
        let ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
        assert!(ids.contains(&"windows_pc"));
        assert!(ids.contains(&"iphone_15_pro"));
        assert!(ids.contains(&"android_xiaomi"));
    }

    #[test]
    fn device_presets_unique_ids() {
        let presets = device_presets();
        let mut ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), presets.len(), "duplicate preset id");
    }

    #[test]
    fn system_config_default_has_client_version() {
        let sys = SystemConfig::default_system();
        assert_eq!(sys.client_version, DEFAULT_CLIENT_VERSION);
        assert_eq!(sys.server_url, "wss://gate-obt.nqf.qq.com/prod/ws");
        assert_eq!(sys.platform, "qq");
    }

    #[test]
    fn runtime_config_default_env_override() {
        // 不设环境变量，应使用默认 3007
        let r = RuntimeConfig::default_runtime();
        assert_eq!(r.admin_port, 3007);
        assert!(r.admin_password.is_none());
        assert_eq!(r.heartbeat_interval_ms, 25_000);
    }

    #[test]
    #[serial(global_config)]
    fn global_config_update() {
        let g = global();
        let original = g.read().server_url.clone();
        let new_sys = SystemConfig {
            server_url: "wss://test.example.com/ws".to_string(),
            client_version: "v1".to_string(),
            platform: "wx".to_string(),
            os: "Android".to_string(),
            device_info: DeviceInfo::android_xiaomi(),
        };
        update_runtime_config(&new_sys);
        assert_eq!(g.read().server_url, "wss://test.example.com/ws");
        assert_eq!(g.read().platform, "wx");
        assert_eq!(g.read().os, "Android");
        // 恢复
        let mut restore = new_sys;
        restore.server_url = original;
        update_runtime_config(&restore);
    }

    #[test]
    #[serial(global_config)]
    fn get_device_presets_inherits_client_version() {
        let presets = get_device_presets();
        let cv = global().read().client_version.clone();
        for p in &presets {
            assert_eq!(p.device_info.client_version, cv);
        }
    }
}
