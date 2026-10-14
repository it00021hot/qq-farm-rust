use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use serde_json::json;

#[gpui_kit::test]
fn account_keyword_search_uses_vue_name_qq_and_remark_fields(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [{
                    "id": "search-account",
                    "name": "Saved Account",
                    "qq": "123456",
                    "uin": "654321",
                    "nick": "Runtime Nickname",
                    "remark": "Farm Notes",
                    "platform": "qq",
                    "running": false,
                    "wxAuthorized": true
                }]
            });
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("account-select-search-account").visible());
        assert!(window.try_find("account-search-panel").is_none());
        window.click("account-search-toggle", cx);
        window.render_frame(cx);

        window.click("account-search", cx);
        window.input("Runtime Nickname", cx);
        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.try_find("account-select-search-account").is_none());
        assert!(window.try_find("account-empty-add").is_none());
        assert!(window.find("account-add-toolbar").visible());

        window.click("account-search-reset", cx);
        window.render_frame(cx);
        // Reset restores the search form only. The last submitted result set stays
        // visible until Search applies the cleared fields.
        assert!(window.try_find("account-select-search-account").is_none());
        window.click("account-search", cx);
        window.input("Farm Notes", cx);
        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.find("account-select-search-account").visible());

        window.click("account-search-reset", cx);
        window.click("account-search", cx);
        window.input("654321", cx);
        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.try_find("account-select-search-account").is_none());
        assert!(window.try_find("account-empty-add").is_none());
        assert!(window.find("account-add-toolbar").visible());

        window.click("account-search-reset", cx);
        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.find("account-select-search-account").visible());
    })
    .expect("filter accounts by only the fields Vue searches");
}

#[gpui_kit::test]
async fn account_filter_drafts_apply_only_after_search_and_reset_matches_vue(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            this.page_raw = json!({
                "accounts": [{
                    "id": "qq-account", "name": "QQ account", "platform": "qq",
                    "running": false, "wxAuthorized": false
                }]
            });
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("account-search-toggle", cx);
    })
    .expect("open the account search card");
    super::choose_filter_option(cx, window, "account-platform-select", 2).await;

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("account-select-qq-account").visible());
        assert_eq!(desktop.read(cx).accounts.platform_filter, None);
        assert_eq!(desktop.read(cx).accounts.platform_draft.as_deref(), Some("wx"));

        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.try_find("account-select-qq-account").is_none());
        assert_eq!(desktop.read(cx).accounts.platform_filter.as_deref(), Some("wx"));

        window.click("account-search-reset", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).accounts.platform_filter.as_deref(), Some("wx"));
        assert_eq!(desktop.read(cx).accounts.platform_draft, None);
        assert!(window.try_find("account-select-qq-account").is_none());

        window.click("account-search-submit", cx);
        window.render_frame(cx);
        assert!(window.find("account-select-qq-account").visible());
        assert_eq!(desktop.read(cx).accounts.platform_filter, None);
    })
    .expect("account search drafts apply and reset like the Vue search card");
}
