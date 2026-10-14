use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

use super::wait_for_sheet;

#[gpui_kit::test]
async fn seed_configuration_write_survives_a_fresh_game_config_load(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let seed_id = 987_654_321;

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.open_game_config_form(None, window, cx);
            this.game_config
                .id
                .update(cx, |input, cx| input.set_value(seed_id.to_string(), window, cx));
            this.game_config
                .name
                .update(cx, |input, cx| input.set_value("Persistence probe", window, cx));
            this.game_config.extra_fields["price"]
                .update(cx, |input, cx| input.set_value("720", window, cx));
            this.game_config.extra_fields["exp"]
                .update(cx, |input, cx| input.set_value("9", window, cx));
            this.game_config.extra_fields["seasons"]
                .update(cx, |input, cx| input.set_value("2", window, cx));
        });
        window.render_frame(cx);
    })
    .expect("open and populate the seed configuration form");
    wait_for_sheet(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        window.within("sheet-content").click("config-form-save", cx);
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx), "a valid form should close after saving");
    })
    .expect("save the seed through the GPUI form");

    // Load a new config instance from the same FARM_DATA_DIR to exercise the on-disk overlay,
    // rather than checking the live singleton that the save operation also refreshes.
    let reloaded = qq_farm_core::config::game_config::GameConfig::new();
    reloaded.load();
    let plant =
        reloaded.get_plant_by_seed_id(seed_id).expect("seed overlay is readable after reload");
    assert_eq!(plant.name, "Persistence probe");
    assert_eq!(plant.land_level_need, Some(1));
    assert_eq!(plant.seasons, Some(2));
    assert_eq!(plant.exp, Some(9));
    let seed_item =
        reloaded.get_item_by_id(seed_id).expect("seed sale overlay is readable after reload");
    assert_eq!(seed_item.item_type, 5);
    assert_eq!(seed_item.name, "Persistence probe");
    assert_eq!(seed_item.sells.as_ref().and_then(serde_json::Value::as_str), Some("0:720"));

    qq_farm_app::config::delete_game_config(qq_farm_app::config::GameConfigKind::Plant, seed_id)
        .expect("remove the probe plant overlay");
    qq_farm_app::config::delete_game_config(qq_farm_app::config::GameConfigKind::Item, seed_id)
        .expect("remove the probe seed item overlay");
}

#[gpui_kit::test]
async fn fruit_and_item_configuration_writes_survive_a_fresh_game_config_load(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let plant_id = 987_654_322;
    let fruit_id = 987_654_323;
    let item_id = 987_654_324;
    qq_farm_app::config::upsert_game_config(
        qq_farm_app::config::GameConfigEntry::Plant(qq_farm_core::config::game_config::Plant {
            id: plant_id,
            name: "Fruit persistence parent".into(),
            seed_id: Some(plant_id),
            fruit: Some(qq_farm_core::config::game_config::PlantFruit { id: fruit_id, count: 6 }),
            ..Default::default()
        }),
        None,
    )
    .expect("create the parent plant needed by the fruit editor");

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "fruits".into();
            this.open_game_config_form(None, window, cx);
            this.game_config
                .name
                .update(cx, |input, cx| input.set_value("Fruit persistence probe", window, cx));
            this.game_config.plant_id = Some(plant_id);
            this.sync_game_config_plant_selector(window, cx);
            this.game_config.extra_fields["price"]
                .update(cx, |input, cx| input.set_value("321", window, cx));
            this.game_config.extra_fields["rarity"]
                .update(cx, |input, cx| input.set_value("3", window, cx));
            this.game_config.extra_fields["maxCount"]
                .update(cx, |input, cx| input.set_value("55", window, cx));
            this.game_config.extra_fields["fruitCount"]
                .update(cx, |input, cx| input.set_value("7", window, cx));
            this.game_config
                .asset
                .update(cx, |input, cx| input.set_value("fruit-persistence.webp", window, cx));
        });
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.id.read(cx).value(), "");
    })
    .expect("open and populate the fruit configuration form");
    wait_for_sheet(cx, window).await;
    cx.update_window(window, |_, window, cx| {
        window.within("sheet-content").click("config-form-save", cx);
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx), "a valid fruit form should close after saving");
    })
    .expect("save the fruit through the GPUI form");

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page_section = "items".into();
            this.open_game_config_form(None, window, cx);
            this.game_config
                .id
                .update(cx, |input, cx| input.set_value(item_id.to_string(), window, cx));
            this.game_config
                .name
                .update(cx, |input, cx| input.set_value("Item persistence probe", window, cx));
            this.game_config
                .item_type_form_selector
                .update(cx, |selector, cx| selector.set_selected_value(&9, window, cx));
            this.game_config.extra_fields["interactionType"]
                .update(cx, |input, cx| input.set_value("protect", window, cx));
            this.game_config.extra_fields["canUse"]
                .update(cx, |input, cx| input.set_value("1", window, cx));
            this.game_config.extra_textareas["effectDesc"].update(cx, |input, cx| {
                input.set_value("Item effect persistence probe", window, cx)
            });
            this.game_config.extra_fields["price"]
                .update(cx, |input, cx| input.set_value("88", window, cx));
            this.game_config.extra_fields["priceId"]
                .update(cx, |input, cx| input.set_value("1004", window, cx));
        });
        window.render_frame(cx);
    })
    .expect("open and populate the item configuration form");
    wait_for_sheet(cx, window).await;
    cx.update_window(window, |_, window, cx| {
        window.within("sheet-content").click("config-form-save", cx);
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx), "a valid item form should close after saving");
    })
    .expect("save the item through the GPUI form");

    let reloaded = qq_farm_core::config::game_config::GameConfig::new();
    reloaded.load();
    let parent = reloaded.get_plant_by_id(plant_id).expect("parent plant is readable after reload");
    let fruit = parent.fruit.expect("fruit link survives the fresh load");
    assert_eq!(fruit.id, fruit_id);
    assert_eq!(fruit.count, 7);
    let fruit_item =
        reloaded.get_item_by_id(fruit_id).expect("fruit item is readable after reload");
    assert_eq!(fruit_item.item_type, 6);
    assert_eq!(fruit_item.name, "Fruit persistence probe");
    assert_eq!(fruit_item.rarity, Some(3));
    assert_eq!(fruit_item.max_count, Some(55));
    assert_eq!(fruit_item.asset_name.as_deref(), Some("fruit-persistence.webp"));
    let item = reloaded.get_item_by_id(item_id).expect("item overlay is readable after reload");
    assert_eq!(item.item_type, 9);
    assert_eq!(item.name, "Item persistence probe");
    assert_eq!(item.interaction_type.as_deref(), Some("protect"));
    assert_eq!(item.can_use, Some(1));
    assert_eq!(item.effect_desc.as_deref(), Some("Item effect persistence probe"));
    assert_eq!(item.sells.as_ref().and_then(serde_json::Value::as_str), Some("1004:88"));

    for (kind, id) in [
        (qq_farm_app::config::GameConfigKind::Plant, plant_id),
        (qq_farm_app::config::GameConfigKind::Item, fruit_id),
        (qq_farm_app::config::GameConfigKind::Item, item_id),
    ] {
        qq_farm_app::config::delete_game_config(kind, id).expect("remove config persistence probe");
    }
}
