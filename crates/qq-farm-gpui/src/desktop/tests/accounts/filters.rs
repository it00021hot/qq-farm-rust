use super::{choose_filter_option, open_test_desktop, Page};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use serde_json::json;

#[gpui_kit::test]
async fn account_search_applies_filter_drafts_only_on_search_like_vue(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [
                    {
                        "id": "1", "name": "Farm One", "qq": "100", "remark": "north",
                        "platform": "qq", "running": false, "wxAuthorized": false
                    },
                    {
                        "id": "2", "name": "Farm Two", "qq": "200", "remark": "target",
                        "platform": "wx", "running": true, "wxAuthorized": true
                    },
                    {
                        "id": "3", "name": "Other", "qq": "300", "remark": "south",
                        "platform": "wx", "running": false, "wxAuthorized": false
                    }
                ]
            });
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("account-search-toggle", cx);
    })
    .expect("open the account search form");
    choose_filter_option(cx, window, "account-platform-select", 2).await;
    choose_filter_option(cx, window, "account-run-status-select", 2).await;
    choose_filter_option(cx, window, "account-auth-status-select", 1).await;

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert_eq!(state.accounts.platform_draft.as_deref(), Some("wx"));
        assert_eq!(state.accounts.running_draft, Some(1));
        assert_eq!(state.accounts.auth_draft, Some(true));
        assert_eq!(state.accounts.platform_filter, None);
        assert!(window.find("account-select-1").visible());
        assert!(window.find("account-select-2").visible());
        assert!(window.find("account-select-3").visible());

        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.find("account-select-2").visible());
        assert!(window.try_find("account-select-1").is_none());
        assert!(window.try_find("account-select-3").is_none());

        // Vue's Reset restores the search form model but keeps the last results
        // until Search is pressed again.
        window.click("account-search-reset", cx);
        window.render_frame(cx);
        assert!(window.find("account-select-2").visible());
        assert!(window.try_find("account-select-1").is_none());
        assert!(window.try_find("account-select-3").is_none());
        assert_eq!(desktop.read(cx).accounts.platform_filter.as_deref(), Some("wx"));
        assert_eq!(desktop.read(cx).accounts.platform_draft, None);
        assert_eq!(desktop.read(cx).accounts.running_draft, None);
        assert_eq!(desktop.read(cx).accounts.auth_draft, None);

        window.click("account-search-submit", cx);
        window.render_frame(cx);
        for id in ["1", "2", "3"] {
            assert!(window.find(format!("account-select-{id}")).visible());
        }
    })
    .expect("search and reset account filters with Vue's apply timing");
}

#[gpui_kit::test]
async fn account_filter_drafts_survive_collapse_and_reset_like_vue(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [
                    {"id":"1", "name":"QQ farm", "platform":"qq", "running":false},
                    {"id":"2", "name":"WeChat farm", "platform":"wx", "running":false}
                ]
            });
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("account-search-toggle", cx);
    })
    .expect("open account filter controls");
    choose_filter_option(cx, window, "account-platform-select", 2).await;

    cx.update_window(window, |_, window, cx| {
        window.click("account-search-toggle", cx);
        window.render_frame(cx);
        assert!(window.try_find("account-search-panel").is_none());
        window.click("account-search-toggle", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).accounts.platform_draft.as_deref(), Some("wx"));
        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.find("account-select-2").visible());
        assert!(window.try_find("account-select-1").is_none());

        window.click("account-search-reset", cx);
        window.click("account-search-toggle", cx);
        window.click("account-search-toggle", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).accounts.platform_draft, None);
        assert_eq!(desktop.read(cx).accounts.platform_filter.as_deref(), Some("wx"));
        assert!(window.find("account-select-2").visible());
        assert!(window.try_find("account-select-1").is_none());
        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.find("account-select-1").visible());
        assert!(window.find("account-select-2").visible());
    })
    .expect("collapse and reopen account filters without losing the Vue form model");
}
