use super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

fn snapshot() -> serde_json::Value {
    let now = 1_792_252_900_i64;
    serde_json::json!({
        "id": "2026110104",
        "title": "快乐不独享",
        "icon": "/activity-assets/autumn/happy.png",
        "serverTime": now * 1_000_i64,
        "startTime": (now - 86_400) * 1_000_i64,
        "endTime": (now + 30 * 86_400) * 1_000_i64,
        "active": true,
        "score": 45,
        "dailyReward": 5,
        "firstShareReward": 10,
        "claimedCount": 1,
        "claimLimit": 3,
        "poolClaimedCount": 2,
        "poolClaimLimit": 8,
        "canClaimDaily": true,
        "canShare": false,
        "firstShareAwarded": true,
        "canClaimMilestones": true,
        "milestones": [
            {"id": "40", "threshold": 40, "state": 2, "rewards": [{"id": 1002, "count": 20, "name": "点券", "image": ""}]}
        ]
    })
}

#[gpui_kit::test]
fn share_reward_renders_stats_buttons_and_milestones(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::ShareReward;
            this.page_section = "all".into();
            this.selected_account = Some("7".into());
            this.page_raw = snapshot();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("share-reward-refresh").visible());
        assert_eq!(window.find("share-reward-share").label(), Some("分享"));
        assert_eq!(window.find("share-reward-daily").label(), Some("领取每日奖励"));
        assert_eq!(window.find("share-reward-milestones").label(), Some("领取档位奖励"));
        assert_eq!(window.find(("share-reward-milestone", 0_usize)).label(), Some("40"));
        assert_eq!(window.find("share-reward-logs-guest").label(), Some("我送出的"));
    })
    .expect("share reward page renders stats, action buttons and milestones");
}
