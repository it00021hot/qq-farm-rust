use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn qixi_uses_stage_cards_searchable_friend_picker_and_balance_limited_gifting(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "qixi".into();
            this.selected_account = Some("7".into());
            this.activities.qixi_friends_loading = true;
            this.page_raw = serde_json::json!({
                "qixi": {
                    "title": "七夕鹊桥",
                    "active": true,
                    "startTime": 1_790_000_000_000_i64,
                    "endTime": 1_790_086_400_000_i64,
                    "feather": {"name": "羽毛"},
                    "sachet": {"name": "香囊"},
                    "balances": {"known": true, "feather": "80", "sachet": "3", "receivedSachet": "2"},
                    "gift": {"sentCount": "4"},
                    "bridge": {
                        "currentStage": 2,
                        "claimable": true,
                        "stages": [
                            {"stage": 1, "completed": true, "cost": {"name": "羽毛", "count": 20}, "rewards": [{"name": "钻石", "count": 5}]},
                            {"stage": 2, "current": true, "claimable": true, "cost": {"name": "羽毛", "count": 30}, "rewards": [{"name": "香囊", "count": 1}]}
                        ]
                    },
                    "actions": {"bridge": {"enabled": true}, "gift": {"enabled": true}},
                    "rules": {"title": "活动规则", "paragraphs": ["赠送香囊可增加好友进度。"]}
                },
                "activityFriends": [
                    {"gid": 123, "nickname": "小明", "level": 12},
                    {"gid": 456, "nickname": "Alice", "level": 8}
                ]
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("qixi-balance-label", 0_usize)).label(), Some("羽毛"));
        assert_eq!(window.find(("qixi-balance-label", 2_usize)).label(), Some("获赠香囊"));
        assert_eq!(window.find("qixi-sent-label").label(), Some("已赠好友"));
        assert_eq!(window.find(("qixi-stage-status", 1_usize)).label(), Some("待领取"));
        assert!(window.find("qixi-bridge-claim").visible());
        assert!(window.find("qixi-friend-search").visible());
        assert!(window.find("qixi-friend-123").visible());
        assert!(window.find("qixi-friend-456").visible());
        assert!(window.try_find("qixi-select-friend").is_none());

        window.click("qixi-friend-search", cx);
        window.input("123", cx);
        window.render_frame(cx);
        assert!(window.try_find("qixi-friend-123").is_none());
        assert!(window.try_find("qixi-friend-456").is_none());
        desktop.update(cx, |this, cx| {
            this.activities.qixi_friend_search.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
        });
        window.render_frame(cx);
        assert!(window.find("qixi-friend-123").visible());
        assert!(window.find("qixi-friend-456").visible());

        window.click("qixi-friend-search", cx);
        window.input("ali", cx);
        window.render_frame(cx);
        assert!(window.find("qixi-friend-456").visible());
        assert!(window.try_find("qixi-friend-123").is_none());
        window.click("qixi-friend-456", cx);
        assert_ne!(window.find("qixi-gift-sachet").disabled(), Some(true));
        assert_ne!(window.find("qixi-bridge-claim").disabled(), Some(true));
        assert!(desktop.read(cx).activities.qixi_friends_loading);
    })
    .expect("search and choose a friend in the dedicated Qixi view");

    cx.update(|cx| {
        assert_eq!(desktop.read(cx).activities.qixi_selected_friend_gid, "456");
        assert_eq!(desktop.read(cx).activities.qixi_gift_count.read(cx).value(), "1");
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("qixi-bridge-claim").label(), Some("Claim bridge reward"));
        assert_eq!(window.find("qixi-gift-sachet").label(), Some("Gift sachet"));
        assert_eq!(window.find("qixi-gift-count-max").label(), Some("All"));
        assert_eq!(window.find("qixi-refresh-friends").label(), Some("Refresh List"));
        assert_eq!(window.find("qixi-rules-toggle").label(), Some("Collapse"));
        window.click("qixi-rules-toggle", cx);
        window.render_frame(cx);
        assert_eq!(window.find("qixi-rules-toggle").label(), Some("Expand"));
        desktop.update(cx, |this, cx| {
            this.activities.pending_actions.insert("qixi-gift".into(), 1);
            this.activities.pending_actions.insert("qixi-bridge".into(), 1);
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find("qixi-gift-sachet").label(), Some("Claiming…"));
        assert_eq!(window.find("qixi-bridge-claim").label(), Some("Claiming…"));
    })
    .expect("show Qixi stage and friend-gifting controls in English");
}

#[gpui_kit::test]
fn qixi_balance_labels_follow_vue_item_and_fixed_fallbacks(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "qixi".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "qixi": {
                    "title": "七夕鹊桥",
                    "feather": {},
                    "sachet": {"name": "数字 5"},
                    "receivedSachet": {"name": "收到的其他名字"},
                    "balances": {"known": true, "feather": "80", "sachet": "3", "receivedSachet": "2"},
                    "gift": {"sentCount": "4"},
                    "bridge": {"stages": []},
                    "actions": {"bridge": {"enabled": false}, "gift": {"enabled": false}}
                },
                "activityFriends": []
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("qixi-balance-label", 0_usize)).label(), Some("鹊羽"));
        assert_eq!(window.find(("qixi-balance-label", 1_usize)).label(), Some("数字 5"));
        assert_eq!(
            window.find(("qixi-balance-label", 2_usize)).label(),
            Some("获赠香囊"),
            "the received sachet card uses the fixed Vue label"
        );
        assert_eq!(window.find("qixi-sent-label").label(), Some("已赠好友"));
    })
    .expect("render Qixi balance labels with Vue item and fixed fallbacks");
}
