use base64::{engine::general_purpose::STANDARD, Engine as _};
use gpui_kit::{Image, ImageFormat};

use super::AssetError;

pub(super) fn decode(raw: &str) -> Result<Image, AssetError> {
    let (header, encoded) = raw
        .strip_prefix("data:")
        .and_then(|raw| raw.split_once(','))
        .ok_or(AssetError::InvalidImageData)?;
    let mut parameters = header.split(';');
    let format = match parameters.next().unwrap_or_default().to_lowercase().as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
        "image/webp" => ImageFormat::Webp,
        "image/gif" => ImageFormat::Gif,
        "image/svg+xml" => ImageFormat::Svg,
        "image/bmp" => ImageFormat::Bmp,
        "image/tiff" => ImageFormat::Tiff,
        "image/ico" | "image/x-icon" | "image/vnd.microsoft.icon" => ImageFormat::Ico,
        _ => return Err(AssetError::UnsupportedUrl),
    };
    let bytes = percent_decode(encoded)?;
    let bytes = if parameters.any(|value| value.eq_ignore_ascii_case("base64")) {
        STANDARD.decode(bytes).map_err(|_| AssetError::InvalidImageData)?
    } else {
        bytes
    };
    if bytes.is_empty() {
        return Err(AssetError::InvalidImageData);
    }
    Ok(Image::from_bytes(format, bytes))
}

fn percent_decode(encoded: &str) -> Result<Vec<u8>, AssetError> {
    let mut output = Vec::with_capacity(encoded.len());
    let mut bytes = encoded.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next().and_then(|byte| char::from(byte).to_digit(16));
            let low = bytes.next().and_then(|byte| char::from(byte).to_digit(16));
            match (high, low) {
                (Some(high), Some(low)) => output.push((high * 16 + low) as u8),
                _ => return Err(AssetError::InvalidImageData),
            }
        } else {
            output.push(byte);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_images_accept_base64_and_percent_encoded_svg_without_changing_plus() {
        let png = decode("data:image/png;base64,AQID").expect("base64 PNG bytes");
        assert_eq!(png.format, ImageFormat::Png);
        assert_eq!(png.bytes, [1, 2, 3]);
        let svg = decode("data:image/svg+xml;charset=utf-8,%3Csvg%3E+%3C/svg%3E")
            .expect("percent encoded SVG");
        assert_eq!(svg.format, ImageFormat::Svg);
        assert_eq!(svg.bytes, b"<svg>+</svg>");
    }

    #[test]
    fn invalid_inline_data_produces_the_missing_image_state() {
        for raw in ["data:image/png;base64,?", "data:image/png,", "data:image/png,%1"] {
            assert!(matches!(decode(raw), Err(AssetError::InvalidImageData)));
        }
        assert!(matches!(decode("data:text/html,<html>"), Err(AssetError::UnsupportedUrl)));
    }
}
