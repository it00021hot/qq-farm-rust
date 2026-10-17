use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::component::IndexPath;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use std::path::PathBuf;
use std::time::Duration;

#[gpui_kit::test]
async fn offline_reminder_can_be_saved_and_reloaded_without_an_account(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let app = cx.update(|cx| desktop.read(cx).app.clone());
    let loaded =
        crate::pages::load_page_for_mall_slot(&app, Page::Settings, "", 0, "offlineReminder")
            .await
            .expect("load global reminder settings without an account");
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "offlineReminder".into();
            this.selected_account = None;
            this.page_raw = loaded.raw.clone();
            this.settings.draft = loaded.raw.clone();
            this.settings.draft["systemConfig"]["deviceInfo"]["deviceId"] =
                serde_json::json!("unsaved-system-config-draft");
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.settings.offline_provider.update(cx, |provider, cx| {
                provider.set_selected_index(Some(IndexPath::new(2)), window, cx);
                cx.emit(gpui_kit::component::select::SelectEvent::Confirm(Some("ding_talk")));
            });
            this.settings.offline_endpoint.update(cx, |input, cx| {
                input.set_value("https://example.test/webhook", window, cx);
            });
            this.settings.offline_token.update(cx, |input, cx| {
                input.set_value("gpui-reminder-token", window, cx);
            });
            this.settings.offline_secret.update(cx, |input, cx| {
                input.set_value("gpui-reminder-secret", window, cx);
            });
        });
    })
    .expect("select DingTalk and edit reminder fields from the account-free form");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(desktop.read(cx).settings.draft["offlineReminder"]["provider"], "ding_talk");
    });
    let refresh_revision = cx.update(|cx| desktop.read(cx).request_revision);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("settings-dingtalk-endpoint").visible());
        window.click("save-offline-reminder", cx);
        window.render_frame(cx);
        assert!(window.find("settings-save-notice").visible());
    })
    .expect("save DingTalk reminder settings from the account-free form");
    cx.update(|cx| {
        let this = desktop.read(cx);
        assert_eq!(
            this.request_revision, refresh_revision,
            "saving a reminder must not reload the page"
        );
        assert_eq!(
            this.settings.draft["systemConfig"]["deviceInfo"]["deviceId"],
            "unsaved-system-config-draft",
            "saving a reminder must preserve drafts in other settings groups"
        );
    });

    let store_path = qq_farm_core::models::store::global_config::store_file();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let persisted = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .is_some_and(|store| {
                store.pointer("/userOfflineReminders/local/provider")
                    == Some(&serde_json::json!("ding_talk"))
                    && store.pointer("/userOfflineReminders/local/endpoint")
                        == Some(&serde_json::json!("https://example.test/webhook"))
            });
        if persisted {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "offline reminder did not reach store.json");
        std::thread::sleep(Duration::from_millis(10));
    }

    qq_farm_core::models::store::global_config::load_global_config()
        .expect("reload the saved offline reminder");
    let restored = serde_json::to_value(qq_farm_app::settings::get_offline_reminder(Some("local")))
        .expect("serialize reloaded reminder settings");
    assert_eq!(restored["provider"], "ding_talk");
    assert_eq!(restored["endpoint"], "https://example.test/webhook");
    assert_eq!(restored["token"], "gpui-reminder-token");
    assert_eq!(restored["secret"], "gpui-reminder-secret");
}

#[gpui_kit::test]
fn offline_test_pending_state_only_disables_its_save_controls(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "offlineReminder".into();
            this.selected_account = None;
            this.settings.draft = serde_json::json!({
                "offlineReminder": {"provider": "ding_talk"}
            });
            this.page_raw = this.settings.draft.clone();
            this.settings.offline_test_loading = true;
        });
    });
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.settings.offline_endpoint.update(cx, |input, cx| {
                input.set_value("https://example.test/webhook", window, cx);
            });
        });
        window.render_frame(cx);
        window.click("test-offline-reminder", cx);
        window.click("save-offline-reminder", cx);
        assert_ne!(window.find("offline-reminder-provider").disabled(), Some(true));
        assert_ne!(window.find("settings-dingtalk-endpoint").disabled(), Some(true));
        let state = desktop.read(cx);
        assert!(state.settings.offline_test_loading, "the test action stays pending");
        assert!(!state.loading, "testing a reminder must not lock the settings page");
        assert!(state.settings.save_notice.is_none(), "saving stays inactive while the test runs");
    })
    .expect("render reminder-specific pending controls without locking settings");
}

#[gpui_kit::test]
fn offline_reminder_test_completion_clears_its_pending_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.settings.offline_test_loading = true;
            this.complete_offline_reminder_test(
                Ok(serde_json::json!({"ok": false, "msg": "test provider response"})),
                cx,
            );
            assert!(!this.settings.offline_test_loading);
            assert!(!this.loading, "the test request must not use page-wide loading state");
            assert_eq!(this.error.as_deref(), Some("测试失败: test provider response"));
        });
    });
}

#[gpui_kit::test]
fn settings_automation_and_reminder_controls_follow_english_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "automation".into();
            this.selected_account = Some("7".into());
            this.settings.draft = serde_json::json!({
                "automation": {"farm": true},
                "offlineReminder": {"provider": "ding_talk"}
            });
            this.page_raw = this.settings.draft.clone();
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("automation-switch-farm").label(), Some("Auto Plant & Harvest"));
        assert_eq!(
            window.find("save-automation-settings").label(),
            Some("Save Automation")
        );

        desktop.update(cx, |this, cx| {
            this.page_section = "offlineReminder".into();
            this.settings.offline_provider.update(cx, |provider, cx| {
                provider.set_selected_index(Some(IndexPath::new(2)), window, cx);
            });
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("offline-reminder-provider").label(),
            Some("Bot Type")
        );
        assert_eq!(window.find("test-offline-reminder").label(), Some("Test Notification"));
        assert_eq!(window.find("save-offline-reminder").label(), Some("Save Offline Alert"));
        assert_eq!(window.find("settings-dingtalk-endpoint").label(), Some("Webhook URL"));
        assert_eq!(window.find("settings-dingtalk-token").label(), Some("Access Token"));
        assert_eq!(window.find("settings-dingtalk-secret").label(), Some("Signing secret"));
        assert_eq!(
            window.find("offline-reminder-hint").label(),
            Some("After binding, offline, online, and YYB QR alerts are sent automatically. No title or message setup is needed.")
        );
        assert_eq!(
            desktop
                .read(cx)
                .settings
                .offline_endpoint
                .read(cx)
                .presentation()
                .placeholder()
                .as_ref(),
            "https://oapi.dingtalk.com/robot/send?access_token=... (either this or the access token)"
        );
        assert_eq!(
            desktop.read(cx).settings.offline_token.read(cx).presentation().placeholder().as_ref(),
            "Either the webhook URL or this"
        );
        assert_eq!(
            desktop.read(cx).settings.offline_secret.read(cx).presentation().placeholder().as_ref(),
            "Optional; SEC-prefixed secret when the bot uses signed mode"
        );

        desktop.update(cx, |this, cx| {
            this.settings.draft["offlineReminder"]["provider"] = serde_json::json!("qq_bot");
            this.settings.offline_provider.update(cx, |provider, cx| {
                provider.set_selected_index(Some(IndexPath::new(1)), window, cx);
            });
            this.settings.qq_bot_session_id = Some("pending-session".into());
            this.settings.qq_bot_qr_path = None;
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("qq-bot-hint").label(),
            Some("Enter the AppID and AppSecret, click bind, then open the bot in mobile QQ and send any message. After binding, offline, online, and YYB QR alerts are pushed automatically.")
        );
        assert_eq!(
            window.find("qq-bot-bind-manual-hint").label(),
            Some("Waiting: open this bot in mobile QQ and send any message (no in-app QR required).")
        );

        desktop.update(cx, |this, cx| {
            this.settings.qq_bot_qr_path =
                Some(PathBuf::from("desktop-ui/public/land-assets/1.png"));
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("qq-bot-bind-scan-hint").label(),
            Some("If an invite URL is set, scan to open the bot card, then send any message.")
        );
    })
    .expect("render and switch English automation and offline reminder controls");
}
