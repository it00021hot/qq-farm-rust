use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use std::time::Duration;

#[gpui_kit::test]
fn settings_strategy_changes_stay_in_the_draft_until_saved(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.settings.draft = serde_json::json!({"plantingStrategy": "max_exp"});
            this.page_raw = this.settings.draft.clone();
            this.set_planting_strategy("preferred", cx);
        });
    });
    cx.update(|cx| {
        let desktop = desktop.read(cx);
        assert_eq!(desktop.settings.draft["plantingStrategy"], "preferred");
        assert_eq!(desktop.page_raw["plantingStrategy"], "preferred");
    });
}

#[gpui_kit::test]
fn settings_save_state_is_section_scoped_and_stale_completion_unlocks_the_page(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.settings.draft = serde_json::json!({
                "plantingStrategy": "max_exp",
                "automation": {"farm": true}
            });
            this.page_raw = this.settings.draft.clone();
            this.settings.strategy_saving = true;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_ne!(window.find("planting-strategy-select").disabled(), Some(true));
        desktop.update(cx, |this, cx| {
            this.page_section = "automation".into();
            this.settings.automation_saving = true;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("automation-switch-farm").visible());
        assert_ne!(window.find("automation-switch-farm").disabled(), Some(true));

        desktop.update(cx, |this, cx| {
            this.complete_settings_save("strategy", Ok(None), false, cx);
            assert!(!this.settings.strategy_saving);
            assert!(this.settings.automation_saving);
            assert!(!this.loading, "a stale save must not leave the global page locked");
            assert!(this.settings.save_notice.is_none());
        });
        window.render_frame(cx);
        assert!(window.find("automation-switch-farm").visible());
    })
    .expect("keep settings usable while another section is saving");
}

#[gpui_kit::test]
fn both_fertilizer_mode_shows_vue_ordering_hint_in_both_languages(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let snapshot = serde_json::json!({
        "automation": {
            "farm": true,
            "fertilizer": "smart",
            "fertilizer_buy_organic": false,
            "fertilizer_buy_normal": false
        }
    });
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "automation".into();
            this.selected_account = Some("gpui-settings-both-fertilizer".into());
            this.settings.draft = snapshot.clone();
            this.page_raw = snapshot;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("fertilizer-both-hint").is_none());
        assert!(window.try_find("fertilizer-buy-hint").is_none());
        assert_eq!(
            window.find("fertilizer-land-types-hint").label(),
            Some("施肥前会优先按土地类型过滤，仅对命中范围的地块执行施肥策略。")
        );
        assert_eq!(
            window.find("fertilizer-smart-seconds-hint").label(),
            Some("距离成熟时间 ≤ 此秒数时施有机肥（默认300秒=5分钟）")
        );
        window.click("fertilizer-mode-both", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("fertilizer-both-hint").label(),
            Some("每轮巡田：所有还能施无机肥的地各打 1 次；然后所有未成熟作物循环打有机肥直到成熟或肥料用尽。")
        );
        desktop.update(cx, |this, cx| {
            this.preferences.language = "en-US".into();
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("fertilizer-land-types-hint").label(),
            Some("Fertilizer runs only on lands matching the selected types.")
        );
        assert!(window.try_find("fertilizer-smart-seconds-hint").is_none());
        assert_eq!(
            window.find("fertilizer-both-hint").label(),
            Some("Each farm tick: apply inorganic once to every land that can take it, then loop organic on all immature crops until mature or fertilizer runs out.")
        );
        assert!(window.try_find("fertilizer-buy-hint").is_none());
        window.click("automation-switch-fertilizer_buy_organic", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("fertilizer-buy-hint").label(),
            Some("Organic and inorganic share the same flow: after fertilizing, remaining stock is checked and bought below threshold. Login and saving each trigger one check. When both are on, organic is preferred.")
        );
    })
    .expect("show the fertilizer ordering hint only for the both mode and localize it");
}

#[gpui_kit::test]
async fn automation_form_save_survives_a_fresh_store_load(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let account_id = "gpui-settings-roundtrip";
    let initial = serde_json::json!({
        "automation": {
            "farm": true,
            "farm_push": true,
            "land_upgrade": true,
            "friend": true,
            "friend_steal": false,
            "friend_help": false,
            "friend_bad": false,
            "task": true,
            "sell": true,
            "fertilizer": "smart",
            "fertilizer_land_types": ["gold"],
            "fertilizer_smart_seconds": 480,
            "friend_auto_accept": true
        },
        "plantingStrategy": "max_profit",
        "preferredSeedId": 1204,
        "intervals": {
            "farmMin": 23,
            "farmMax": 29,
            "helpMin": 31,
            "helpMax": 37,
            "stealMin": 61,
            "stealMax": 83
        },
        "friendQuietHours": {
            "enabled": true,
            "start": "22:30",
            "end": "06:15",
            "continueFarm": false
        },
        "bagSeedPriority": [1204, 1205],
        "bagSeedFallbackStrategy": "level",
        "bagSeedMultiLandReservationEnabled": true,
        "plantBlacklist": [1206, 1207],
        "autoAcceptFriendMinLevel": 18,
        "autoAcceptRequireOwnLevel": true,
        "autoAcceptHarvestStealEnabled": true,
        "autoAcceptHarvestStealHarvest": 12,
        "autoAcceptHarvestStealSteal": 3
    });

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "automation".into();
            this.selected_account = Some(account_id.into());
            this.accounts.records = vec![(account_id.into(), "设置回归账号".into(), true)];
            this.settings.draft = initial.clone();
            this.page_raw = initial.clone();
            this.rows = crate::pages::page_data(Page::Settings, initial.clone()).rows;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("automation-switch-farm", cx);
        window.click("automation-switch-friend_steal", cx);
        window.render_frame(cx);
        {
            let state = desktop.read(cx);
            assert_eq!(state.settings.draft["automation"]["farm"], false);
            assert_eq!(state.settings.draft["automation"]["friend_steal"], true);
        }
        assert!(window.find("save-automation-settings").visible());
        window.render_frame(cx);
    })
    .expect("edit automation controls and expose the selected account save action");

    let (app, snapshot) = cx.update(|cx| {
        let state = desktop.read(cx);
        (state.app.clone(), state.settings.draft.clone())
    });
    qq_farm_app::settings::save_settings(&app, account_id, snapshot)
        .expect("save the automation form through the app facade");

    let store_path = qq_farm_core::models::store::global_config::store_file();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let persisted = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|store| store.get("accountConfigs").cloned())
            .and_then(|configs| configs.get(account_id).cloned())
            .is_some_and(|config| {
                config.pointer("/automation/farm").and_then(serde_json::Value::as_bool)
                    == Some(false)
                    && config
                        .pointer("/automation/friend_steal")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
            });
        if persisted {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "settings did not reach store.json");
        std::thread::sleep(Duration::from_millis(10));
    }

    qq_farm_core::models::store::global_config::load_global_config()
        .expect("reload the persisted settings store");
    let restored = qq_farm_app::settings::settings_panel(account_id, "");
    assert_eq!(restored["automation"]["farm"], false);
    assert_eq!(restored["automation"]["friend_steal"], true);
    assert_eq!(restored["plantingStrategy"], "max_profit");
    assert_eq!(restored["preferredSeed"], 1204);
    assert_eq!(restored["intervals"]["farmMin"], 23);
    assert_eq!(restored["friendQuietHours"]["start"], "22:30");
    assert_eq!(restored["friendQuietHours"]["continue_farm"], false);
    assert_eq!(restored["bagSeedPriority"], serde_json::json!([1204, 1205]));
    assert_eq!(restored["plantBlacklist"], serde_json::json!([1206, 1207]));
    assert_eq!(restored["autoAcceptFriendMinLevel"], 18);
    assert_eq!(restored["autoAcceptHarvestStealSteal"], 3);

    let reloaded_page =
        crate::pages::load_page_for_mall_slot(&app, Page::Settings, account_id, 0, "strategy")
            .await
            .expect("reload settings through the GPUI page loader");
    assert_eq!(reloaded_page.raw["friendQuietHours"]["continueFarm"], false);
    assert!(reloaded_page.raw["friendQuietHours"].get("continue_farm").is_none());

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page_section = "strategy".into();
            this.settings.draft = reloaded_page.raw.clone();
            this.page_raw = reloaded_page.raw.clone();
            cx.notify();
        });
        window.render_frame(cx);
        window.click("quiet-hours-continue-farm", cx);
        let state = desktop.read(cx);
        assert_eq!(state.settings.draft["friendQuietHours"]["continueFarm"], true);
    })
    .expect("the reloaded quiet-hours switch starts off and toggles on");
}

#[gpui_kit::test]
fn saving_strategy_does_not_persist_or_discard_unsaved_automation_edits(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_, desktop, _restore_data_dir) = open_test_desktop(cx);
    let account_id = "gpui-settings-section-save";
    let app = cx.update(|cx| desktop.read(cx).app.clone());
    qq_farm_app::settings::save_settings(
        &app,
        account_id,
        serde_json::json!({"automation": {"farm": true, "friend_steal": false}}),
    )
    .expect("seed the persisted automation settings");

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some(account_id.into());
            this.loading = false;
            this.accounts.records = vec![(account_id.into(), "Settings account".into(), true)];
            this.settings.draft = serde_json::json!({
                "plantingStrategy": "max_profit",
                "intervals": {"farm": 20},
                "automation": {"farm": false, "friend_steal": true}
            });
            this.page_raw = this.settings.draft.clone();
            this.save_settings(cx);
        });
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while cx.update(|cx| desktop.read(cx).loading) {
        cx.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "the strategy save should finish");
        std::thread::sleep(Duration::from_millis(10));
    }

    let (draft, loading) = cx.update(|cx| {
        let state = desktop.read(cx);
        (state.settings.draft.clone(), state.loading)
    });
    assert!(!loading);
    assert_eq!(draft["automation"]["farm"], false, "hidden automation edits stay in the draft");
    assert_eq!(draft["automation"]["friend_steal"], true);

    let persisted = qq_farm_app::settings::settings_panel(account_id, "local");
    assert_eq!(persisted["plantingStrategy"], "max_profit");
    assert_eq!(persisted["intervals"]["farm"], 20);
    assert_eq!(
        persisted["automation"]["farm"], true,
        "the strategy save must not write automation"
    );
    assert_eq!(persisted["automation"]["friend_steal"], false);
}

#[gpui_kit::test]
fn disabled_auto_accept_keeps_its_threshold_fields_visible_but_inactive(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let snapshot = serde_json::json!({
        "automation": {
            "friend": true,
            "friend_auto_accept": false
        },
        "autoAcceptFriendMinLevel": 18,
        "autoAcceptRequireOwnLevel": true,
        "autoAcceptHarvestStealEnabled": true,
        "autoAcceptHarvestStealHarvest": 12,
        "autoAcceptHarvestStealSteal": 3
    });

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "automation".into();
            this.selected_account = Some("gpui-settings-friend-gates".into());
            this.settings.draft = snapshot.clone();
            this.page_raw = snapshot;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        for id in [
            "automation-switch-friend_auto_accept",
            "settings-number-autoAcceptFriendMinLevel",
            "settings-switch-autoAcceptRequireOwnLevel",
            "settings-switch-autoAcceptHarvestStealEnabled",
            "settings-number-autoAcceptHarvestStealHarvest",
            "settings-number-autoAcceptHarvestStealSteal",
        ] {
            assert!(window.find(id).visible(), "Vue keeps {id} visible while auto-accept is off");
        }
        assert_eq!(
            window.find("settings-auto-accept-hint").label(),
            Some("开启后自动通过好友申请；可按等级与生涯收偷比过滤（收偷比 = 偷菜份数 / 收获份数）。")
        );
        // 禁用态的数字输入框点击无效果（Tauri NInputNumber 同款 disabled）
        window.click("settings-number-autoAcceptFriendMinLevel", cx);
        window.click("settings-switch-autoAcceptRequireOwnLevel", cx);
        let state = desktop.read(cx);
        assert!(
            state.settings.edit_path.is_none(),
            "disabled numeric field must not open its editor"
        );
        assert_eq!(state.settings.draft["autoAcceptRequireOwnLevel"], true);
        assert_eq!(state.settings.draft["autoAcceptFriendMinLevel"], 18, "禁用态点击不得改动阈值");
        desktop.update(cx, |this, cx| {
            this.preferences.language = "en-US".into();
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find("auto-accept-min-level-hint").label(), Some("0 = no limit"));
        assert_eq!(
            window.find("settings-auto-accept-hint").label(),
            Some("Friend requests are accepted automatically; filter by level and career steal/harvest ratio (ratio = steal / harvest).")
        );
    })
    .expect("keep Vue's visible-but-disabled auto-accept thresholds");
}
