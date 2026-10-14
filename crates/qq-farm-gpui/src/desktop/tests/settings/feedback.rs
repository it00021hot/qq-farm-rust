use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn settings_save_notice_is_visible_on_the_strategy_page(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.settings.save_notice = Some("策略设置已保存".into());
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("settings-save-notice").visible());
    })
    .expect("render strategy save feedback");
}

#[gpui_kit::test]
fn resetting_system_configuration_shows_success_feedback(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "systemConfig".into();
            this.settings.draft["systemConfig"] = serde_json::json!({"serverUrl": "custom"});
            this.page_raw["systemConfig"] = serde_json::json!({"serverUrl": "custom"});
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("reset-system-config", cx);
        window.render_frame(cx);
        assert!(window.find("settings-save-notice").visible());
    })
    .expect("show reset confirmation feedback in the shared settings area");

    cx.update(|cx| {
        assert_eq!(
            desktop.read(cx).settings.save_notice.as_deref(),
            Some("运行环境已重置为默认值")
        );
    });
}
