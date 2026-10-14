use super::super::super::Page;
use super::super::support::open_test_desktop;
use crate::pages::DisplayRow;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn visitor_rows_match_vue_fields_and_localize_the_relative_time(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "records".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "records": [{
                    "visitorGid": 99,
                    "nick": "Alice",
                    "level": 12,
                    "actionType": 2,
                    "actionDetail": "Helped water plants",
                    "avatarUrl": "https://example.invalid/avatar.png",
                    "serverTimeMs": chrono::Local::now().timestamp_millis()
                }]
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        assert_eq!(desktop.read(cx).page, Page::Friends);
        assert_eq!(desktop.read(cx).page_section, "records");
        assert_eq!(desktop.read(cx).rows.len(), 1);
        window.render_frame(cx);
        assert_eq!(
            window.find("friend-row-records/0").label(),
            Some("Alice, Interact, Lv12, GID 99, Helped water plants, Just now")
        );
    })
    .expect("visitor history matches Vue identity, action, and translated timestamp fields");
}

#[gpui_kit::test]
fn visitor_name_and_action_detail_use_vue_fallbacks(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "records".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "records": [{
                    "visitorGid": 777,
                    "actionLabel": "Helped",
                    "avatarUrl": "ftp://invalid/avatar.png",
                    "serverTimeMs": 0
                }]
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert!(window.find("friend-visitor-avatar-0").visible());
        assert_eq!(window.find("friend-visitor-avatar-0").label(), Some("Avatar ?"));
        assert_eq!(
            window.find("friend-row-records/0").label(),
            Some("GID:777, Helped, GID 777, Helped, --")
        );
    })
    .expect("visitor names, action labels, and missing-time values match Vue fallbacks");
}

#[gpui_kit::test]
fn visitors_page_rows_remain_independent_of_generic_friend_metadata(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "records".into();
            this.selected_account = Some("test-account".into());
            this.rows = vec![DisplayRow {
                id: "records/0".into(),
                section: "records".into(),
                title: "Visitor".into(),
                detail: "不应显示的通用字段".into(),
                state: "不应显示的状态".into(),
                record: serde_json::json!({
                    "visitorGid": 100,
                    "nick": "Visitor",
                    "actionDetail": "Visited the farm",
                    "serverTimeMs": 0
                }),
            }];
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(
            window.find("friend-row-records/0").label(),
            Some("Visitor, Interact, GID 100, Visited the farm, --")
        );
    })
    .expect("visitor layout renders through its dedicated business module");
}
