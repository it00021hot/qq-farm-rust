use super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{point, px, AppContext as _, TestAppContext};

#[gpui_kit::test]
fn sidebar_drag_clamps_persists_and_keeps_navigation_visible(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, desktop, _restore) = open_test_desktop(cx);
    for target_width in [264., 400., 120.] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let sidebar = window.find("farm-sidebar").bounds();
            let y = sidebar.size.height / 2.;
            window.drag(point(sidebar.right() + px(0.5), y), point(px(target_width), y), cx);
        })
        .expect("drag sidebar divider");
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let expected = target_width.clamp(200., 288.);
            assert_eq!(
                desktop.read(cx).preferences.sidebar_width,
                expected as u32,
                "sidebar={:?}, panel_sizes={:?}",
                window.find("farm-sidebar").bounds(),
                desktop.read(cx).sidebar_layout.read(cx).sizes()
            );
            assert!(
                (window.find("farm-sidebar").bounds().size.width.as_f32() - expected).abs() < 1.
            );
            assert!(window.find("accounts").visible());
            assert!(window.find("sidebar-account-switcher").visible());
            let directory = desktop.read(cx)._lease.path().parent().expect("lease directory");
            assert_eq!(
                crate::runtime::load_ui_preferences_from(directory).sidebar_width,
                expected as u32
            );
        })
        .expect("check sidebar size and preferences");
    }
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.sidebar_collapsed = true;
            cx.notify();
        })
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("sidebar-account-switcher").is_none());
        assert!(window.find("accounts").visible());
        assert!(window.find("farm-sidebar").bounds().size.width < px(100.));
    })
    .expect("collapsed sidebar keeps navigation");
}

#[gpui_kit::test]
fn sidebar_account_switcher_switches_between_saved_accounts(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, desktop, _restore) = open_test_desktop(cx);
    let previous_revision = cx.update(|cx| desktop.read(cx).request_revision);
    cx.update_window(handle, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.accounts.records =
                vec![("a".into(), "Alpha".into(), false), ("b".into(), "Beta".into(), true)];
            this.selected_account = Some("a".into());
            cx.notify();
        });
        window.render_frame(cx);
        // `label()` reports the announced accessibility name; the visible trigger
        // text is the same account name carried inside it.
        assert_eq!(window.find("sidebar-account-switcher").label(), Some("当前账号：Alpha"));
        window.click("sidebar-account-switcher", cx);
        window.within("popup-menu").click(1_usize, cx);
        window.render_frame(cx);
    })
    .expect("switch to the second account from the sidebar dropdown");
    cx.update(|cx| {
        let state = desktop.read(cx);
        assert_eq!(state.selected_account.as_deref(), Some("b"));
        assert!(state.request_revision > previous_revision);
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("sidebar-account-switcher").label(), Some("当前账号：Beta"));
    })
    .expect("the sidebar switcher follows the new selection");
}

#[gpui_kit::test]
fn page_search_placeholder_tracks_page_and_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, desktop, _restore) = open_test_desktop(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            desktop.read(cx).search.read(cx).presentation().placeholder().as_ref(),
            "搜索当前页面"
        );
        for (page_id, expected) in [
            ("mall", "搜索商品名称 / ID"),
            ("friends", "搜索昵称 / GID"),
            ("game-config", "请输入关键词搜索"),
        ] {
            window.click(page_id, cx);
            window.render_frame(cx);
            assert_eq!(desktop.read(cx).page.id(), page_id);
            assert_eq!(
                desktop.read(cx).search.read(cx).presentation().placeholder().as_ref(),
                expected,
                "placeholder after switching to {page_id}"
            );
        }
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        assert_eq!(
            desktop.read(cx).search.read(cx).presentation().placeholder().as_ref(),
            "Please enter keyword"
        );
        for (page_id, expected) in
            [("mall", "Search item name / ID"), ("friends", "Search nickname / GID")]
        {
            window.click(page_id, cx);
            window.render_frame(cx);
            assert_eq!(
                desktop.read(cx).search.read(cx).presentation().placeholder().as_ref(),
                expected,
                "localized placeholder after switching to {page_id}"
            );
        }
    })
    .expect("search placeholder follows the selected page and locale");
}

#[gpui_kit::test]
fn page_level_states_show_update_notice_and_loading_text(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, desktop, _restore) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = crate::pages::Page::Friends;
            this.selected_account = Some("7".into());
            this.update_message = Some("发现新版本 v9.9.9".into());
            this.release_url = Some("https://example.com/releases/tag/v9.9.9".into());
            this.loading = true;
        });
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("update-notice").label(), Some("发现新版本 v9.9.9"));
        assert_eq!(window.find("open-release").label(), Some("打开发布页下载"));
        assert_eq!(window.find("page-loading").label(), Some("正在加载…"));
        desktop.update(cx, |this, _| {
            this.release_url = None;
            this.loading = false;
        });
        window.render_frame(cx);
        assert!(window.find("update-notice").visible());
        assert!(window.try_find("open-release").is_none());
        assert!(window.try_find("page-loading").is_none());
    })
    .expect("page-level update notice and loading text follow their state");
}
