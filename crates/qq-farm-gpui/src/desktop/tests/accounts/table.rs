use super::{open_test_desktop, Page};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{point, px, size, AppContext as GpuiContextExt, ScrollDelta, TestAppContext};
use serde_json::json;

#[gpui_kit::test]
fn account_table_scrolls_the_body_and_keeps_the_header_fixed(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let records = (1..=25)
        .map(|id| {
            json!({
                "id": id.to_string(),
                "name": format!("账号 {id}"),
                "platform": "qq",
                "running": false,
                "wxAuthorized": false
            })
        })
        .collect::<Vec<_>>();
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.accounts.page_size = 20;
            this.page_raw = json!({"accounts": records});
            cx.notify();
        });
    });
    cx.simulate_window_resize(window, size(px(1440.), px(900.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let select_all_before = window.find("account-select-all").bounds();
        let footer_top = window.find("account-page-size-selector").bounds().top();
        let last_before = window.find("account-select-20").bounds();
        let pinned_first = window.find("account-edit-1").bounds();
        let center_first = window.find("account-select-1").bounds();
        assert!(
            (pinned_first.center().y - center_first.center().y).abs() < px(1.),
            "the pinned action lane is baseline-aligned with the center rows"
        );
        assert!(
            last_before.bottom() > footer_top,
            "rows past the visible body start out below the frame"
        );
        window.scroll("account-select-1", ScrollDelta::Pixels(point(px(0.), px(-1000.))), cx);
        window.render_frame(cx);
        let last_after = window.find("account-select-20").bounds();
        assert!(
            last_after.top() < last_before.top() - px(40.),
            "the body scrolls rows that used to be clipped away"
        );
        assert!(last_after.bottom() < footer_top, "scrolled rows land inside the visible body");
        let pinned_last = window.find("account-edit-20").bounds();
        assert!(
            (pinned_last.center().y - last_after.center().y).abs() < px(1.),
            "the pinned lane scrolls in lockstep with the center rows"
        );
        let select_all_after = window.find("account-select-all").bounds();
        assert!(
            (select_all_after.origin.y - select_all_before.origin.y).abs() < px(0.5),
            "the account table header stays fixed while the body scrolls"
        );
    })
    .expect("scroll a tall account page inside its table body");
}

#[gpui_kit::test]
fn account_table_empty_copy_follows_vue_no_data_and_filters(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Accounts;
            this.page_raw = json!({"accounts": []});
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("accounts-empty").label(), Some("无数据"));
        desktop.update(cx, |this, _| this.accounts.keyword_applied = "missing".into());
        window.render_frame(cx);
        assert_eq!(window.find("accounts-empty").label(), Some("没有符合条件的账号"));
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("accounts-empty").label(), Some("No accounts match these filters"));
    })
    .expect("account table empty copy mirrors Vue noData unless filters are active");
}

#[gpui_kit::test]
fn account_column_settings_change_visibility_order_and_fixed_lane(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [{
                    "id": "1", "name": "账号一", "platform": "qq",
                    "running": false, "wxAuthorized": false
                }]
            });
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("account-column-settings-trigger", cx);
        window.render_frame(cx);
        assert!(window.find("account-column-visible-platform").visible());
        window.click("account-column-visible-platform", cx);
        window.render_frame(cx);
        assert!(!desktop.read(cx).accounts.columns[2].visible);
        window.click("account-column-up-platform", cx);
        window.render_frame(cx);
        window.click("account-column-pin-platform", cx);
        window.render_frame(cx);
        window.click("account-column-visible-platform", cx);
        window.render_frame(cx);
        window.click("account-columns-select-all", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).accounts.columns.iter().all(|column| !column.visible));
        window.click("account-columns-select-all", cx);
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(window.try_find("account-column-visible-platform").is_none());
    })
    .expect("edit account table column settings");
    cx.update(|cx| {
        let desktop = desktop.read(cx);
        assert_eq!(desktop.accounts.columns[1].id.title("zh-CN"), "平台");
        assert!(desktop.accounts.columns[1].visible);
        assert_eq!(
            desktop.accounts.columns[1].pin,
            super::super::super::accounts::AccountColumnPin::Left
        );
        assert!(desktop.accounts.columns.iter().all(|column| column.visible));
    });
}

#[gpui_kit::test]
fn account_pagination_uses_vue_page_sizes_and_resets_to_first_page(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let records = (1..=101)
        .map(|id| {
            json!({
                "id": id.to_string(),
                "name": format!("账号 {id}"),
                "platform": "qq",
                "running": false,
                "wxAuthorized": false
            })
        })
        .collect::<Vec<_>>();
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({"accounts": records});
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).accounts.page_size, 10);
        assert_eq!(window.find(("account-page-number", 4_usize)).label(), Some("第 5 页"));
        window.click(("account-page-number", 4_usize), cx);
        assert_eq!(desktop.read(cx).accounts.page, 4);
        window.click(("account-page-jump", 7_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).accounts.page, 7);
        window.click(("account-page-number", 8_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).accounts.page, 8);
        window.click("account-page-size-selector", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
        window.render_frame(cx);
        assert_eq!(window.find("account-page-size-selector").label(), Some("每页 15 条"));
        assert_eq!(desktop.read(cx).accounts.page_size, 15);
        assert_eq!(desktop.read(cx).accounts.page, 0);
        window.click(("account-page-number", 2_usize), cx);
        assert_eq!(desktop.read(cx).accounts.page, 2);
    })
    .expect("change account page size and navigate the resulting pages");
}
