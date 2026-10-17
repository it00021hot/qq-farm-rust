use super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn activity_center_lists_current_activities_and_navigates(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_raw = serde_json::Value::Null;
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("activity-center-solar-calendar").visible());
        assert!(window.find("activity-center-milk-tea").visible());
        assert!(window.find("activity-center-share-reward").visible());
        window.click("activity-center-open-milk-tea", cx);
        window.render_frame(cx);
    })
    .expect("activity center lists the current activities");
    cx.update(|cx| assert_eq!(desktop.read(cx).page, Page::MilkTea));
}

#[gpui_kit::test]
fn sidebar_keeps_one_activity_entry_and_hides_sub_pages(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, _desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("activities").visible());
        assert!(window.try_find("solar-calendar").is_none());
        assert!(window.try_find("milk-tea").is_none());
        assert!(window.try_find("share-reward").is_none());
    })
    .expect("sidebar keeps a single activity center entry");
}
