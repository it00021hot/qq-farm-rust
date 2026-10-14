use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{px, size, AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn strategy_sections_top_align_with_the_strategy_controls(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let label_top = window.find("planting-strategy-label").bounds().top();
        for id in
            ["settings-intervals-card", "settings-quiet-hours-card", "settings-plant-delay-card"]
        {
            let top = window.find(id).bounds().top();
            assert!(
                (top - label_top).abs() < px(1.),
                "{id} must top-align with the strategy controls (label {label_top:?}, card {top:?})"
            );
        }
    })
    .expect("settings strategy sections share one top alignment line");
}

#[gpui_kit::test]
fn planting_selectors_restore_vue_strategy_and_seed_choices(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.settings.draft = serde_json::json!({
                "plantingStrategy": "bag_priority",
                "bagSeedFallbackStrategy": "preferred",
                "preferredSeedId": 42
            });
            this.page_raw = serde_json::json!({
                "seedOptions": [
                    {"seed_id": 42, "name": "玉米", "land_level_need": 6},
                    {"seed_id": 43, "name": "番茄", "land_level_need": 2}
                ]
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_setting_selectors(window, cx));
        window.render_frame(cx);
        assert!(window.find("planting-strategy-select").visible());
        assert!(window.find("bag-seed-fallback-select").visible());
        assert!(window.find("preferred-seed-select").visible());
        assert_eq!(
            window.find("settings-switch-bagSeedMultiLandReservationEnabled").label(),
            Some("多格种子预留空地")
        );
        assert_eq!(window.find("bag-seed-priority-reset").label(), Some("重置顺序"));
        assert!(window.try_find("strategy-preview-value").is_none());
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(
            window.find("settings-switch-bagSeedMultiLandReservationEnabled").label(),
            Some("Reserve Lands for Multi-Land Seeds")
        );
        assert_eq!(window.find("bag-seed-priority-reset").label(), Some("Reset Order"));
        desktop.update(cx, |this, cx| this.set_language("zh-CN", window, cx));
    })
    .expect("render planting strategy selectors");

    cx.update_window(window, |_, window, cx| {
        window.click("preferred-seed-select", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .expect("choose a preferred seed from the keyboard");

    cx.update(|cx| {
        let desktop = desktop.read(cx);
        assert_eq!(
            desktop.settings.planting_strategy.read(cx).selected_value().copied(),
            Some("bag_priority")
        );
        assert_eq!(
            desktop.settings.bag_fallback.read(cx).selected_value().copied(),
            Some("preferred")
        );
        assert_eq!(desktop.settings.preferred_seed.read(cx).selected_value().copied(), Some(43));
        assert_eq!(desktop.settings.draft["preferredSeedId"], 43);
    });
}

#[gpui_kit::test]
fn strategy_preview_shows_vue_level_choice_and_empty_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.settings.draft = serde_json::json!({"plantingStrategy":"level"});
            this.page_raw = serde_json::json!({
                "seedOptions": [
                    {"seedId":20,"name":"番茄","requiredLevel":10},
                    {"seedId":10,"name":"玉米","requiredLevel":10},
                    {"seedId":1,"name":"锁定种子","requiredLevel":99,"locked":true}
                ]
            });
            this.refresh_strategy_preview(cx);
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_setting_selectors(window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("strategy-preview-label").label(), Some("策略选种预览"));
        assert_eq!(window.find("strategy-preview-value").label(), Some("Lv10 玉米"));

        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("strategy-preview-label").label(), Some("Strategy Seed Preview"));
        assert_eq!(window.find("strategy-preview-value").label(), Some("Lv10 玉米"));

        desktop.update(cx, |this, cx| {
            this.page_raw["seedOptions"] = serde_json::json!([
                {"seedId":1,"name":"锁定种子","requiredLevel":99,"locked":true}
            ]);
            this.refresh_strategy_preview(cx);
        });
        window.render_frame(cx);
        assert_eq!(window.find("strategy-preview-value").label(), Some("No available seeds"));
    })
    .expect("show the same strategy preview and empty state as the Vue settings form");
}

#[gpui_kit::test]
fn bag_seed_priority_rows_can_be_reordered_by_dragging(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.settings.draft = serde_json::json!({
                "plantingStrategy": "bag_priority",
                "bagSeedFallbackStrategy": "max_exp",
                "bagSeedPriority": [10, 20, 30]
            });
            this.page_raw = serde_json::json!({
                "bagSeeds": [
                    {"seedId": 10, "name": "玉米", "count": 3, "requiredLevel": 4, "plantSize": 2},
                    {"seedId": 20, "name": "番茄", "count": 2, "requiredLevel": 5, "plantSize": 1},
                    {"seedId": 30, "name": "南瓜", "count": 1, "requiredLevel": 6, "plantSize": 3}
                ]
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_setting_selectors(window, cx));
        window.render_frame(cx);
        assert!(window.find("bag-seed-priority-10").visible());
        assert!(window.find("bag-seed-priority-30").visible());
        assert_eq!(
            window.find("bag-seed-order-hint").label(),
            Some("先按下方顺序消耗背包中的 1x1 / 2x2 种子；背包种子不足时，再按“第二优先策略”补种。切换第二优先策略或重置时会据此重新排序。")
        );
        window.drag_to("bag-seed-priority-10", "bag-seed-priority-30", cx);
        desktop.update(cx, |this, cx| {
            this.preferences.language = "en-US".into();
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("bag-seed-order-hint").label(),
            Some("Consume 1x1 / 2x2 bag seeds in the order below first; when bag seeds run out, fall back to the secondary strategy. Changing the fallback strategy or resetting will re-sort accordingly.")
        );
    })
    .expect("drag the first inventory seed onto the third row");

    cx.update(|cx| {
        let state = desktop.read(cx);
        assert_eq!(state.settings.draft["bagSeedPriority"], serde_json::json!([20, 10, 30]));
    });
}

#[gpui_kit::test]
fn bag_seed_loading_error_and_empty_states_are_mutually_exclusive(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.settings.draft = serde_json::json!({
                "plantingStrategy": "bag_priority",
                "bagSeedFallbackStrategy": "max_exp"
            });
            this.page_raw = serde_json::json!({"bagSeeds": null});
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_setting_selectors(window, cx));
        window.render_frame(cx);
        assert!(window.find("bag-seeds-loading").visible());
        assert!(window.try_find("bag-seeds-error").is_none());
        assert!(window.try_find("bag-seeds-empty").is_none());

        desktop.update(cx, |this, cx| {
            this.page_raw = serde_json::json!({
                "bagSeedsError": "offline",
                "bagSeeds": [{"seedId": 10, "name": "玉米", "count": 3}]
            });
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("bag-seeds-error").visible());
        assert!(window.try_find("bag-seeds-loading").is_none());
        assert!(window.try_find("bag-seeds-empty").is_none());
        assert!(window.try_find("bag-seed-priority-10").is_none());

        desktop.update(cx, |this, cx| {
            this.page_raw = serde_json::json!({"bagSeeds": []});
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("bag-seeds-empty").visible());
        assert!(window.try_find("bag-seeds-loading").is_none());
        assert!(window.try_find("bag-seeds-error").is_none());
    })
    .expect("render only the active loading, error, or empty state");
}
