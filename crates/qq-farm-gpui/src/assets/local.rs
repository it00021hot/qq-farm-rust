use super::AssetError;
use std::path::{Component, Path, PathBuf};

/// Resolve a game catalog or activity image to a canonical local asset.
pub fn resolve_local_asset(url: &str) -> Result<PathBuf, AssetError> {
    resolve_local_asset_at(&resource_root(), url)
}

pub(super) fn resolve_local_asset_at(resources: &Path, url: &str) -> Result<PathBuf, AssetError> {
    let (root, relative) = if let Some(relative) = url.strip_prefix("/game-config/") {
        (resources.join("assets/game_config"), relative)
    } else if let Some(relative) = url.strip_prefix("/activity-assets/") {
        (
            {
                let packaged = resources.join("activity-assets");
                if packaged.is_dir() {
                    packaged
                } else {
                    resources.join("desktop-ui/public/activity-assets")
                }
            },
            relative,
        )
    } else if let Some(relative) = url.strip_prefix("/land-assets/") {
        // 土地等级图（bot e5f3552 web/public/land-assets；桌面端打包或回退前端 public）
        (
            {
                let packaged = resources.join("land-assets");
                if packaged.is_dir() {
                    packaged
                } else {
                    resources.join("desktop-ui/public/land-assets")
                }
            },
            relative,
        )
    } else {
        return Err(AssetError::UnsupportedUrl);
    };
    if Path::new(relative).components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err(AssetError::InvalidPath);
    }
    let root = root.canonicalize().map_err(|_| AssetError::Missing)?;
    let path = root.join(relative).canonicalize().map_err(|_| AssetError::Missing)?;
    path.starts_with(&root).then_some(path).ok_or(AssetError::InvalidPath)
}

pub fn resource_root() -> PathBuf {
    if let Some(path) = std::env::var_os("QQ_FARM_ASSET_DIR") {
        return PathBuf::from(path);
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(binary_dir) = executable.parent() {
            #[cfg(target_os = "macos")]
            {
                let bundle_resources = binary_dir.join("../Resources");
                if bundle_resources.join("assets/game_config").is_dir() {
                    return bundle_resources;
                }
            }
            if binary_dir.join("assets/game_config").is_dir() {
                return binary_dir.to_path_buf();
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}
