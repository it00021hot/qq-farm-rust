use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

use super::wait_for_sheet;

#[gpui_kit::test]
fn game_config_success_feedback_is_rendered_for_catalog_changes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        window.click("game-config", cx);
        desktop.update(cx, |this, cx| {
            this.game_config.success_feedback = Some("配置已保存".into());
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("game-config-success-feedback").visible());
    })
    .expect("show confirmation after saving or deleting a configuration");
}

#[gpui_kit::test]
async fn game_config_sheet_preserves_seed_defaults_and_fruit_picker_semantics(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.selected_account = Some("7".into());
            this.open_game_config_form(None, window, cx);
        });
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
        assert_eq!(desktop.read(cx).game_config.level.read(cx).value(), "1");
    })
    .expect("open the seed configuration sheet with the Vue default level");
    wait_for_sheet(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        window.within("sheet-content").click("config-form-cancel", cx);
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx));
        desktop.update(cx, |this, cx| {
            this.page_section = "fruits".into();
            this.open_game_config_form(None, window, cx);
        });
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
        assert_eq!(desktop.read(cx).game_config.kind, "fruit");
        assert!(desktop.read(cx).game_config.record.is_null());
    })
    .expect("close seed editing and open a new fruit form");
    wait_for_sheet(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.game_config.plants = vec![serde_json::json!({
                "id": 1001,
                "seed_id": 2001,
                "name": "草莓",
                "fruit": {"id": 4001, "count": 5}
            })];
            this.sync_game_config_plant_selector(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("config-field-row-config-fruit-count").visible());
        window.within("config-plant").click("input", cx);
        window.input("2001", cx);
    })
    .expect("search the linked plants using the searchable single-select control");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        let selector = desktop.read(cx).game_config.plant_form_selector.read(cx);
        assert_eq!(selector.selected_index(cx), Some(gpui_kit::component::IndexPath::new(0)),);
        window.press("enter", cx);
    })
    .expect("confirm the highlighted plant option");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert_eq!(state.game_config.plant_id, Some(1001));
        assert!(state.game_config.id.read(cx).value().is_empty());
        assert_eq!(state.game_config.name.read(cx).value(), "草莓");
        window.within("sheet-content").click("config-form-cancel", cx);
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx));
        desktop.update(cx, |this, cx| {
            this.open_game_config_form(
                Some(serde_json::json!({
                    "id": 101,
                    "plantId": 999,
                    "name": "测试果实",
                    "fruitCount": 777,
                    "price": 10,
                    "priceId": 0,
                    "rarity": 1,
                    "maxCount": 9999,
                    "assetName": "fruit_test"
                })),
                window,
                cx,
            );
        });
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
        let state = desktop.read(cx);
        assert_eq!(state.game_config.kind, "fruit");
        assert_eq!(state.game_config.record["id"], 101);
        assert_eq!(window.find("config-plant").value(), Some("植物 #999（当前目录未找到）"));
        assert_eq!(state.game_config.extra_fields["fruitCount"].read(cx).value(), "200");
        assert!(window.find("config-field-row-config-fruit-count").visible());
        assert!(state.game_config.form_open);
    })
    .expect("open the linked fruit edit sheet and retain its source record");
    wait_for_sheet(cx, window).await;
}

#[gpui_kit::test]
async fn game_config_item_type_uses_the_searchable_catalog_options(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "items".into();
            this.open_game_config_form(None, window, cx);
        });
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
    })
    .expect("open the item editor with the default Vue type");
    wait_for_sheet(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.find("config-type").value(), Some("7 - 化肥"));
        window.within("config-type").click("input", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .expect("choose an allowed item type from the searchable catalog");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("config-type").value(), Some("9 - 宠物食品"));
        assert_eq!(
            desktop.read(cx).game_config.item_type_form_selector.read(cx).selected_value(),
            Some(&9),
        );
    })
    .expect("choose a type only from the searchable backend catalog");
}

#[gpui_kit::test]
async fn game_config_item_editor_preserves_an_unknown_existing_type(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "items".into();
            this.open_game_config_form(
                Some(serde_json::json!({
                    "id": 987_654_321,
                    "type": 99,
                    "name": "Legacy item"
                })),
                window,
                cx,
            );
        });
        window.render_frame(cx);
    })
    .expect("open an existing item whose type is absent from the current directory");
    wait_for_sheet(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.find("config-type").value(), Some("99"));
        assert_eq!(
            desktop.read(cx).game_config.item_type_form_selector.read(cx).selected_value(),
            Some(&99),
        );
    })
    .expect("preserve the existing Vue model value when no catalog option matches it");
}

#[gpui_kit::test]
async fn game_config_seed_edit_fallbacks_match_vue_for_zero_and_missing_fields(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.open_game_config_form(
                Some(serde_json::json!({
                    "seedId": 987_654_321,
                    "name": "Legacy seed",
                    "requiredLevel": 0,
                    "seasons": 0,
                    "harvestCount": 0
                })),
                window,
                cx,
            );
        });
        window.render_frame(cx);
    })
    .expect("open an existing seed with falsey legacy values");
    wait_for_sheet(cx, window).await;

    cx.update_window(window, |_, _, cx| {
        let state = desktop.read(cx);
        assert_eq!(state.game_config.level.read(cx).value(), "1");
        assert_eq!(state.game_config.extra_fields["seasons"].read(cx).value(), "1");
        assert_eq!(state.game_config.extra_fields["fruitCount"].read(cx).value(), "200");
        assert_eq!(state.game_config.extra_textareas["growPhases"].read(cx).value(), "");
    })
    .expect("use the same Vue truthy fallbacks for seed fields");
}

#[gpui_kit::test]
async fn game_config_new_seed_prefills_the_vue_phase_template(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.open_game_config_form(None, window, cx);
        });
        window.render_frame(cx);
    })
    .expect("open the new-seed form");
    wait_for_sheet(cx, window).await;
    cx.update_window(window, |_, _, cx| {
        assert_eq!(
            desktop.read(cx).game_config.extra_textareas["growPhases"].read(cx).value(),
            "种子:2400;发芽:2400;小叶子:2400;大叶子:2400;开花:2400;成熟:0;"
        );
    })
    .expect("new seeds start from Vue's first phase template");
}

#[gpui_kit::test]
async fn game_config_seed_edit_restores_stored_grow_phases(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.open_game_config_form(
                Some(serde_json::json!({
                    "seedId": 42,
                    "name": "Stored seed",
                    "growPhases": "种子:4800;发芽:4800;小叶子:4800;大叶子:4800;开花:4800;成熟:0;"
                })),
                window,
                cx,
            );
        });
        window.render_frame(cx);
    })
    .expect("open an existing seed with stored phases");
    wait_for_sheet(cx, window).await;
    cx.update_window(window, |_, _, cx| {
        assert_eq!(
            desktop.read(cx).game_config.extra_textareas["growPhases"].read(cx).value(),
            "种子:4800;发芽:4800;小叶子:4800;大叶子:4800;开花:4800;成熟:0;"
        );
    })
    .expect("reopen stored phases instead of dropping them");
}

#[gpui_kit::test]
async fn game_config_page_and_seed_sheet_follow_english_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.set_language("en-US", window, cx);
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [{"seedId": 1204, "name": "Golden Wheat", "seasons": 1}]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("game-config-search-toggle", cx);
        window.render_frame(cx);

        assert_eq!(window.find("game-config-season-select").value(), Some("All seasons"));
        window.within("game-config-season-select").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .expect("choose a season from the localized single-select menu");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("game-config-season-select").value(), Some("Single season"));
        assert_eq!(window.find("game-config-add").label(), Some("Add"));
        assert_eq!(window.find("game-config-page-previous").label(), Some("Previous"));
        assert_eq!(
            desktop.read(cx).game_config.name.read(cx).presentation().placeholder().as_ref(),
            "Name"
        );

        window.click("game-config-add", cx);
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
        assert_eq!(window.find("config-form-save").label(), Some("Confirm"));
        assert_eq!(
            desktop.read(cx).game_config.extra_textareas["growPhases"]
                .read(cx)
                .presentation()
                .placeholder()
                .as_ref(),
            "Growth phases"
        );
    })
    .expect("localize the game configuration page and seed sheet");
    wait_for_sheet(cx, window).await;
}
