use super::{open_test_desktop, Page};
use gpui_kit::component::WindowExt;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{px, size, AppContext as GpuiContextExt, TestAppContext};
use serde_json::json;
use std::time::Duration;

use crate::pages::PageData;

async fn exercise_account_navigation(cx: &mut TestAppContext) {
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("accounts", cx);
        window.click("account-search-toggle", cx);
        window.render_frame(cx);
        window.click("account-search", cx);
        window.input("farm", cx);
        assert_eq!(window.find("account-search").value(), Some("farm"));
        window.press("backspace", cx);
        assert_eq!(window.find("account-search").value(), Some("far"));
        window.click("account-add-toolbar", cx);
        assert!(window.find("account-name").visible());
        window.click("account-name", cx);
        window.input("new account", cx);
        window.press("backspace", cx);
        assert_eq!(window.find("account-name").value(), Some("new accoun"));
        window.press("secondary-s", cx);
    })
    .expect("edit account and dispatch save shortcut");
    cx.update_window(window, |_, window, cx| {
        assert_eq!(
            desktop.read(cx).accounts.form_error.as_deref(),
            Some("请输入登录 code 或粘贴完整登录链接")
        );
        for step in 0..14 {
            window.press("tab", cx);
            assert!(window.has_active_sheet(cx), "sheet closed after Tab {step}");
            assert_eq!(window.find("sheet").focused(), Some(true));
            assert_ne!(window.find("account-search").focused(), Some(true));
        }
        window.press("escape", cx);
    })
    .expect("interact with farm window");
    cx.update_window(window, |_, window, _cx| {
        assert!(window.try_find("account-name").is_none());
    })
    .expect("Escape closes the account sheet and restores focus");
    cx.update_window(window, |_, window, _cx| {
        assert_eq!(window.find("account-search").focused(), Some(true));
    })
    .expect("return focus to the account search field");
    for close_button in ["account-cancel", "close"] {
        cx.update_window(window, |_, window, cx| window.click("account-add-toolbar", cx))
            .expect("open account sheet");
        cx.wait_for(window, Duration::from_secs(1), |window, _| {
            window.try_find(close_button).is_some_and(|button| button.visible())
                && window.find("sheet-content").bounds().right() <= window.viewport_size().width
        })
        .await;
        cx.update_window(window, |_, window, cx| window.click(close_button, cx))
            .expect("click sheet close command");
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(
                !window.has_active_sheet(cx),
                "{close_button} did not close the sheet; form_open={}",
                desktop.read(cx).accounts.form_open,
            );
            assert!(window.try_find("account-name").is_none());
            assert_eq!(window.find("account-search").focused(), Some(true));
        })
        .expect("dismiss account sheet and restore focus");
    }
    cx.update(|cx| {
        let desktop = desktop.read(cx);
        assert_eq!(desktop.page, Page::Accounts);
        assert!(!desktop.accounts.form_open);
    });
}

#[gpui_kit::test]
async fn navigation_search_and_account_sheet_are_keyboard_accessible(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    exercise_account_navigation(cx).await;
}

#[gpui_kit::test]
fn search_shortcut_opens_and_focuses_the_account_filter(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({ "accounts": [] });
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!desktop.read(cx).accounts.search_open);
        window.press("secondary-f", cx);
    })
    .expect("dispatch the page search shortcut");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(desktop.read(cx).accounts.search_open);
        assert!(window.find("account-search-panel").visible());
        assert_eq!(window.find("account-search").focused(), Some(true));
    })
    .expect("show and focus the account page's own search field");
}

#[test]
fn parses_account_login_links_and_preserves_manual_codes() {
    assert_eq!(
        super::super::super::parse_account_login_input(
            "https://game.qq.com/prod/ws?code=abc%2Bdef&platform=wx&os=mac",
            "qq",
        ),
        ("wx".into(), "abc+def".into()),
    );
    assert_eq!(
        super::super::super::parse_account_login_input("plain-code", "qq"),
        ("qq".into(), "plain-code".into()),
    );
}

#[gpui_kit::test]
async fn account_filters_and_login_sheet_follow_the_selected_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [{
                    "id": "1", "name": "Farm account", "platform": "wx",
                    "running": false, "wxAuthorized": false
                }]
            });
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert!(window.try_find("account-search-panel").is_none());
        window.click("account-search-toggle", cx);
        window.render_frame(cx);
        assert!(desktop.read(cx).accounts.search_open);
        assert!(window.find("account-search-panel").visible());
        assert_eq!(window.find("account-search-submit").label(), Some("Search"));
        assert_eq!(window.find("account-platform-select").value(), Some("All"));
        assert_eq!(window.find("account-run-status-select").value(), Some("All"));
        assert_eq!(window.find("account-auth-status-select").value(), Some("All"));
    })
    .expect("localize account search controls");
    super::choose_filter_option(cx, window, "account-run-status-select", 3).await;

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("account-run-status-select").value(), Some("Error"));
        assert_eq!(desktop.read(cx).accounts.running_draft, Some(2));
        assert_eq!(
            desktop.read(cx).accounts.keyword.read(cx).presentation().placeholder().as_ref(),
            "Search account name, QQ number, or notes"
        );
        assert_eq!(
            desktop.read(cx).accounts.name.read(cx).presentation().placeholder().as_ref(),
            "Optional, defaults to account#{id}"
        );
        window.click("account-add-toolbar", cx);
        window.render_frame(cx);
        assert_eq!(window.find("account-login-code").label(), Some("Enter code"));
        assert_eq!(window.find("account-login-wx").label(), Some("WeChat authorization"));
        assert_eq!(window.find("account-cancel").label(), Some("Cancel"));
        assert_eq!(window.find("account-save").label(), Some("Confirm"));
    })
    .expect("switch the account page and login sheet to English");
}

#[gpui_kit::test]
fn switching_accounts_discards_a_late_page_response(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Dashboard;
            this.selected_account = Some("old-account".into());
            this.page_raw = json!({"account": "old-account-before-refresh"});
            this.loading = true;
            let old_request_revision = this.request_revision;

            // Use the normal account-switch lifecycle, which invalidates outstanding requests.
            this.set_account("new-account".into(), window, cx);
            assert_eq!(this.selected_account.as_deref(), Some("new-account"));
            assert!(this.request_revision > old_request_revision);
            assert!(this.page_raw.is_null());

            // Deliver the old account's response after the switch, through the production commit
            // path used by the async loader callback.
            this.apply_page_load_result(
                old_request_revision,
                Page::Dashboard,
                0,
                "old-account".into(),
                None,
                Ok(PageData {
                    summary: "stale account response".into(),
                    rows: Vec::new(),
                    raw: json!({"account": "old-account-late-response"}),
                }),
                cx,
            );

            assert_eq!(this.selected_account.as_deref(), Some("new-account"));
            assert!(this.page_raw.is_null(), "stale response replaced the new account view");
            assert!(!this.loading, "stale response changed the current request's loading state");
        });
    })
    .expect("switch account and deliver the old page response");
}

#[gpui_kit::test]
fn account_row_actions_stay_visible_at_the_minimum_window_width(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [{
                    "id": "1", "name": "A long account name", "platform": "qq",
                    "running": false, "wxAuthorized": false
                }]
            });
        });
    });
    cx.simulate_window_resize(window, size(px(1100.), px(720.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let table_columns = window.find("account-table-columns").bounds();
        let actions = window.find("account-pinned-right").bounds();
        assert!(actions.size.width > px(0.));
        assert!(actions.right() <= window.viewport_size().width);
        assert!(
            (table_columns.origin.y - actions.origin.y).abs() < px(1.),
            "pinned action column must start at the same vertical position as the table"
        );
    })
    .expect("render the account table at the minimum desktop window size");
}
