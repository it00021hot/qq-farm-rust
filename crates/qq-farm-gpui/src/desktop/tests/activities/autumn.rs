use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn autumn_happy_directory_entry_opens_happy_view_and_schedules_log_load(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "directory".into();
            this.selected_account = None;
            this.page_raw = serde_json::json!({
                "activities": [
                    {"id":"wish", "name":"秋祈良愿", "gameplayKey":"autumnWish", "detailTarget":"wish"},
                    {"id":"happy", "name":"快乐不独享", "gameplayKey":"autumnHappy", "detailTarget":"happy"}
                ],
                "autumn": {
                    "wish": {"title":"秋祈良愿", "choices":[{"id":1,"name":"丰收"}], "canDraw":true},
                    "happy": {
                        "title":"快乐不独享",
                        "score": 18,
                        "canClaimDaily": true,
                        "canShare": false,
                        "canClaimMilestones": false,
                        "logs": {"total": 0, "logs": []}
                    }
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            let record = this.page_raw["activities"][1].clone();
            this.open_activity_record(&record, window, cx);
            this.selected_account = Some("7".into());
        });
        window.render_frame(cx);
        assert!(window.find("autumn-happy-daily").visible());
        assert!(window.try_find("autumn-draw").is_none());
        assert!(window.try_find("autumn-mode-wish").is_none());
        assert!(window.try_find("autumn-mode-happy").is_none());
        assert!(window.find("autumn-refresh").visible());
        assert!(window.find("activity-back-to-directory").visible());
    })
    .expect("open the selected autumn gameplay from the directory");
    cx.update(|cx| {
        let activities = &desktop.read(cx).activities;
        assert_eq!(activities.autumn_mode, "happy");
        assert!(activities.autumn_logs_load_pending);
    });
}

#[gpui_kit::test]
fn autumn_wish_renders_choices_pending_rewards_and_daily_rewards(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "autumn".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "autumn": {
                    "wish": {
                        "title": "秋祈良愿",
                        "active": true,
                        "startTime": 1_790_000_000_000_i64,
                        "endTime": 1_790_086_400_000_i64,
                        "rules": ["每天可以祈愿一次"],
                        "day": 3,
                        "remaining": 1,
                        "choices": [{"id": 12, "name": "丰收"}, {"id": 34, "name": "成长"}],
                        "canDraw": true,
                        "canClaim": false,
                        "pending": {
                            "day": 2,
                            "text": "祈愿结果",
                            "rewards": [{"name": "秋日种子", "count": 2}]
                        },
                        "rewardDays": [{"day": 1, "reward": {"name": "秋日果实", "count": 1}}]
                    }
                }
            });
            this.activities.autumn_choice_id = "12".into();
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert!(window.try_find("autumn-mode-wish").is_none());
        assert_eq!(window.find("autumn-wish-choice-12").role(), Some(gpui_kit::Role::RadioButton));
        assert!(window.find("autumn-wish-choice-12").visible());
        assert!(window.find("autumn-wish-choice-34").visible());
        assert!(window.find("autumn-draw").visible());
        assert!(window.find("autumn-claim").visible());
        assert!(window.try_find("autumn-mode-happy").is_none());
        assert_eq!(window.find("autumn-refresh").label(), Some("Refresh"));
        assert_eq!(window.find("autumn-draw").label(), Some("Draw a wish"));
        assert_eq!(window.find("autumn-claim").label(), Some("Claim reward"));
        window.click("autumn-wish-choice-12", cx);
        assert_eq!(window.find("autumn-wish-choice-12").focused(), Some(true));
        window.press("right", cx);
        window.render_frame(cx);
        assert_eq!(window.find("autumn-wish-choice-12").selected(), Some(false));
        assert_eq!(window.find("autumn-wish-choice-34").selected(), Some(true));
        assert_eq!(window.find("autumn-wish-choice-34").focused(), Some(true));
        window.press("right", cx);
        window.render_frame(cx);
        assert_eq!(window.find("autumn-wish-choice-12").selected(), Some(true));
        assert_eq!(window.find("autumn-wish-choice-12").focused(), Some(true));
        window.press("left", cx);
        window.render_frame(cx);
        assert_eq!(window.find("autumn-wish-choice-34").selected(), Some(true));
        assert_eq!(window.find("autumn-wish-choice-34").focused(), Some(true));
        window.click("autumn-refresh", cx);
        window.render_frame(cx);
        assert!(window.find("autumn-refresh").visible());
    })
    .expect("render and navigate the autumn wish radio group");
    cx.update(|cx| {
        let this = desktop.read(cx);
        assert_eq!(this.activities.autumn_choice_id, "34");
        assert_eq!(this.page, Page::Activities);
        assert_eq!(this.page_section, "autumn");
        assert!(!this.loading);
    });

    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page_raw["autumn"]["wish"]["canDraw"] = serde_json::Value::Bool(false)
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.press("left", cx);
        window.render_frame(cx);
        assert_eq!(window.find("autumn-wish-choice-34").selected(), Some(true));
    })
    .expect("disable choice navigation when the wish cannot be drawn");
    cx.update(|cx| assert_eq!(desktop.read(cx).activities.autumn_choice_id, "34"));
}

#[gpui_kit::test]
fn autumn_happy_renders_dynamic_actions_milestones_and_log_tabs(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "autumn".into();
            this.selected_account = Some("7".into());
            this.activities.autumn_mode = "happy".into();
            this.page_raw = serde_json::json!({
                "autumn": {
                    "happy": {
                        "title": "快乐不独享",
                        "active": true,
                        "score": 20,
                        "dailyReward": 5,
                        "firstShareReward": 10,
                        "claimedCount": 1,
                        "claimLimit": 3,
                        "poolClaimedCount": 2,
                        "poolClaimLimit": 8,
                        "canClaimDaily": false,
                        "canShare": true,
                        "firstShareAwarded": false,
                        "canClaimMilestones": true,
                        "milestones": [{
                            "id": "20",
                            "threshold": 20,
                            "state": 2,
                            "rewards": [{"name": "快乐礼包", "count": 1}]
                        }],
                        "logs": {"total": 1, "logs": [{
                            "seq": 1,
                            "score": 3,
                            "createdAt": 1_790_000_000,
                            "actor": {"name": "好友"}
                        }]}
                    }
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        let share = window.find("autumn-happy-share");
        assert_eq!(share.label(), Some("Share for first reward +10"));
        assert!(window.find("autumn-happy-daily").visible());
        assert!(window.find("autumn-happy-milestones").visible());
        assert!(window.find("autumn-log-tab-0").visible());
        assert!(window.find("autumn-log-tab-1").visible());
        assert_eq!(window.find("autumn-happy-daily").label(), Some("Claim daily happiness"));
        assert_eq!(
            window.find("autumn-happy-milestones").label(),
            Some("Claim all milestone rewards")
        );
        assert_eq!(window.find("autumn-log-tab-0").label(), Some("Available to claim"));
        assert_eq!(window.find("autumn-log-tab-1").label(), Some("Claimed by me"));
        assert_eq!(window.find(("autumn-milestone-status", 0_usize)).label(), Some("Claimable"));
    })
    .expect("render the happy activity details and operations");
}
