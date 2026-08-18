//! 登录 URL Profile — 把登录 URL 中的 platform/os/ver 写入全局系统配置。
//!
//! 1:1 翻译原 `core/src/services/login-url-profile.ts`（92 行）。
//!
//! ## 业务
//!
//! 登录 URL 携带的 `platform` / `os` / `ver` 三个 hint 可能不同于当前运行时配置。
//! 本服务把它们合并到 `SystemConfig.deviceInfo` 并热更新 `global_runtime_config`，
//! 避免后续 RPC 用错的 device。

use std::collections::HashMap;

use serde::Serialize;

use crate::config::system_config::{
    get_device_presets, get_runtime_config, update_runtime_config, DeviceInfo, SystemConfig,
};
use crate::utils::login_url::normalize_login_platform;

/// 客户端 hints（从登录 URL 解析）
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LoginClientHints {
    pub platform: Option<String>,
    pub os: Option<String>,
    pub ver: Option<String>,
}

/// 设备预设（精简版）
#[derive(Debug, Clone, Serialize)]
pub struct PresetLite {
    pub name: String,
    pub os: String,
    pub device_info: DeviceInfo,
}

/// 按 os 找匹配的设备预设
#[must_use]
pub fn find_device_preset_by_os(os: &str) -> Option<PresetLite> {
    let needle = os.trim().to_lowercase();
    if needle.is_empty() {
        return None;
    }
    let presets = get_device_presets();
    let aliases: HashMap<&str, Vec<&str>> = [
        ("windows", vec!["windows", "win"]),
        ("ios", vec!["ios", "iphone", "ipad"]),
        ("android", vec!["android"]),
        ("os x", vec!["os x", "osx", "mac", "macos", "mac os", "mac os x"]),
    ]
    .into_iter()
    .collect();

    for preset in presets {
        let preset_os = preset.device_info.os.trim().to_lowercase();
        if preset_os.is_empty() {
            continue;
        }
        if preset_os == needle {
            return Some(PresetLite {
                name: preset.name.to_string(),
                os: preset_os,
                device_info: preset.device_info.clone(),
            });
        }
        for (canon, list) in &aliases {
            if list.contains(&needle.as_str())
                && (preset_os == *canon || list.contains(&preset_os.as_str()))
            {
                return Some(PresetLite {
                    name: preset.name.to_string(),
                    os: preset_os,
                    device_info: preset.device_info.clone(),
                });
            }
        }
    }
    None
}

/// 把登录 URL 解析的 hints 应用到全局系统配置
///
/// 返回 `Some(SystemConfig)` 表示有变更并已应用；`None` 表示 hints 为空
#[must_use]
pub fn apply_login_client_hints_to_system_config(
    hints: Option<&LoginClientHints>,
) -> Option<SystemConfig> {
    let hints = hints?;
    let platform_str = hints.platform.as_deref().map(normalize_login_platform).unwrap_or("");
    let os = hints.os.as_deref().unwrap_or("").trim().to_string();
    let ver = hints.ver.as_deref().unwrap_or("").trim().to_string();
    let has_ver = !ver.is_empty() && is_valid_version(&ver) && ver.len() > 4;

    if platform_str.is_empty() && os.is_empty() && !has_ver {
        return None;
    }

    let current = get_runtime_config();
    let mut current_device = current.device_info.clone();

    if !os.is_empty() {
        if let Some(preset) = find_device_preset_by_os(&os) {
            current_device = DeviceInfo { os: os.clone(), ..preset.device_info };
        } else {
            current_device.os = os.clone();
            if current_device.sys_software.is_empty() {
                current_device.sys_software = os.clone();
            }
        }
    }

    if has_ver {
        current_device.client_version = ver.clone();
    }

    let resolved_os = if !os.is_empty() {
        os.clone()
    } else if !current_device.os.is_empty() {
        current_device.os.clone()
    } else if !current.platform.is_empty() {
        current.platform.clone()
    } else {
        "Windows".to_string()
    };
    let resolved_client_version =
        if has_ver { ver.clone() } else { current_device.client_version.clone() };
    let resolved_platform = if !platform_str.is_empty() {
        platform_str.to_string()
    } else if !current.platform.is_empty() {
        current.platform.clone()
    } else {
        "qq".to_string()
    };
    let resolved_top_version = if has_ver {
        ver.clone()
    } else if !current.client_version.is_empty() {
        current.client_version.clone()
    } else {
        current_device.client_version.clone()
    };

    let next_device =
        DeviceInfo { os: resolved_os, client_version: resolved_client_version, ..current_device };

    let next = SystemConfig {
        server_url: current.server_url.clone(),
        client_version: resolved_top_version,
        platform: resolved_platform,
        os: next_device.os.clone(),
        device_info: next_device,
    };

    update_runtime_config(&next);
    Some(next)
}

/// 验证版本号字符串（仅字母 / 数字 / `.` / `-` / `_`）
#[must_use]
pub fn is_valid_version(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_version_basic() {
        assert!(is_valid_version("1.0.0"));
        assert!(is_valid_version("1.0.0-beta"));
        assert!(is_valid_version("1.13.1.6_20260723"));
        assert!(is_valid_version("v1.2.3-rc1_build42"));
    }

    #[test]
    fn is_valid_version_rejects_invalid() {
        assert!(!is_valid_version(""));
        assert!(!is_valid_version("1.0.0 beta")); // 空格
        assert!(!is_valid_version("1.0.0/"));
        assert!(!is_valid_version("1.0.0#"));
    }

    #[test]
    fn empty_hints_returns_none() {
        let h = LoginClientHints::default();
        let r = apply_login_client_hints_to_system_config(Some(&h));
        // 全空 -> None
        assert!(r.is_none());
    }

    #[test]
    fn none_hints_returns_none() {
        let r = apply_login_client_hints_to_system_config(None);
        assert!(r.is_none());
    }

    #[test]
    fn invalid_version_too_short() {
        // ver="1.0" 长度 3，不满足 > 4
        let h = LoginClientHints { ver: Some("1.0".to_string()), ..Default::default() };
        let r = apply_login_client_hints_to_system_config(Some(&h));
        assert!(r.is_none());
    }

    #[test]
    fn invalid_version_chars() {
        let h = LoginClientHints { ver: Some("1.0.0!".to_string()), ..Default::default() };
        let r = apply_login_client_hints_to_system_config(Some(&h));
        assert!(r.is_none());
    }

    #[test]
    fn platform_only_applies() {
        let h = LoginClientHints { platform: Some("wx".to_string()), ..Default::default() };
        let r = apply_login_client_hints_to_system_config(Some(&h));
        assert!(r.is_some());
        assert_eq!(r.unwrap().platform, "wx");
    }

    #[test]
    fn os_only_applies() {
        let h = LoginClientHints { os: Some("iOS".to_string()), ..Default::default() };
        let r = apply_login_client_hints_to_system_config(Some(&h));
        assert!(r.is_some());
        let cfg = r.unwrap();
        assert_eq!(cfg.device_info.os, "iOS");
    }

    #[test]
    fn os_unknown_no_preset_keeps_sys_software() {
        let h = LoginClientHints { os: Some("Plan 9".to_string()), ..Default::default() };
        let r = apply_login_client_hints_to_system_config(Some(&h));
        // OS 不在预设里也应返回 Some
        assert!(r.is_some());
    }

    #[test]
    fn ver_only_applies() {
        let h =
            LoginClientHints { ver: Some("1.13.1.6_20260723".to_string()), ..Default::default() };
        let r = apply_login_client_hints_to_system_config(Some(&h));
        assert!(r.is_some());
        let cfg = r.unwrap();
        assert_eq!(cfg.device_info.client_version, "1.13.1.6_20260723");
    }

    #[test]
    fn login_client_hints_default() {
        let h = LoginClientHints::default();
        assert!(h.platform.is_none());
        assert!(h.os.is_none());
        assert!(h.ver.is_none());
    }

    #[test]
    fn find_preset_by_os_no_match() {
        let r = find_device_preset_by_os("Plan 9");
        assert!(r.is_none());
    }

    #[test]
    fn find_preset_by_os_empty() {
        let r = find_device_preset_by_os("");
        assert!(r.is_none());
    }

    #[test]
    fn find_preset_by_os_whitespace() {
        let r = find_device_preset_by_os("   ");
        assert!(r.is_none());
    }
}
