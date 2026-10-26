use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn settings_save_feedback_shows_the_shared_toast(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            // 操作结果只走 toast：保存反馈不得再渲染内联横幅
            this.show_settings_notice("策略设置已保存", "Strategy settings saved", false, cx);
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("app-toast").visible());
        assert!(window.try_find("settings-save-notice").is_none());
    })
    .expect("render strategy save feedback as the shared toast");

    cx.update(|cx| {
        let toast = desktop.read(cx).toast.as_ref().expect("toast state");
        assert_eq!(toast.message, "策略设置已保存");
        assert!(toast.success);
    });
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
        assert!(window.find("app-toast").visible());
    })
    .expect("show reset confirmation feedback as the shared toast");

    cx.update(|cx| {
        let toast = desktop.read(cx).toast.as_ref().expect("toast state");
        assert_eq!(toast.message, "运行环境已重置为默认值");
        assert!(toast.success);
    });
}
