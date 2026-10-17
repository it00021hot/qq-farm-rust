use super::*;

#[test]
fn navigation_icon_catalog_is_embedded_in_the_application_bundle() {
    use gpui_kit::AssetSource as _;
    for page in crate::pages::Page::ALL {
        let path = page.icon().path();
        let svg = gpui_kit::assets::AllAssets.load(&path).expect("navigation icon is embedded");
        assert!(svg.is_some(), "{path}");
    }
}
#[test]
fn resolves_existing_game_assets_and_rejects_traversal() {
    assert!(resolve_local_asset("/game-config/seed_images_named/seed_images/20218.webp").is_ok());
    assert!(matches!(
        resolve_local_asset("/game-config/../Cargo.toml"),
        Err(AssetError::InvalidPath)
    ));
    assert!(matches!(
        resolve_local_asset("https://example.com/image.png"),
        Err(AssetError::UnsupportedUrl)
    ));
}

#[test]
fn activity_assets_resolve_in_the_source_checkout() {
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(
        local::resolve_local_asset_at(&checkout, "/activity-assets/milk-tea/recipe-1.png").is_ok()
    );
    assert!(local::resolve_local_asset_at(&checkout, "/activity-assets/autumn/happy.png").is_ok());
    assert!(matches!(
        local::resolve_local_asset_at(&checkout, "/activity-assets/../Cargo.toml"),
        Err(AssetError::InvalidPath)
    ));
}

#[test]
fn adapter_preserves_remote_resources_and_converts_inline_bytes() {
    assert!(matches!(
        resolve_image_source("https://example.com/image.png"),
        Ok(ImageSource::Resource(_))
    ));
    let Ok(ImageSource::Image(image)) = resolve_image_source("data:image/png;base64,AQID") else {
        panic!("inline image bytes");
    };
    assert_eq!(image.bytes, [1, 2, 3]);
    assert!(matches!(
        resolve_image_source("file:///tmp/image.png"),
        Err(AssetError::UnsupportedUrl)
    ));
}

#[test]
fn adapter_maps_legacy_farmcfg_urls_to_packaged_game_resources() {
    assert!(matches!(
        resolve_image_source("farmcfg://localhost/seed_images_named/seed_images/20218.webp"),
        Ok(ImageSource::Resource(_))
    ));
    assert!(matches!(
        resolve_image_source(
            "farmcfg://localhost/game-config/seed_images_named/seed_images/20218.webp"
        ),
        Ok(ImageSource::Resource(_))
    ));
    assert!(matches!(
        resolve_image_source("farmcfg://localhost/../Cargo.toml"),
        Err(AssetError::InvalidPath)
    ));
}
