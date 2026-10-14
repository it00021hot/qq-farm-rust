mod data_url;
mod local;

use gpui_kit::{Image, ImageFormat, ImageSource};
use std::sync::Arc;

pub use local::{resolve_local_asset, resource_root};

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("unsupported asset URL")]
    UnsupportedUrl,
    #[error("invalid image data URL")]
    InvalidImageData,
    #[error("asset could not be resolved")]
    Missing,
    #[error("asset path is outside its resource directory")]
    InvalidPath,
}

/// Adapt the Vue image protocol to GPUI resources without changing backend DTOs.
pub fn resolve_image_source(raw: &str) -> Result<ImageSource, AssetError> {
    if raw.starts_with("https://") || raw.starts_with("http://") {
        return Ok(raw.to_string().into());
    }
    if raw.starts_with("data:") {
        return data_url::decode(raw).map(|image| Arc::new(image).into());
    }
    if let Some(path) = raw.strip_prefix("farmcfg://localhost/") {
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let relative = path.strip_prefix("game-config/").unwrap_or(path);
        if relative.is_empty() {
            return Err(AssetError::InvalidPath);
        }
        return resolve_local_asset(&format!("/game-config/{relative}")).map(Into::into);
    }
    if let Some(name) = raw.strip_prefix("/api/config/images/") {
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(AssetError::InvalidPath);
        }
        let format = match name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).as_deref()
        {
            Some("png") => ImageFormat::Png,
            Some("jpg" | "jpeg") => ImageFormat::Jpeg,
            Some("webp") => ImageFormat::Webp,
            _ => return Err(AssetError::UnsupportedUrl),
        };
        let bytes = qq_farm_app::config::read_image(name).ok_or(AssetError::Missing)?;
        return Ok(Arc::new(Image::from_bytes(format, bytes)).into());
    }
    let url = if raw.starts_with("/game-config/") || raw.starts_with("/activity-assets/") {
        raw.to_string()
    } else if raw.contains("://") {
        return Err(AssetError::UnsupportedUrl);
    } else {
        format!("/game-config/{}", raw.trim_start_matches('/'))
    };
    resolve_local_asset(&url).map(Into::into)
}

#[cfg(test)]
mod tests;
