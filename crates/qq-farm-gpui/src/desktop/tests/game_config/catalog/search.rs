use super::*;

#[gpui_kit::test]
fn game_config_empty_copy_follows_vue_no_data_and_filters(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({"seeds": []});
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("game-config-empty").label(), Some("无数据"));
        desktop.update(cx, |this, _| this.game_config.applied_keyword = "missing".into());
        window.render_frame(cx);
        assert_eq!(window.find("game-config-empty").label(), Some("没有符合条件的配置"));
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("game-config-empty").label(), Some("No matching configurations"));
    })
    .expect("game-config table empty copy mirrors Vue noData unless filters are active");
}

#[gpui_kit::test]
fn game_config_search_matches_vue_catalog_fields_only(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [
                    {"name":"Golden Wheat", "seedId":1204, "requiredLevel":9},
                    {"name":"Silver Wheat", "seedId":1205, "requiredLevel":18}
                ]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            this.game_config.applied_keyword = "18".into();
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, _| this.game_config.applied_keyword = "18".into());
        window.render_frame(cx);
        assert!(window.try_find("game-config-record-0").is_none());
        assert!(window.try_find("game-config-record-1").is_none());

        desktop.update(cx, |this, _| this.game_config.applied_keyword = "1205".into());
        window.render_frame(cx);
        assert!(window.try_find("game-config-record-0").is_none());
        assert!(window.find("game-config-record-1").visible());
        window.click("game-config-select-1", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).game_config.selected_ids.contains(&("seeds".into(), 1205)));
    })
    .expect("search only the game-config fields supported by the Vue page");
}
#[gpui_kit::test]
fn game_config_search_drafts_apply_only_on_search_and_reset_matches_vue(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [
                    {"name": "Silver Wheat", "seedId": 1204, "seasons": 1},
                    {"name": "Golden Wheat", "seedId": 1205, "seasons": 2}
                ]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("page-search").is_none());
        assert!(window.try_find("game-config-search-keyword").is_none());
        window.click("game-config-search-toggle", cx);
        window.render_frame(cx);
        assert!(window.find("game-config-search-keyword").visible());
        assert!(window.find("game-config-record-0").visible());
        assert!(window.find("game-config-record-1").visible());

        desktop.update(cx, |this, cx| {
            this.search.update(cx, |state, cx| state.set_value("1205", window, cx));
        });
        window.within("game-config-season-select").click("input", cx);
        assert_eq!(window.find("game-config-season-select").expanded(), Some(true));
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .expect("choose a season from the single-select menu");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.seasons, Some(2));
        assert!(window.find("game-config-record-0").visible());
        assert!(window.find("game-config-record-1").visible());

        window.click("game-config-search-apply", cx);
        window.render_frame(cx);
        assert!(window.try_find("game-config-record-0").is_none());
        assert!(window.find("game-config-record-1").visible());

        window.click("game-config-search-reset", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).search.read(cx).value().is_empty());
        assert_eq!(desktop.read(cx).game_config.seasons, None);
        let expected_season_all = if desktop.read(cx).preferences.language == "en-US" {
            "All seasons"
        } else {
            "全部季数"
        };
        assert_eq!(window.find("game-config-season-select").value(), Some(expected_season_all));
        assert!(window.try_find("game-config-record-0").is_none());
        assert!(window.find("game-config-record-1").visible());

        window.click("game-config-search-apply", cx);
        window.render_frame(cx);
        assert!(window.find("game-config-record-0").visible());
        assert!(window.find("game-config-record-1").visible());
    })
    .expect("apply the catalog search only on submit and reset only its draft fields");
}

#[gpui_kit::test]
fn search_shortcut_opens_and_focuses_the_game_config_filter(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [{"name":"Wheat", "seedId":1204}]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!desktop.read(cx).game_config.search_open);
        assert!(window.try_find("game-config-search-keyword").is_none());
        window.press("secondary-f", cx);
    })
    .expect("dispatch the page search shortcut");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(desktop.read(cx).game_config.search_open);
        assert!(window.find("game-config-search-keyword").visible());
        assert_eq!(window.find("game-config-search-keyword").focused(), Some(true));
    })
    .expect("show and focus the game-config page's own search field");
}
#[gpui_kit::test]
fn game_config_item_type_and_rarity_are_single_select_drafts(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "items".into();
            this.page_raw = serde_json::json!({
                "items": [
                    {"id": 1, "name": "Compost", "type": 3, "rarity": 4},
                    {"id": 2, "name": "Water", "type": 7, "rarity": 1}
                ],
                "itemTypes": [
                    {"value": 3, "label": "经验"},
                    {"value": 7, "label": "化肥"}
                ]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            this.sync_game_config_selectors(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("game-config-search-toggle", cx);
        window.render_frame(cx);
        window.within("game-config-item-type-select").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .expect("choose an item type from the single-select menu");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        assert_eq!(desktop.read(cx).game_config.item_type, Some(3));
        window.within("game-config-rarity-select").click("input", cx);
        for _ in 0..5 {
            window.press("down", cx);
        }
        window.press("enter", cx);
    })
    .expect("choose a rarity from the single-select menu");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let draft = desktop.read(cx);
        assert_eq!(draft.game_config.rarity, Some(4));
        assert_eq!(draft.game_config.applied_item_type, None);
        assert_eq!(draft.game_config.applied_rarity, None);
        assert!(window.find("game-config-record-0").visible());
        assert!(window.find("game-config-record-1").visible());

        window.click("game-config-search-apply", cx);
        window.render_frame(cx);
        assert!(window.find("game-config-record-0").visible());
        assert!(window.try_find("game-config-record-1").is_none());

        window.click("game-config-search-reset", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.item_type, None);
        assert_eq!(desktop.read(cx).game_config.rarity, None);
        assert_eq!(window.find("game-config-item-type-select").value(), Some("全部类型"));
        assert_eq!(window.find("game-config-rarity-select").value(), Some("全部品质"));
        assert!(window.find("game-config-record-0").visible());
        assert!(window.try_find("game-config-record-1").is_none());

        window.click("game-config-search-apply", cx);
        window.render_frame(cx);
        assert!(window.find("game-config-record-0").visible());
        assert!(window.find("game-config-record-1").visible());
    })
    .expect("keep game-config select filters as drafts until search is submitted");
}
#[gpui_kit::test]
fn game_config_category_change_clears_vue_filters_selection_and_keyword(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.game_config.seasons = Some(2);
            this.game_config.rarity = Some(4);
            this.game_config.item_type = Some(3);
            this.game_config.page = 2;
            this.game_config.selected_ids.insert(("seeds".into(), 501));
            this.game_config.columns.get_mut("fruits").unwrap()[0].checked = false;
            this.search.update(cx, |state, cx| state.set_value("wheat", window, cx));
            this.set_page_section("fruits", window, cx);
        });
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert_eq!(state.page_section, "fruits");
        assert_eq!(state.search.read(cx).value(), "");
        assert_eq!(state.game_config.seasons, None);
        assert_eq!(state.game_config.rarity, None);
        assert_eq!(state.game_config.item_type, None);
        assert_eq!(state.game_config.page, 0);
        assert!(state.game_config.selected_ids.is_empty());
        assert!(state.game_config.columns["fruits"].iter().all(|column| column.checked));
    })
    .expect("changing catalog tabs resets Vue-local filters and row selection");
}
