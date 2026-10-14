use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use std::time::Duration;

#[gpui_kit::test]
fn system_configuration_exposes_applies_and_persists_runtime_fields(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let snapshot = serde_json::json!({
        "plantingStrategy": "preferred",
        "automation": {"farm": false, "steal": true},
        "offlineReminder": {"provider": "ding_talk", "title": "unsaved reminder"},
        "systemConfig": {
            "serverUrl": "wss://old.example/ws",
            "clientVersion": "old-version",
            "platform": "qq",
            "os": "Windows",
            "timeZone": "Asia/Shanghai",
            "deviceInfo": {
                "os": "Windows",
                "clientVersion": "old-version",
                "sysSoftware": "Old Windows",
                "deviceId": "OLD-PC",
                "memory": "8192",
                "network": "wifi",
                "userAgent": "Old Agent"
            }
        },
        "devicePresets": [{
            "id": "iphone_test",
            "name": "iPhone test",
            "description": "Test device profile",
            "deviceInfo": {
                "os": "iOS",
                "clientVersion": "ios-version",
                "sysSoftware": "iOS 17",
                "deviceId": "iPhone-test",
                "memory": "16384",
                "network": "wifi",
                "userAgent": "iPhone Agent"
            }
        }]
    });
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "systemConfig".into();
            this.page_raw = snapshot.clone();
            this.settings.draft = snapshot.clone();
            this.request_revision = 41;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        for id in [
            "system-server-url",
            "system-client-version",
            "system-software",
            "system-device-id",
            "system-memory",
            "system-network",
            "system-user-agent",
            "system-device-preset-iphone_test",
            "system-timezone-select",
            "system-platform-wx",
            "system-os-Android",
            "save-system-config",
            "reset-system-config",
        ] {
            assert!(window.find(id).visible(), "missing system setting control {id}");
        }
        window.click("system-server-url", cx);
        window.press("cmd-a", cx);
        window.input("wss://custom.example/ws", cx);
        desktop.update(cx, |this, cx| {
            this.settings.time_zone.update(cx, |_, cx| {
                cx.emit(gpui_kit::component::select::SelectEvent::Confirm(Some("UTC")));
            });
        });
        window.click("system-platform-wx", cx);
        window.click("system-os-Android", cx);
        window.click("system-device-preset-iphone_test", cx);
        window.render_frame(cx);
    })
    .expect("render and operate the complete runtime environment form");

    cx.update(|cx| {
        let state = desktop.read(cx);
        let config = &state.settings.draft["systemConfig"];
        assert_eq!(config["serverUrl"], "wss://custom.example/ws");
        assert_eq!(config["timeZone"], "UTC");
        assert_eq!(config["platform"], "wx");
        assert_eq!(config["os"], "iOS");
        assert_eq!(config["deviceInfo"]["os"], "iOS");
        assert_eq!(config["clientVersion"], "ios-version");
        assert_eq!(config["deviceInfo"]["clientVersion"], "ios-version");
        assert_eq!(config["deviceInfo"]["deviceId"], "iPhone-test");
        assert_eq!(config["deviceInfo"]["userAgent"], "iPhone Agent");
        assert_eq!(state.settings.system_device_id.read(cx).value().as_ref(), "iPhone-test");
    });

    cx.update_window(window, |_, window, cx| {
        window.click("save-system-config", cx);
        window.render_frame(cx);
        assert!(window.find("settings-save-notice").visible());
    })
    .expect("save the edited runtime configuration");

    cx.update(|cx| {
        let state = desktop.read(cx);
        assert_eq!(state.settings.draft["plantingStrategy"], "preferred");
        assert_eq!(state.settings.draft["automation"]["farm"], false);
        assert_eq!(state.settings.draft["automation"]["steal"], true);
        assert_eq!(state.settings.draft["offlineReminder"]["title"], "unsaved reminder");
        assert_eq!(state.request_revision, 41);
    });

    let store_path = qq_farm_core::models::store::global_config::store_file();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let persisted = std::fs::read_to_string(&store_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .is_some_and(|store| {
                store.pointer("/systemConfig/serverUrl").and_then(serde_json::Value::as_str)
                    == Some("wss://custom.example/ws")
                    && store
                        .pointer("/systemConfig/deviceInfo/deviceId")
                        .and_then(serde_json::Value::as_str)
                        == Some("iPhone-test")
            });
        if persisted {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "system config did not reach store.json");
        std::thread::sleep(Duration::from_millis(10));
    }

    qq_farm_core::models::store::global_config::load_global_config()
        .expect("reload the persisted system configuration");
    let restored = qq_farm_app::admin::get_system_config();
    assert_eq!(restored["serverUrl"], "wss://custom.example/ws");
    assert_eq!(restored["timeZone"], "UTC");
    assert_eq!(restored["platform"], "wx");
    assert_eq!(restored["os"], "iOS");
    assert_eq!(restored["clientVersion"], "ios-version");
    assert_eq!(restored["deviceInfo"]["deviceId"], "iPhone-test");
    assert_eq!(restored["deviceInfo"]["userAgent"], "iPhone Agent");

    cx.update_window(window, |_, window, cx| {
        window.click("reset-system-config", cx);
        window.render_frame(cx);
        assert!(window.find("settings-save-notice").visible());
    })
    .expect("reset only the runtime environment settings");

    cx.update(|cx| {
        let state = desktop.read(cx);
        assert_eq!(
            state.settings.draft["systemConfig"],
            serde_json::json!(qq_farm_core::config::get_default_system_config())
        );
        assert_eq!(state.settings.draft["plantingStrategy"], "preferred");
        assert_eq!(state.settings.draft["automation"]["farm"], false);
        assert_eq!(state.settings.draft["offlineReminder"]["title"], "unsaved reminder");
        assert_eq!(state.request_revision, 41);
    });
}
