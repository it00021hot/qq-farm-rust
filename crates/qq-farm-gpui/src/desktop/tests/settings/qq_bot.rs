use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::component::IndexPath;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use std::path::PathBuf;

#[gpui_kit::test]
fn qq_bot_binding_qr_is_rendered_in_the_binding_section_at_vue_size(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "offlineReminder".into();
            this.selected_account = Some("7".into());
            this.settings.draft = serde_json::json!({
                "offlineReminder": {
                    "provider": "qq_bot",
                    "qqBot": {"appId": "app-id", "clientSecret": "secret"},
                    "qqBotBinding": {"userOpenid": ""}
                }
            });
            this.settings.qq_bot_status = Some("请使用 QQ 扫描二维码完成绑定。".into());
            this.settings.qq_bot_session_id = Some("session".into());
            this.settings.qq_bot_qr_path =
                Some(PathBuf::from("desktop-ui/public/activity-assets/autumn/happy.png"));
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.settings.offline_provider.update(cx, |provider, cx| {
                provider.set_selected_index(Some(IndexPath::new(1)), window, cx);
            });
        });
        window.render_frame(cx);
        let qr = window.find("qq-bot-bind-qr");
        assert!(qr.visible());
        assert!((qr.bounds().size.height.as_f32() - 180.).abs() < 1.);
    })
    .expect("place the full-size QQ binding QR inside the reminder form");
}

#[gpui_kit::test]
fn qq_bot_bind_start_is_disabled_while_the_start_request_is_pending(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "offlineReminder".into();
            this.selected_account = Some("7".into());
            this.settings.draft = serde_json::json!({"offlineReminder": {"provider": "qq_bot"}});
            this.settings.qq_bot_bind_loading = true;
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.settings.qq_bot_app_id.update(cx, |state, cx| {
                state.set_value("app-id", window, cx);
            });
            this.settings.qq_bot_secret.update(cx, |state, cx| {
                state.set_value("secret", window, cx);
            });
            this.settings.offline_provider.update(cx, |provider, cx| {
                provider.set_selected_index(Some(IndexPath::new(1)), window, cx);
            });
        });
        window.render_frame(cx);
        assert!(window.find("qqbot-bind-start").visible());
        window.click("qqbot-bind-start", cx);
        window.render_frame(cx);
        let state = desktop.read(cx);
        assert!(state.settings.qq_bot_bind_loading);
        assert!(state.settings.qq_bot_session_id.is_none());
    })
    .expect("prevent a second QQ binding session while the first request is pending");
}

#[gpui_kit::test]
fn qq_bot_binding_refresh_keeps_unsubmitted_settings_drafts(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.settings.draft = serde_json::json!({
                "plantingStrategy": "preferred",
                "automation": {"farm": false, "steal": true},
                "systemConfig": {"serverUrl": "wss://unsaved.example"},
                "offlineReminder": {"provider": "qq_bot", "title": "reminder draft"}
            });
            this.page_raw = this.settings.draft.clone();
            this.request_revision = 41;

            this.apply_qq_bot_binding_snapshot(
                serde_json::json!({
                    "provider": "qq_bot",
                    "title": "saved reminder",
                    "qqBot": {"appId": "app-id", "clientSecret": "secret"},
                    "qqBotBinding": {"userOpenid": "openid", "nickname": "Farmer"}
                }),
                serde_json::json!({
                    "credentialsConfigured": true,
                    "bound": true,
                    "binding": {"userOpenid": "openid", "nickname": "Farmer"},
                    "botInviteUrl": "https://example.invalid/bot"
                }),
            );
        });
    });

    cx.update(|cx| {
        let state = desktop.read(cx);
        assert_eq!(state.settings.draft["plantingStrategy"], "preferred");
        assert_eq!(state.settings.draft["automation"]["farm"], false);
        assert_eq!(state.settings.draft["automation"]["steal"], true);
        assert_eq!(state.settings.draft["systemConfig"]["serverUrl"], "wss://unsaved.example");
        assert_eq!(state.settings.draft["offlineReminder"]["title"], "saved reminder");
        assert_eq!(state.page_raw["qqBotBinding"]["bound"], true);
        assert_eq!(state.request_revision, 41);
    });
}
