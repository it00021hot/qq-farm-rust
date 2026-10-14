use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn charity_actions_localize_and_follow_claim_pending_states(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "charity".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "charity": {
                    "love": {"name": "Love"},
                    "loveBalance": "18",
                    "donatedLove": "4",
                    "settlement": {"requiredLove": "10", "eligible": false, "reward": {"name": "Reward"}},
                    "globalProgress": {"donated": "120", "target": "200", "rewardTarget": "150", "reward": {"name": "Flower"}},
                    "seedReward": {"claimed": false, "reward": {"name": "Seeds"}},
                    "dailyGift": {"claimed": false, "reward": {"name": "Gift"}},
                    "progressRewards": [
                        {"target": "100", "reached": false, "claimed": false, "claimSupported": true, "claimable": true, "reward": {"name": "Bonus", "count": 1}},
                        {"target": "200", "reached": false, "claimed": false, "claimSupported": true, "claimable": true, "reward": {"name": "Bonus", "count": 2}}
                    ],
                    "actions": {
                        "claimSeeds": {"enabled": false},
                        "donateLove": {"enabled": true},
                        "claimDailyGift": {"enabled": true}
                    }
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);

        assert_eq!(window.find("charity-seeds").label(), Some("Unavailable"));
        assert_eq!(window.find("charity-daily-gift").label(), Some("Claim gift"));
        assert_eq!(window.find("charity-donate").label(), Some("Donate all love"));
        assert_eq!(window.find(("charity-progress-claim", 0_usize)).label(), Some("Claim reward"));

        desktop.update(cx, |this, cx| {
            this.activities.pending_actions.insert("charity-seeds".into(), 1);
            this.activities.pending_actions.insert("charity".into(), 1);
            this.activities.pending_actions.insert("charity-progress:100".into(), 1);
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find("charity-seeds").label(), Some("Claiming…"));
        assert_eq!(window.find("charity-daily-gift").label(), Some("Claiming…"));
        assert_eq!(window.find(("charity-progress-claim", 0_usize)).label(), Some("Claiming…"));
        assert_eq!(window.find(("charity-progress-claim", 1_usize)).label(), Some("Claiming…"));
        let pending_before_clicks = desktop.read(cx).activities.pending_actions.clone();
        window.click("charity-seeds", cx);
        window.click("charity-daily-gift", cx);
        window.click(("charity-progress-claim", 1_usize), cx);
        assert_eq!(desktop.read(cx).activities.pending_actions, pending_before_clicks);
    })
    .expect("render charity action labels and pending states in English");
}

#[gpui_kit::test]
fn charity_rules_collapse_and_daily_gift_claimed_state_follow_vue(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "charity".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "charity": {
                    "love": {"name": "爱心"},
                    "loveBalance": "18",
                    "donatedLove": "4",
                    "settlement": {"requiredLove": "10", "eligible": true},
                    "globalProgress": {"donated": "120", "target": "200", "reward": {"name": "小红花", "count": 3}},
                    "seedReward": {"claimed": false, "reward": {"name": "种子", "count": 1}},
                    "dailyGift": {"claimed": true, "reward": {"name": "礼包", "count": 2}},
                    "progressRewards": [],
                    "rules": {
                        "title": "公益规则",
                        "paragraphs": ["规则一"],
                        "tips": {"txt": ["规则二"]},
                        "lines": ["规则三"]
                    },
                    "actions": {
                        "claimSeeds": {"enabled": false},
                        "donateLove": {"enabled": false},
                        "claimDailyGift": {"enabled": false}
                    }
                }
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("charity-rules-toggle").label(), Some("⌄ 公益规则"));
        assert!(window.find("charity-rules-body").visible());
        assert_eq!(window.find("charity-gift-claimed").label(), Some("今日已领取"));
        window.click("charity-rules-toggle", cx);
        window.render_frame(cx);
        assert_eq!(window.find("charity-rules-toggle").label(), Some("› 公益规则"));
        assert!(window.try_find("charity-rules-body").is_none());
        window.click("charity-rules-toggle", cx);
        window.render_frame(cx);
        assert!(window.find("charity-rules-body").visible());
    })
    .expect("render and collapse the Vue charity rules section");
}
