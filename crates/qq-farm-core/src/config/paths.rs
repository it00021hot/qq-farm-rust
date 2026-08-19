//! 运行时路径工具。
//!
//! 1:1 翻译原 `core/src/config/runtime-paths.ts`（去 node:fs / path，
//! 用 std::path + std::fs + env var 替代）。
//!
//! - `is_packaged` 编译期 flag
//! - 资源根 / 数据目录 / 分享文件路径
//! - `ensure_data_dir` 自动创建

use std::fs;
use std::path::{Path, PathBuf};

/// 是否打包（编译期决定）
pub const IS_PACKAGED: bool = cfg!(feature = "packaged");

/// 获取资源根目录
///
/// Rust 中对应：
/// - 编译时：CARGO_MANIFEST_DIR（`crates/qq-farm-core/`）
/// - 打包时（feature = "packaged"）：可执行文件目录
#[must_use]
pub fn get_resource_root() -> PathBuf {
    if IS_PACKAGED {
        return std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
    }
    // 开发模式：workspace 根目录（CARGO_MANIFEST_DIR 上一级）
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let crate_dir = Path::new(manifest_dir);
    crate_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| crate_dir.to_path_buf())
}

/// 获取资源路径（资源根 + segments）
#[must_use]
pub fn get_resource_path(segments: &[&str]) -> PathBuf {
    let mut p = get_resource_root();
    for s in segments {
        p.push(s);
    }
    p
}

/// 获取可写应用根目录
#[must_use]
pub fn get_app_root() -> PathBuf {
    if IS_PACKAGED {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        Path::new(manifest_dir)
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// 获取数据目录
///
/// 优先 FARM_DATA_DIR 环境变量；否则 `<app_root>/data`
#[must_use]
pub fn get_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FARM_DATA_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    get_app_root().join("data")
}

/// 确保数据目录存在，返回目录路径
pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let dir = get_data_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// 获取数据目录下某文件路径
#[must_use]
pub fn get_data_file(filename: &str) -> PathBuf {
    get_data_dir().join(filename)
}

/// 分享文件路径（项目根）
#[must_use]
pub fn get_share_file_path() -> PathBuf {
    get_app_root().join("share.txt")
}

/// 面板 `/game-config` 静态目录（对齐原 `express.static(gameConfig)`）。
///
/// 默认只用本仓 `assets/game_config`；可用 `FARM_GAME_CONFIG_DIR` 覆盖。
#[must_use]
pub fn game_config_static_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FARM_GAME_CONFIG_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    get_resource_path(&["assets", "game_config"])
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use super::*;

    #[test]
    fn get_resource_root_returns_path() {
        let root = get_resource_root();
        assert!(!root.as_os_str().is_empty());
    }

    #[test]
    #[serial(farm_data_dir)]
    fn get_data_dir_ends_with_data() {
        let d = get_data_dir();
        assert_eq!(d.file_name().and_then(|s| s.to_str()), Some("data"));
    }

    #[test]
    #[serial(farm_data_dir)]
    fn ensure_data_dir_creates() {
        let tmp = std::env::temp_dir().join(format!("qq-farm-ensure-data-{}", std::process::id()));
        let prev = std::env::var("FARM_DATA_DIR").ok();
        std::env::set_var("FARM_DATA_DIR", &tmp);
        let _ = fs::remove_dir_all(&tmp);
        let created = ensure_data_dir().expect("ensure");
        assert!(created.exists());
        assert!(created.is_dir());
        let again = ensure_data_dir().expect("ensure again");
        assert_eq!(again, created);
        let _ = fs::remove_dir_all(&tmp);
        match prev {
            Some(v) => std::env::set_var("FARM_DATA_DIR", v),
            None => std::env::remove_var("FARM_DATA_DIR"),
        }
    }

    #[test]
    fn get_data_file_appends() {
        let p = get_data_file("accounts.json");
        assert_eq!(p.file_name().and_then(|s| s.to_str()), Some("accounts.json"));
    }

    #[test]
    fn get_share_file_path_appends_share() {
        let p = get_share_file_path();
        assert!(p.ends_with("share.txt"));
    }

    #[test]
    fn get_resource_path_concatenates() {
        let p = get_resource_path(&["assets", "tsdk.wasm"]);
        assert!(p.ends_with("assets/tsdk.wasm"));
    }

    #[test]
    fn game_config_static_dir_resolves() {
        let p = game_config_static_dir();
        assert!(p.ends_with("assets/game_config") || p.ends_with("assets\\game_config"));
        assert!(
            p.join("seed_images_named").is_dir(),
            "seed_images_named should be vendored in-repo"
        );
    }
}
