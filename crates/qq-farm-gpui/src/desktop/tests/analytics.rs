use super::super::Page;
use super::support::open_test_desktop;
use gpui_kit::component::combobox::ComboboxEvent;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, Entity, Focusable, TestAppContext};

#[gpui_kit::test]
fn analytics_without_an_account_uses_vue_select_account_copy(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Analytics;
            this.page_section = "strategy".into();
            this.selected_account = None;
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("analytics-no-account").label(), Some("请选择账号"));
    })
    .expect("analytics uses the Vue select-account copy without an account");
}

fn wait_for_blacklist_save(cx: &mut TestAppContext, desktop: &Entity<super::super::FarmDesktop>) {
    cx.run_until_parked();
    assert!(!cx.update(|cx| desktop.read(cx).analytics.blacklist_save_pending));
}

#[gpui_kit::test]
fn analytics_page_load_syncs_the_reference_level_to_the_account(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Analytics;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
        });
    });
    // The page snapshot arrives outside any window update, exactly like the
    // background task that calls this in production.
    cx.update(|cx| {
        let revision = desktop.read(cx).request_revision;
        let raw = serde_json::json!({
            "accountLevel": 96,
            "plantBlacklist": [],
            "configuration": {"plantingStrategy": "max_exp", "preferredSeed": 11},
            "rankings": []
        });
        let data = crate::pages::page_data(Page::Analytics, raw);
        desktop.update(cx, |this, cx| {
            this.apply_page_load_result(
                revision,
                Page::Analytics,
                0,
                "7".into(),
                None,
                Ok(data),
                cx,
            );
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let state = desktop.read(cx);
        assert_eq!(
            state.analytics.level.read(cx).value(),
            "96",
            "the reference level must follow the loaded account level like Vue's syncStrategyLevel"
        );
        assert!(!state.analytics.level_is_user_override);
    });

    // A notification that does not change the text must not count as a user edit.
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.analytics.level.update(cx, |_, cx| cx.notify());
        });
    });
    cx.run_until_parked();
    cx.update(|cx| assert!(!desktop.read(cx).analytics.level_is_user_override));

    // Typing still takes over the reference level until the next page load.
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.analytics.level.update(cx, |input, cx| input.set_value("12", window, cx));
        });
    })
    .expect("edit the reference level");
    cx.run_until_parked();
    cx.update(|cx| assert!(desktop.read(cx).analytics.level_is_user_override));
}

#[gpui_kit::test]
fn analysis_initial_level_is_automatic_until_the_user_edits_it(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            assert!(!this.analytics.level_is_user_override);
            assert_eq!(this.analytics.level.read(cx).value(), "1");
            this.analytics.level.update(cx, |input, cx| input.set_value("18", window, cx));
        });
    })
    .expect("edit reference level");
    cx.run_until_parked();
    cx.update(|cx| assert!(desktop.read(cx).analytics.level_is_user_override));
}

#[gpui_kit::test]
fn search_shortcut_focuses_reference_level_on_analytics_strategy(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Analytics;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "accountLevel": 18,
                "plantBlacklist": [],
                "configuration": {"plantingStrategy": "max_exp"},
                "rankings": []
            });
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.press("secondary-f", cx);
    })
    .expect("dispatch the analytics search shortcut");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let level = desktop.read(cx).analytics.level.read(cx).focus_handle(cx);
        assert!(level.is_focused(window));
    })
    .expect("focus the analytics page's reference-level filter");
}

#[gpui_kit::test]
fn analytics_blacklist_selector_persists_multiselect_changes_without_refresh(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
            desktop.update(cx, |this, _| {
                this.page = Page::Analytics;
                this.page_section = "strategy".into();
                this.selected_account = Some("7".into());
                this.accounts.records = vec![("7".into(), "测试账号".into(), true)];
                this.analytics.level_is_user_override = false;
                this.page_raw = serde_json::json!({
                    "accountLevel": 18,
                    "plantBlacklist": [],
                    "configuration": {"plantingStrategy": "max_exp", "preferredSeed": 11},
                    "rankings": [
                        {"seedId": 11, "name": "小麦", "level": 3, "seasons": 1, "image": "/game-config/seed_images_named/seed_images/20218.webp", "expPerHour": 15, "profitPerHour": 10, "normalFertilizerExpPerHour": 20, "normalFertilizerProfitPerHour": 8},
                        {"seedId": 12, "name": "草莓", "level": 22, "seasons": 2, "expPerHour": 30, "profitPerHour": 20, "normalFertilizerExpPerHour": 25, "normalFertilizerProfitPerHour": 18}
                    ]
                });
                this.rows = crate::pages::page_data(Page::Analytics, this.page_raw.clone()).rows;
            });
        });

    let mut revision = 0;
    let mut config_revision = 0;
    let other_account_blacklist = qq_farm_app::farm::plant_blacklist("8");
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_plant_blacklist_selector(window, cx));
        let selector_state = desktop.read(cx).analytics.blacklist_selector.clone();
        selector_state.update(cx, |selector, cx| {
            selector.set_selected_values(&[11], window, cx);
            assert_eq!(selector.selected_values(), vec![11]);
            selector.set_selected_values(&[], window, cx);
        });
        window.render_frame(cx);
        assert!(window.find(("page-section", 0_usize)).visible());
        window.click(("page-section", 1_usize), cx);
        desktop.update(cx, |this, cx| this.sync_plant_blacklist_selector(window, cx));
        window.render_frame(cx);
        let selector_id = desktop.read(cx).analytics.blacklist_selector.entity_id();
        let selector = ("multi-combo-box", selector_id);
        assert!(window.find(selector).visible());
        assert!(window.find("blacklist-add-all").visible());
        revision = desktop.read(cx).request_revision;
        config_revision = desktop.read(cx).app.engine.runtime_state().config_revision();
        let selector_state = desktop.read(cx).analytics.blacklist_selector.clone();
        selector_state.update(cx, |selector, cx| {
            let selected = vec![11, 12];
            selector.set_selected_values(&selected, window, cx);
            cx.emit(ComboboxEvent::Change(selected));
        });
    })
    .expect("emit a multi-select change");
    wait_for_blacklist_save(cx, &desktop);

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let desktop_state = desktop.read(cx);
        assert_eq!(
            desktop_state.analytics.blacklist_selector.read(cx).selected_values(),
            vec![11, 12]
        );
        assert_eq!(desktop_state.page_raw["plantBlacklist"], serde_json::json!([11, 12]));
        assert_eq!(desktop_state.request_revision, revision);
        assert_eq!(desktop_state.page_section, "plantBlacklist");
        assert!(!desktop_state.analytics.blacklist_save_pending);
        assert_eq!(desktop_state.app.engine.runtime_state().config_revision(), config_revision);
    })
    .expect("persist multiselect changes from the blacklist picker");

    assert_eq!(qq_farm_app::farm::plant_blacklist("7"), serde_json::json!([11, 12]));
    assert_eq!(qq_farm_app::farm::plant_blacklist("8"), other_account_blacklist);
}

#[gpui_kit::test]
fn analytics_blacklist_matches_vue_semantics_and_localizes_real_updates(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _cx| {
            this.page = Page::Analytics;
            this.page_section = "plantBlacklist".into();
            this.selected_account = Some("7".into());
            this.preferences.language = "en-US".into();
            this.page_raw = serde_json::json!({
                "accountLevel": "18",
                "plantBlacklist": ["11"],
                "configuration": {"plantingStrategy": "max_profit"},
                "rankings": [{
                    "seedId": "11", "name": "Wheat", "level": "3", "expPerHour": "12.5",
                    "profitPerHour": "9", "normalFertilizerExpPerHour": "16",
                    "normalFertilizerProfitPerHour": "7"
                }]
            });
            this.rows = crate::pages::page_data(Page::Analytics, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.set_language("en-US", window, cx);
            this.sync_plant_blacklist_selector(window, cx);
        });
        window.render_frame(cx);
        assert!(window.find("analytics-blacklist-tab-count").visible());
        assert_eq!(window.find(("page-section", 1_usize)).label(), Some("Plant blacklist, 1"));
        assert_eq!(window.find("blacklist-add-all").label(), Some("Add all to blacklist"));
        assert_eq!(window.find("blacklist-clear-all").label(), Some("Clear blacklist"));
        assert_eq!(window.find("analytics-blacklist-remove-11").label(), Some("Remove"));
        assert_eq!(
            desktop.read(cx).analytics.level.read(cx).presentation().placeholder().as_ref(),
            "Reference level"
        );
        assert_eq!(
            desktop.read(cx).analytics.blacklist_selector.read(cx).selected_values(),
            vec![11]
        );
        window.click("analytics-blacklist-remove-11", cx);
    })
    .expect("start removing a blacklist entry");
    wait_for_blacklist_save(cx, &desktop);

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("analytics-blacklist-tab-count").is_none());
        assert_eq!(window.find(("page-section", 1_usize)).label(), Some("Plant blacklist"));
        assert!(window.find("analytics-blacklist-feedback").visible());
        assert_eq!(desktop.read(cx).page_raw["plantBlacklist"], serde_json::json!([]));
        assert!(!desktop.read(cx).analytics.blacklist_feedback_is_error);
        assert_eq!(
            desktop.read(cx).analytics.blacklist_feedback.as_deref(),
            Some("Blacklist updated")
        );
    })
    .expect(
        "English blacklist controls update the selected account configuration and show feedback",
    );

    assert_eq!(qq_farm_app::farm::plant_blacklist("7"), serde_json::json!([]));
}

#[gpui_kit::test]
fn analytics_blacklist_save_failure_keeps_previous_selection_and_shows_error(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Analytics;
            this.page_section = "plantBlacklist".into();
            this.selected_account = Some(String::new());
            this.page_raw = serde_json::json!({
                "plantBlacklist": [11],
                "rankings": [{"seedId": 11, "name": "Wheat", "level": 3}]
            });
        });
    });

    cx.update_window(window, |_, _, cx| {
        desktop.update(cx, |this, cx| this.replace_plant_blacklist(vec![], cx));
    })
    .expect("start the account-scoped blacklist save");
    wait_for_blacklist_save(cx, &desktop);

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert!(!state.analytics.blacklist_save_pending);
        assert!(state.analytics.blacklist_feedback_is_error);
        assert_eq!(state.page_raw["plantBlacklist"], serde_json::json!([11]));
        assert!(state
            .analytics
            .blacklist_feedback
            .as_deref()
            .is_some_and(|message| message.contains("Missing account id")));
    })
    .expect("failed saves preserve the old value and show the app error");
}

#[gpui_kit::test]
fn analytics_strategy_view_uses_localized_comparison_sections(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Analytics;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "accountLevel": "18",
                "configuration": {"plantingStrategy": "max_profit"},
                "rankings": [{
                    "seedId": "11", "name": "Wheat", "level": "3", "seasons": "2",
                    "expPerHour": "12.5", "profitPerHour": "9",
                    "normalFertilizerExpPerHour": "16", "normalFertilizerProfitPerHour": "7"
                }]
            });
            this.rows = crate::pages::page_data(Page::Analytics, this.page_raw.clone()).rows;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert!(window.find("analytics-current-strategy").visible());
        assert!(window.find("analytics-strategy-comparison").visible());
        assert_eq!(
            desktop.read(cx).analytics.level.read(cx).presentation().placeholder().as_ref(),
            "Reference level"
        );
    })
    .expect("localized strategy comparison renders all metric cards for the selected account");
}

#[gpui_kit::test]
fn analytics_reference_level_matches_vue_for_fractional_and_cleared_values(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Analytics;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.analytics.level_is_user_override = false;
            this.page_raw = serde_json::json!({
                "accountLevel": 18,
                "configuration": {"plantingStrategy": "max_exp"},
                "plantBlacklist": [],
                "rankings": [
                    {"seedId": 1, "name": "Low", "level": 1, "expPerHour": 10},
                    {"seedId": 2, "name": "Fractional", "level": 12.5, "expPerHour": 20},
                    {"seedId": 3, "name": "Unknown level", "level": null, "expPerHour": 5},
                    {"seedId": 4, "name": "High", "level": 13, "expPerHour": 100}
                ]
            });
            this.rows = crate::pages::page_data(Page::Analytics, this.page_raw.clone()).rows;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("analytics-current-plant-4").visible());
    })
    .expect("start from the synchronized account level");

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.analytics.level.update(cx, |input, cx| input.set_value("12.5", window, cx));
        });
    })
    .expect("set a fractional reference level");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("analytics-current-plant-2").visible());
        assert!(window.try_find("analytics-current-plant-4").is_none());
    })
    .expect("retain fractional levels when filtering candidate crops");

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.analytics.level.update(cx, |input, cx| input.set_value("", window, cx));
        });
    })
    .expect("clear the reference level");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("analytics-current-plant-3").visible());
        assert!(window.try_find("analytics-current-plant-1").is_none());
        assert!(window.try_find("analytics-current-plant-2").is_none());
        assert!(window.try_find("analytics-current-plant-4").is_none());
    })
    .expect("a cleared reference level follows Vue's zero fallback");
}
