use super::support::open_test_desktop;
use gpui_kit::{ImageSource, TestAppContext};

#[gpui_kit::test]
fn uploaded_game_config_images_can_be_resolved_by_the_vue_compatible_url(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, _desktop, _restore_data_dir) = open_test_desktop(cx);
    let url = qq_farm_app::config::save_image("uploaded.png", "AQID")
        .expect("write the same image payload accepted by the configuration editor");
    let Ok(ImageSource::Image(image)) = crate::assets::resolve_image_source(&url) else {
        panic!("uploaded image should resolve to decoded image bytes");
    };
    assert_eq!(image.bytes, [1, 2, 3]);
    assert!(matches!(
        crate::assets::resolve_image_source("/api/config/images/../Plant.json"),
        Err(crate::assets::AssetError::InvalidPath)
    ));
}
