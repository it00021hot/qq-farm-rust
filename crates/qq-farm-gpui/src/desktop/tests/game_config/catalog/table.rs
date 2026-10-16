use super::*;

#[gpui_kit::test]
fn game_config_full_page_renders_scrolls_and_selects_all_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let seeds = (1..=101)
        .map(|seed_id| serde_json::json!({"seedId": seed_id, "name": format!("Seed {seed_id}")}))
        .collect::<Vec<_>>();
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.game_config.page_size = 100;
            this.page_raw = serde_json::json!({"seeds": seeds});
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let footer_top = window.find("game-config-page-size-selector").bounds().top();
        assert!(window.find("game-config-record-0").visible());
        let last_before = window.find("game-config-record-99").bounds();
        assert!(
            last_before.bottom() > footer_top,
            "with 100 rows per page the last row starts below the visible body"
        );
        window.scroll("game-config-record-0", ScrollDelta::Pixels(point(px(0.), px(-10_000.))), cx);
        window.render_frame(cx);
        let last_after = window.find("game-config-record-99").bounds();
        assert!(last_after.top() < last_before.top() - px(1000.));
        assert!(last_after.bottom() < footer_top, "the last row scrolls into view");
        window.click("game-config-select-page", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.selected_ids.len(), 100);
    })
    .expect("render, scroll and select a full 100-row game-config page");
}

#[gpui_kit::test]
fn game_config_table_scrolls_the_body_and_keeps_the_header_fixed(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let seeds = (1..=10)
        .map(|seed_id| serde_json::json!({"seedId": seed_id, "name": format!("Seed {seed_id}")}))
        .collect::<Vec<_>>();
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({"seeds": seeds});
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let header_before = window.find("game-config-table-header").bounds();
        let footer_top = window.find("game-config-page-size-selector").bounds().top();
        let last_before = window.find("game-config-record-9").bounds();
        assert!(
            last_before.bottom() > footer_top,
            "the last row of a tall page starts out below the visible body"
        );
        // 大幅滚动：滚轮 delta 会按命中链在页面/表体滚动容器间分摊，
        // 小滚动量的净位移可能落在阈值边缘（随工具栏高度浮动）。
        window.scroll("game-config-record-0", ScrollDelta::Pixels(point(px(0.), px(-800.))), cx);
        window.render_frame(cx);
        let last_after = window.find("game-config-record-9").bounds();
        // 行随表体滚动上移并落入可视区（位移量取决于表体剩余可滚范围，随布局浮动）
        assert!(
            last_after.top() < last_before.top() - px(5.),
            "the body scrolls rows: before {last_before:?} after {last_after:?}"
        );
        assert!(last_after.bottom() < footer_top, "scrolled rows land inside the visible body");
        let header_after = window.find("game-config-table-header").bounds();
        assert!(
            (header_after.origin.y - header_before.origin.y).abs() < px(0.5),
            "the header band stays fixed while the body scrolls"
        );
    })
    .expect("scroll a tall game-config page inside its table body");
}

#[gpui_kit::test]
fn game_config_footer_uses_the_shared_table_pagination(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let seeds = (1..=25)
        .map(|seed_id| serde_json::json!({"seedId": seed_id, "name": format!("Seed {seed_id}")}))
        .collect::<Vec<_>>();
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({"seeds": seeds});
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("game-config-page-size-selector").label(), Some("每页 10 条"));
        assert_eq!(window.find("game-config-page-indicator").label(), Some("第 1 / 3 页"));
        window.click("game-config-page-next", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.page, 1);
        assert!(window.find("game-config-record-10").visible());
        window.click("game-config-page-previous", cx);
        window.render_frame(cx);
        window.click("game-config-page-size-selector", cx);
        window.render_frame(cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
        window.render_frame(cx);
        assert_eq!(window.find("game-config-page-size-selector").label(), Some("每页 20 条"));
        assert_eq!(desktop.read(cx).game_config.page_size, 20);
        assert_eq!(desktop.read(cx).game_config.page, 0);
        assert_eq!(window.find("game-config-page-indicator").label(), Some("第 1 / 2 页"));
    })
    .expect("drive the shared pagination footer from the game-config table");
}

#[gpui_kit::test]
fn game_config_pagination_jumps_to_numbered_and_collapsed_pages(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let seeds = (1..=101)
        .map(|seed_id| serde_json::json!({"seedId": seed_id, "name": format!("Seed {seed_id}")}))
        .collect::<Vec<_>>();
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({"seeds": seeds});
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("game-config-page-number", 4_usize)).label(), Some("第 5 页"));
        window.click(("game-config-page-jump", 7_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.page, 7);
        assert!(window.find("game-config-record-70").visible());
        assert!(window.try_find("game-config-record-0").is_none());

        window.click(("game-config-page-number", 8_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.page, 8);
        assert!(window.find("game-config-record-80").visible());
        assert!(window.try_find("game-config-record-70").is_none());
    })
    .expect("jump to a collapsed page range and then a directly numbered catalog page");
}
#[gpui_kit::test]
fn game_config_row_without_an_identifier_stays_visible_and_cannot_be_selected(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [{"name":"Incomplete crop", "requiredLevel":2}]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("game-config-record-0").visible());
        assert!(window.find("game-config-cell-seed-0").visible());
        assert!(window.try_find(("config-delete", 0_usize)).is_none());
        window.click("game-config-select-0", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).game_config.selected_ids.is_empty());
    })
    .expect("display incomplete source rows without allowing invalid writes");
}
#[gpui_kit::test]
fn game_config_column_settings_hide_reorder_and_pin_columns(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [{
                    "seedId": 501, "name": "金麦", "requiredLevel": 8,
                    "seasons": 2, "growTime": 3600, "harvestCount": 3,
                    "exp": 9, "price": 12, "priceId": 1005
                }]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("game-config-column-settings-trigger", cx);
        window.render_frame(cx);
        assert!(window.find("game-config-column-visible-seedId").visible());
        window.click("game-config-column-visible-seedId", cx);
        window.render_frame(cx);
        assert!(!desktop.read(cx).game_config.columns["seeds"][1].checked);
        assert!(window.try_find("game-config-header-seedId").is_none());

        window.click("game-config-column-up-requiredLevel", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.columns["seeds"][1].key, "requiredLevel");
        window.click("game-config-column-pin-requiredLevel", cx);
        window.render_frame(cx);
        assert_eq!(
            desktop.read(cx).game_config.columns["seeds"][1].pin,
            crate::desktop::game_config::columns::GameConfigColumnPin::Left
        );
        assert!(window.find("game-config-table-left-header").visible());
        assert!(window.find("game-config-header-requiredLevel").visible());
        let pinned_cell = window.find("game-config-cell-requiredLevel-0").bounds();
        let center_cell = window.find("game-config-select-0").bounds();
        assert!(
            (pinned_cell.center().y - center_cell.center().y).abs() < px(1.),
            "the pinned column lane stays baseline-aligned with the center rows"
        );

        window.click("game-config-columns-select-all", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).game_config.columns["seeds"].iter().all(|column| column.checked));
    })
    .expect("change game-config column visibility, order, and pinned lane");
}
#[gpui_kit::test]
async fn game_config_catalog_uses_vue_columns_and_row_selection(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::GameConfig;
            this.page_section = "seeds".into();
            this.page_raw = serde_json::json!({
                "seeds": [{
                    "seedId": 501, "plantId": 701, "name": "金麦", "requiredLevel": 8,
                    "seasons": 2, "growTime": 3660, "harvestCount": 3,
                    "exp": 9, "price": 12, "priceId": 1005
                }],
                "fruits": [{
                    "id": 601, "name": "麦穗", "plantName": "金麦", "rarity": 3,
                    "price": 6, "priceId": 1004
                }],
                "items": [{
                    "id": 801, "name": "播种机", "type": 3, "rarity": 4,
                    "price": 20, "priceId": 0
                }],
                "itemTypes": [{"value": 3, "label": "农具"}]
            });
            this.rows = crate::pages::page_data(Page::GameConfig, this.page_raw.clone()).rows;
            cx.notify();
        });
    });

    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        for column in [
            "seed",
            "seedId",
            "requiredLevel",
            "seasons",
            "growTime",
            "harvestCount",
            "exp",
            "price",
            "operate",
        ] {
            assert!(window.find(format!("game-config-header-{column}")).visible());
            assert!(window.find(format!("game-config-cell-{column}-0")).visible());
        }
        assert!(window.find("game-config-select-0").visible());
        assert!(window.find("game-config-cell-operate-0").visible());
        assert!(window.find(("config-edit", 0_usize)).visible());
        assert!(window.find(("config-delete", 0_usize)).visible());

        window.click(("config-delete", 0_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).game_config.pending_delete, Some(("seed".into(), 501)));
    })
    .expect("open the anchored single-row delete confirmation");
    wait_for_delete_popover(cx, window).await;
    cx.update_window(window, |_, window, cx| window.click(("config-delete-cancel", 0_usize), cx))
        .expect("cancel the row deletion");
    cx.wait_for(window, Duration::from_secs(1), |_, cx| {
        desktop.read(cx).game_config.pending_delete.is_none()
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        assert!(desktop.read(cx).game_config.pending_delete.is_none());
        window.click(("config-edit", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
    })
    .expect("edit the row after the confirmation has closed");
    wait_for_sheet(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        window.within("sheet-content").click("config-form-cancel", cx);
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx));

        window.click("game-config-select-0", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).game_config.selected_ids.contains(&("seeds".into(), 501)));

        desktop.update(cx, |this, cx| {
            this.page_section = "fruits".into();
            cx.notify();
        });
        window.render_frame(cx);
        for column in ["fruit", "fruitId", "plantName", "rarity", "price", "operate"] {
            assert!(window.find(format!("game-config-header-{column}")).visible());
            assert!(window.find(format!("game-config-cell-{column}-1")).visible());
        }

        desktop.update(cx, |this, cx| {
            this.page_section = "items".into();
            cx.notify();
        });
        window.render_frame(cx);
        for column in ["item", "itemId", "type", "rarity", "price", "operate"] {
            assert!(window.find(format!("game-config-header-{column}")).visible());
            assert!(window.find(format!("game-config-cell-{column}-2")).visible());
        }
    })
    .expect("render category-specific Vue columns and toggle a real row checkbox");

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page_section = "seeds".into();
            this.game_config.page = 0;
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1100.), px(720.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("game-config-header-seed").visible());
        window.scroll(
            "game-config-table-header",
            ScrollDelta::Pixels(point(px(-1000.), px(0.))),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find("game-config-header-operate").visible());
    })
    .expect(
        "the seed table should expose its action column by horizontal scrolling at minimum size",
    );
}
