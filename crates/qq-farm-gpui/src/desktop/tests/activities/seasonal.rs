use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn solar_terms_selects_a_term_and_claims_from_its_detail_panel(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "solarTerms".into();
            this.selected_account = Some("7".into());
            this.activities.solar_term_id = "6".into();
            this.page_raw = serde_json::json!({
                "actions": {"claimSolar": {"enabled": true}},
                "solarTerms": {
                    "currentTermId": 6,
                    "terms": [
                        {
                            "id": 6,
                            "name": "芒种",
                            "englishName": "Grain in Ear",
                            "description": "忙收忙种",
                            "claimable": true,
                            "rewards": [{"id": 1, "name": "种子", "count": 2}]
                        },
                        {"id": 7, "name": "夏至", "locked": true, "rewards": []}
                    ]
                }
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("solar-term-select-6").visible());
        assert!(window.find("solar-claim-selected").visible());
        assert_eq!(window.find("solar-claim-selected").label(), Some("领取"));
        assert!(window.try_find(("solar-select", 0_usize)).is_none());
        window.click("solar-term-select-7", cx);
        window.render_frame(cx);
        assert_eq!(window.find("solar-claim-selected").label(), Some("未开始"));
    })
    .expect("select a solar term and inspect its claim state");
    cx.update(|cx| assert_eq!(desktop.read(cx).activities.solar_term_id, "7"));
}

#[gpui_kit::test]
fn constellation_rules_open_in_a_dialog_without_changing_the_page_layout(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "constellation".into();
            this.selected_account = Some("7".into());
            this.activities.constellation_group_id = "alpha".into();
            this.page_raw = serde_json::json!({
                "actions": {"lightConstellation": {"enabled": true}},
                "constellation": {
                    "rules": {"title": "点亮规则", "paragraphs": ["每天可点亮星座"]},
                    "groups": [
                        {"id": "alpha", "name": "白羊座", "order": 1, "visualState": "lit", "rewards": [{"id": 1, "name": "星砂", "count": 3}]},
                        {"id": "beta", "name": "金牛座", "order": 2, "visualState": "lightable", "rewards": []}
                    ]
                }
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("constellation-group-alpha").visible());
        assert!(window.find("constellation-light-selected").visible());
        assert!(window.find("constellation-rules-toggle").visible());
        assert!(window.try_find("activity-rules").is_none());
        window.click("constellation-group-beta", cx);
        window.render_frame(cx);
        assert_eq!(window.find("constellation-light-selected").label(), Some("点亮观星"));
        window.click("constellation-rules-toggle", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert!(window.find("activity-rules").visible());
        assert_eq!(window.find("constellation-rules-toggle").label(), Some("活动说明"));
        window.press("escape", cx);
    })
    .expect("open constellation rules in a modal dialog");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        assert_eq!(window.find("constellation-rules-toggle").label(), Some("活动说明"));
    })
    .expect("Escape closes the rules dialog without changing the opener");
    cx.update(|cx| assert_eq!(desktop.read(cx).activities.constellation_group_id, "beta"));
}

#[gpui_kit::test]
fn season_pass_shows_progress_level_rewards_and_claim_all_gate(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "season".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "actions": {"claimPass": {"enabled": true}},
                "season": {
                    "pass": {
                        "level": "5",
                        "progress": 25,
                        "progressMax": 50,
                        "rules": {"title": "战令规则", "paragraphs": ["完成任务获得旅行积分"]},
                        "nodes": [
                            {"id": "4", "level": "4", "claimed": true, "locked": false, "rewards": [{"name": "星砂", "count": 2}]},
                            {"id": "5", "level": "5", "current": true, "claimable": true, "rewards": [{"name": "种子", "count": 1}]}
                        ]
                    }
                }
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("season-pass-claim").visible());
        assert_eq!(window.find("season-pass-claim").label(), Some("一键领取"));
        assert!(window.find("season-rules-toggle").visible());
        assert_eq!(window.find("activity-detail-refresh").label(), Some("刷新"));
        let revision = desktop.read(cx).request_revision;
        window.click("activity-detail-refresh", cx);
        assert_eq!(desktop.read(cx).request_revision, revision + 1);
        assert!(window.try_find("activity-rules").is_none());
        window.click("season-rules-toggle", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert!(window.find("activity-rules").visible());
        assert_eq!(window.find("season-rules-toggle").label(), Some("活动说明"));
        window.press("escape", cx);
    })
    .expect("open travel pass rules in a modal dialog");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!window.has_active_dialog(cx));
        assert_eq!(window.find("season-rules-toggle").label(), Some("活动说明"));
    })
    .expect("Escape closes the travel pass rules dialog");
}

#[gpui_kit::test]
fn season_pass_without_nodes_keeps_the_vue_claim_button_and_header_refresh(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "season".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "actions": {"claimPass": {"enabled": true}},
                "season": {}
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let claim = window.find("season-pass-claim");
        assert!(claim.visible());
        assert_eq!(claim.label(), Some("一键领取"));
        assert!(window.find("activity-detail-refresh").visible());
    })
    .expect("empty travel pass keeps the Vue claim button and header refresh");
}

#[gpui_kit::test]
fn stellar_seasonal_detail_controls_follow_english_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "season".into();
            this.selected_account = Some("7".into());
            this.activities.constellation_group_id = "alpha".into();
            this.activities.solar_term_id = "6".into();
            this.page_raw = serde_json::json!({
                "actions": {
                    "claimPass": {"enabled": true},
                    "claimSolar": {"enabled": true},
                    "lightConstellation": {"enabled": true}
                },
                "season": {
                    "pass": {
                        "level": 5,
                        "progress": 25,
                        "progressMax": 50,
                        "rules": {"title": "Rules", "paragraphs": ["Rule"]},
                        "nodes": [{"level": 5, "claimable": true, "rewards": []}]
                    }
                },
                "constellation": {
                    "rules": {"title": "Rules", "paragraphs": ["Rule"]},
                    "groups": [{"id": "alpha", "name": "Alpha", "visualState": "lightable", "rewards": []}]
                },
                "solarTerms": {
                    "terms": [
                        {"id": "6", "name": "Grain in Ear", "claimable": true, "rewards": []},
                        {"id": "7", "name": "Summer Solstice", "locked": true, "rewards": []}
                    ]
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("season-pass-claim").label(), Some("Claim All"));
        assert_eq!(window.find("season-rules-toggle").label(), Some("Event info"));
        assert_eq!(window.find("activity-detail-refresh").label(), Some("Refresh"));

        desktop.update(cx, |this, _| this.page_section = "constellation".into());
        window.render_frame(cx);
        assert_eq!(
            window.find("constellation-light-selected").label(),
            Some("Light Constellation")
        );
        assert_eq!(window.find("constellation-rules-toggle").label(), Some("Event info"));

        desktop.update(cx, |this, _| this.page_section = "solarTerms".into());
        window.render_frame(cx);
        assert_eq!(window.find("solar-claim-selected").label(), Some("Claim"));
        window.click("solar-term-select-7", cx);
        window.render_frame(cx);
        assert_eq!(window.find("solar-claim-selected").label(), Some("Not started"));
    })
    .expect("switch between English travel, constellation and solar term details");
}

#[test]
fn activity_buttons_follow_backend_action_gates() {
    let raw = serde_json::json!({
        "actions": {"claimPass": {"enabled": false}, "exchange": {"enabled": true}},
        "capabilities": {"lightConstellation": true}
    });
    assert_eq!(crate::desktop::activity_action_enabled(&raw, "claimPass"), Some(false));
    assert_eq!(crate::desktop::activity_action_enabled(&raw, "exchange"), Some(true));
    assert_eq!(crate::desktop::activity_action_enabled(&raw, "lightConstellation"), Some(true));
}
