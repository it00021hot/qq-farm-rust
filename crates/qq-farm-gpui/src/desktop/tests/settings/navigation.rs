use super::super::super::{default_page_section, page_sections, Page};
use super::super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[test]
fn settings_navigation_matches_vue_tabs_and_default() {
    assert_eq!(default_page_section(Page::Settings), "strategy");
    assert_eq!(
        page_sections(Page::Settings).iter().map(|(key, _)| *key).collect::<Vec<_>>(),
        ["strategy", "automation", "offlineReminder", "systemConfig", "ui"]
    );
    assert_eq!(Page::Settings.section_title_for("strategy", "en-US"), Some("Strategy settings"));
}

#[gpui_kit::test]
async fn global_settings_load_without_an_account_while_account_tabs_prompt_for_one(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let app = cx.update(|cx| desktop.read(cx).app.clone());
    let loaded =
        crate::pages::load_page_for_mall_slot(&app, Page::Settings, "", 0, "offlineReminder")
            .await
            .expect("load global settings without selecting an account");

    assert!(loaded.raw["systemConfig"].is_object());
    assert!(loaded.raw["offlineReminder"].is_object());
    assert!(loaded.raw["qqBotBinding"].is_object());
    assert!(loaded.rows.is_empty(), "structured settings should not be repeated as generic rows");

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.selected_account = None;
            this.page_raw = loaded.raw.clone();
            this.settings.draft = loaded.raw.clone();
            this.rows = loaded.rows.clone();
            this.page_section = "strategy".into();
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.sync_system_config_inputs(window, cx);
            this.sync_setting_selectors(window, cx);
        });
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert_eq!(state.page, Page::Settings);
        assert_eq!(state.page_section, "strategy");
        assert!(state.selected_account.is_none());
        assert!(window.try_find("empty-refresh").is_none());
        assert!(window.find("settings-select-account-strategy").visible());
        assert!(window.try_find("planting-strategy-select").is_none());
        assert!(window.try_find("planting-strategy-select").is_none());

        desktop.update(cx, |this, cx| {
            this.page_section = "automation".into();
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("settings-select-account-automation").visible());
        assert!(window.try_find("automation-switch-farm").is_none());

        desktop.update(cx, |this, cx| {
            this.page_section = "offlineReminder".into();
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("offline-reminder-provider").visible());
        assert!(window.find("save-offline-reminder").visible());

        desktop.update(cx, |this, cx| {
            this.page_section = "systemConfig".into();
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("system-server-url").visible());
        assert!(window.find("save-system-config").visible());
    })
    .expect("render account-required and global settings sections with no selected account");
}

#[gpui_kit::test]
fn english_locale_localizes_navigation_and_shared_toolbar(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.preferences.language = "en-US".into();
            this.page = Page::Personal;
            this.page_section = "lands".into();
            this.selected_account = Some("7".into());
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("personal").label(), Some("My Farm"));
        assert_eq!(window.find(("page-section", 0_usize)).label(), Some("Farm"));
        assert_eq!(window.find(("page-section", 1_usize)).label(), Some("My Bag"));
        assert_eq!(window.find("refresh").label(), Some("Refresh"));
        assert!(window.try_find("farm-cycle").is_none());
        assert!(window.try_find("add-account").is_none());
    })
    .expect("render English navigation and toolbar labels");
}
