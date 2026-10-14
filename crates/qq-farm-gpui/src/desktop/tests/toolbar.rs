//! Regression for the 2026-10-14 crash: the toolbar's refresh, sidebar, update
//! check and settings save buttons re-dispatched their actions through the
//! focus handle from inside their click listeners. That synchronously
//! re-entered the view entity's lease and panicked with "cannot update
//! FarmDesktop while it is already being updated" on a real click. Each button
//! must run its command directly instead.

use super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use serde_json::json;

#[gpui_kit::test]
async fn toolbar_buttons_run_their_commands_without_reentering_the_view(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);

    // The crash was reported while the accounts page was open.
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Accounts;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);

        let revision = desktop.read(cx).request_revision;
        window.click("refresh", cx);
        assert_eq!(
            desktop.read(cx).request_revision,
            revision + 1,
            "toolbar refresh must run the page refresh command"
        );

        let collapsed = desktop.read(cx).sidebar_collapsed;
        window.click("toggle-sidebar", cx);
        assert_eq!(desktop.read(cx).sidebar_collapsed, !collapsed);

        window.click("check-update", cx);
        assert_eq!(desktop.read(cx).update_message.as_deref(), Some("正在检查发布…"));
    })
    .expect("click the toolbar refresh, sidebar and update buttons");

    // The settings save button shares the same click path.
    let app = cx.update(|cx| desktop.read(cx).app.clone());
    let loaded = crate::pages::load_page_for_mall_slot(&app, Page::Settings, "", 0, "strategy")
        .await
        .expect("load the strategy settings section");
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some("1".into());
            this.page_raw = loaded.raw.clone();
            this.settings.draft = loaded.raw.clone();
            this.settings.draft["plantingStrategy"] = json!("level");
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-settings", cx);
        window.render_frame(cx);
        assert!(window.find("settings-save-notice").visible());
    })
    .expect("click the toolbar settings save button");
    cx.update(|cx| {
        assert_eq!(desktop.read(cx).settings.save_notice.as_deref(), Some("策略设置已保存"));
    });
}
