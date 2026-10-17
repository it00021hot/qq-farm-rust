use super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

fn shop_snapshot(opened: bool) -> serde_json::Value {
    let now = 1_792_252_900_i64;
    serde_json::json!({
        "id": 2_026_110_101_i64,
        "title": "奶茶补给站",
        "serverTime": now,
        "startTime": now - 86_400,
        "endTime": now + 30 * 86_400,
        "active": true,
        "settling": false,
        "recommendedNames": ["奶茶小铺", "奶茶冲鸭"],
        "summary": {
            "shopOpenAt": if opened { now - 3_600 } else { 0 },
            "shopName": "奶茶补给站",
            "renameCdUntilAt": 0,
            "level": 2,
            "totalRevenue": 320,
            "hireDisabled": false,
            "cashierFull": false,
            "cashierWarning": false,
            "canUpgrade": true
        },
        "shop": {
            "level": 2,
            "totalRevenue": 320,
            "nextLevelRevenue": 800,
            "cashierCapacitySec": 14_400,
            "cashierElapsedSec": 3_600,
            "slots": [],
            "salesItemId": 1041,
            "isSettling": false,
            "pendingRewards": [{"id": 1041, "count": 18, "name": "奶茶券", "image": ""}],
            "boostPreviewRewards": [{"id": 1041, "count": 24, "name": "奶茶券", "image": ""}]
        },
        "recipes": []
    })
}

#[gpui_kit::test]
fn milk_tea_shop_renders_summary_and_opens_rename_dialog(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::MilkTea;
            this.page_section = "shop".into();
            this.selected_account = Some("7".into());
            this.page_raw = shop_snapshot(true);
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("milk-tea-refresh").visible());
        assert_eq!(window.find("milk-tea-claim").label(), Some("领取收益"));
        assert_eq!(window.find("milk-tea-upgrade").label(), Some("升级店铺"));
        window.click("milk-tea-rename", cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        window.press("escape", cx);
    })
    .expect("milk tea shop page renders summary and rename dialog");
}

#[gpui_kit::test]
fn milk_tea_unopened_shop_shows_create_flow(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::MilkTea;
            this.page_section = "shop".into();
            this.selected_account = Some("7".into());
            this.page_raw = shop_snapshot(false);
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("milk-tea-create").label(), Some("开店营业"));
        assert!(window.find(("milk-tea-welcome", 0_usize)).visible());
        assert!(window.try_find("milk-tea-claim").is_none());
    })
    .expect("unopened shop shows the create flow instead of shop panels");
}
