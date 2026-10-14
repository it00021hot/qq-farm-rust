use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn daily_gifts_render_as_status_cards_and_growth_tasks(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Personal;
            this.page_section = "dailyGifts".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "dailyGifts": {
                    "gifts": [
                        {"key":"task_claim", "label":"每日任务", "doneToday":true,
                            "completedCount":2, "totalCount":5, "lastAt":1790800000000_i64},
                        {"key":"vip_daily_gift", "label":"会员礼包", "hasGift":false,
                            "doneToday":false, "enabled":false}
                    ],
                        "growth": {
                            "doneToday":false, "completedCount":1, "totalCount":3,
                            "tasks":[
                                {"id":1, "desc":"收获 5 个作物并访问好友农场", "isCompleted":false,
                                    "progress":2, "totalProgress":5},
                                {"id":2, "desc":"收集每日奖励", "isCompleted":true,
                                    "progress":5, "totalProgress":5},
                                {"id":3, "desc":"兼容数字字符串进度", "isCompleted":false,
                                    "progress":"0", "totalProgress":"0"}
                            ]
                        }
                }
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("daily-gift-task_claim").visible());
        assert!(window.find("daily-gift-vip_daily_gift").visible());
        assert_eq!(window.find("daily-gift-status-task_claim").label(), Some("今日已完成"));
        assert_eq!(window.find("growth-summary-status").label(), Some("进度：1/3"));
        assert!(window.find("growth-task-0").visible());
        assert_eq!(window.find("growth-task-status-0").label(), Some("进行中"));
        assert_eq!(window.find("growth-task-progress-0").label(), Some("进度：2/5"));
        assert_eq!(window.find("growth-task-status-1").label(), Some("已完成"));
        assert_eq!(window.find("growth-task-progress-2").label(), Some("进度：0/0"));
        assert!(window.find("growth-task-description-0").visible());
    })
    .expect("render read-only daily gift and growth task cards");
}
