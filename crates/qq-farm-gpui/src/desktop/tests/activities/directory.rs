use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{point, px, AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn charity_donation_requires_an_explicit_confirmation(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "charity".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "charity": {
                    "loveBalance": "18",
                    "actions": {"donateLove": {"enabled": true}}
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("charity-donate", cx);
        window.render_frame(cx);
        assert!(window.find("charity-donate-cancel").visible());
        assert!(window.find("charity-donate-confirm").visible());
        assert_eq!(window.find("charity-donate").label(), Some("确认捐赠"));
        desktop.update(cx, |this, cx| {
            this.activities.pending_actions.insert("charity-donate".into(), 1);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("charity-donate-confirm", cx);
        window.click("charity-donate", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).activities.charity_donation_pending);
        desktop.update(cx, |this, cx| {
            this.activities.pending_actions.remove("charity-donate");
            cx.notify();
        });
        window.render_frame(cx);
        window.click("charity-donate-cancel", cx);
    })
    .expect("confirm or cancel the charity donation");
    cx.update(|cx| {
        assert!(!desktop.read(cx).activities.charity_donation_pending);
    });
}

#[gpui_kit::test]
fn season_snapshot_does_not_render_the_pass_on_other_activity_tabs(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| desktop.update(cx, |this, _| {
        this.page = Page::Activities;
        this.page_section = "directory".into();
        this.selected_account = Some("7".into());
        this.page_raw = serde_json::json!({
            "activities": [
                {"id":"charity", "name":"公益小红花", "gameplayKey":"charity", "detailTarget":"charity"},
                {"id":"stellar", "name":"千星游记", "gameplayKey":"stellar", "detailTarget":"travel"}
            ],
            "charity":{"loveBalance":18,"actions":{"donateLove":{"enabled":true}}},
            "season":{"pass":{"level":1,"progress":5,"progressMax":10,"nodes":[{"level":1,"claimed":false,"claimable":true}]}}
        });
    }));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(("activity-directory-open", 1_usize)).visible());
        window.click_at(("activity-directory-open", 1_usize), point(px(280.), px(150.)), cx);
        window.render_frame(cx);
        assert!(window.find("activity-back-to-directory").visible());
        assert!(window.find("season-pass-claim").visible());
        window.click("activity-back-to-directory", cx);
        window.render_frame(cx);
        window.click(("activity-directory-open", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.find("charity-donate").visible());
        assert!(window.try_find("season-pass-claim").is_none());
    })
    .expect(
        "activity directory opens the selected detail and returns without leaking another view",
    );
}

#[gpui_kit::test]
fn activity_rules_only_render_in_the_matching_vue_detail(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "directory".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "activities": [{
                    "id":"stellar", "name":"千星游记", "gameplayKey":"stellar",
                    "detailTarget":"travel"
                }],
                "season": {"pass": {"rules": {"title":"战令规则", "paragraphs":["战令规则正文"]}}},
                "constellation": {"rules": {"title":"星座规则", "paragraphs":["星座规则正文"]}},
                "shop": {"goods": []},
                "petDiary": {"rules": {"title":"萌宠规则", "paragraphs":["萌宠规则正文"]}},
                "charity": {"loveBalance": 12,
                    "rules": {"title":"慈善规则", "paragraphs":["慈善规则正文"]}}
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("activity-rules").is_none());

        desktop.update(cx, |this, _| this.page_section = "shop".into());
        window.render_frame(cx);
        assert!(window.try_find("activity-rules").is_none());

        desktop.update(cx, |this, _| this.page_section = "petDiary".into());
        window.render_frame(cx);
        assert!(window.try_find("activity-rules").is_none());

        desktop.update(cx, |this, _| this.page_section = "charity".into());
        window.render_frame(cx);
        assert!(window.find("activity-rules").visible());
    })
    .expect("keep activity rules within the detail views that display them in Vue");
}

#[gpui_kit::test]
fn opening_activities_from_navigation_starts_at_the_activity_directory(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.set_page(Page::Activities, cx);
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "activities": [{
                    "id": "charity",
                    "name": "公益小红花",
                    "gameplayKey": "charity",
                    "detailTarget": "charity"
                }]
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(("activity-directory-open", 0_usize)).visible());
        assert!(window.try_find("activity-back-to-directory").is_none());
    })
    .expect("the activity page opens the Vue directory before a gameplay detail");
}

#[gpui_kit::test]
fn activity_directory_cards_localize_status_schedule_and_supported_actions(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let now = chrono::Local::now().timestamp_millis();
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "directory".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "activities": [
                    {
                        "id":"qixi", "name":"Magpie event", "gameplayKey":"qixi", "detailTarget":"qixi",
                        "startTime": now - 86_400_000, "endTime": now + 86_400_000
                    },
                    {
                        "id":"future", "name":"Future event", "gameplayKey":"future-mode", "detailTarget":"future",
                        "startTime": now + 86_400_000, "endTime": now + 172_800_000
                    }
                ]
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.set_language("en-US", window, cx);
            this.search.update(cx, |state, cx| state.set_value("event", window, cx));
        });
        window.render_frame(cx);
        let current = window.find(("activity-directory-open", 0_usize));
        assert!(current.visible());
        let current_label = current.label().expect("activity card has a screen-reader label");
        assert!(current_label.contains("Magpie event"), "card label was {current_label:?}");
        assert!(current_label.contains("Active"));
        assert!(current_label.contains("View details"));
        let current_status = window.find("activity-directory-status-0").bounds();
        let current_title = window.find("activity-directory-title-0").bounds();
        let current_period = window.find("activity-directory-period-0").bounds();
        let current_action = window.find("activity-directory-action-0").bounds();
        assert!(current_status.bottom() <= current_title.top());
        assert!(current_title.bottom() <= current_period.top());
        assert!(current_period.bottom() <= current_action.top());
        assert!(current_action.right() <= current.bounds().right());

        let unsupported = window.find(("activity-directory-open", 1_usize));
        let unsupported_label =
            unsupported.label().expect("unsupported activity explains its disabled state");
        assert!(unsupported_label.contains("Upcoming"));
        assert!(unsupported_label.contains("Details not supported"));
        let unsupported_period = window.find("activity-directory-period-1").bounds();
        let unsupported_action = window.find("activity-directory-action-1").bounds();
        assert!(unsupported_period.bottom() <= unsupported_action.top());
        assert!(unsupported_action.right() <= unsupported.bounds().right());
        window.click(("activity-directory-open", 1_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).page_section, "directory");
    })
    .expect("render accessible English activity cards and preserve support gating");
}

#[gpui_kit::test]
fn stellar_activity_cards_keep_the_current_subpage_unless_the_record_selects_one(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "directory".into();
            this.activities.stellar_section = "solarTerms".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "activities": [
                    {"id":"stellar-unknown", "name":"千星活动", "gameplayKey":"stellar", "detailTarget":"future-tab"},
                    {"id":"stellar-shop", "name":"千星商店", "gameplayKey":"stellar", "detailTarget":"shop"}
                ],
                "solarTerms":{"terms":[]},
                "shop":{"goods":[]}
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("activity-directory-open", 0_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).page_section, "solarTerms");

        window.click("activity-back-to-directory", cx);
        window.render_frame(cx);
        window.click(("activity-directory-open", 1_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).page_section, "shop");
    })
    .expect("stellar directory navigation preserves or applies its requested subpage");
}

#[gpui_kit::test]
fn stellar_detail_header_matches_vue_labels_badges_balance_and_remaining_time(
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
                "season": {
                    "title":"当前旅程",
                    "endTime": chrono::Local::now().timestamp() + 86_400,
                    "pass":{"nodes":[{"claimable":true}]}
                },
                "constellation":{"title":"观星礼录","groups":[{"id":"today","visualState":"lightable"}]},
                "shop":{"title":"星砂商店","balance":"42","goods":[{"id":"reward","exchangeable":true}]},
                "solarTerms":{"terms":[{"id":"rain","claimable":true}]},
                "actions":{"claimPass":{"enabled":true}}
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("activity-detail-tab", 0_usize)).label(), Some("千星游记 •"));
        assert_eq!(window.find(("activity-detail-tab", 1_usize)).label(), Some("观星礼录 •"));
        assert_eq!(window.find(("activity-detail-tab", 2_usize)).label(), Some("星砂商店 •"));
        assert_eq!(window.find(("activity-detail-tab", 3_usize)).label(), Some("节令小札 •"));
        assert!(window.find("activity-detail-title").visible());
        assert!(window.find("activity-shop-balance").visible());
        assert!(window.find("activity-detail-remaining").visible());

        desktop.update(cx, |this, cx| {
            this.page_raw["season"].as_object_mut().unwrap().remove("title");
            this.set_language("en-US", window, cx);
        });
        window.render_frame(cx);
        assert_eq!(window.find("activity-back-to-directory").label(), Some("Back to activities"));
        assert_eq!(window.find("activity-detail-title").label(), Some("Travel Pass"));
        assert_eq!(window.find(("activity-detail-tab", 0_usize)).label(), Some("Travel Pass •"));
        assert_eq!(window.find(("activity-detail-tab", 1_usize)).label(), Some("Constellation •"));
        assert_eq!(window.find(("activity-detail-tab", 2_usize)).label(), Some("Star Sand Shop •"));
        assert_eq!(window.find(("activity-detail-tab", 3_usize)).label(), Some("Solar Terms •"));
        assert!(window
            .find("activity-detail-remaining")
            .label()
            .is_some_and(|label| label.starts_with("Left:")));
    })
    .expect("stellar detail header exposes Vue-equivalent labels and actionable status");
}

#[gpui_kit::test]
fn switching_activity_details_keeps_in_flight_operation_and_feedback_state(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "season".into();
            this.activities.view_revision = 12;
            this.activities.pending_actions.insert("battle-pass".into(), 9);
            this.activities.action_notice = Some(("奖励已领取".into(), true));
            this.activities.action_notice_generation = 4;
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_page_section("constellation", window, cx));
    })
    .expect("switch from travel to constellation detail");
    cx.update(|cx| {
        let state = desktop.read(cx);
        assert_eq!(state.page_section, "constellation");
        assert_eq!(state.activities.view_revision, 12);
        assert_eq!(state.activities.pending_actions.get("battle-pass"), Some(&9));
        assert_eq!(
            state.activities.action_notice.as_ref().map(|(message, _)| message.as_str()),
            Some("奖励已领取")
        );
        assert_eq!(state.activities.action_notice_generation, 4);
    });
}
