use super::super::super::Page;
use super::super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};

#[gpui_kit::test]
fn quiet_hours_use_24_hour_time_fields_and_follow_the_enable_switch(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let settings = serde_json::json!({
        "friendQuietHours": {
            "enabled": true,
            "start": "22:30",
            "end": "06:15",
            "continueFarm": true
        }
    });
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Settings;
            this.page_section = "strategy".into();
            this.selected_account = Some("7".into());
            this.settings.draft = settings.clone();
            this.page_raw = settings;
        });
    });

    let (start_id, end_id) = cx.update(|cx| {
        let state = desktop.read(cx);
        (
            ("time-field", state.settings.quiet_hours_start.entity_id()),
            ("time-field", state.settings.quiet_hours_end.entity_id()),
        )
    });
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_setting_selectors(window, cx));
        window.render_frame(cx);
        assert_eq!(window.find(start_id).value(), Some("22:30"));
        assert_eq!(window.find(end_id).value(), Some("06:15"));
        assert_ne!(window.find(start_id).disabled(), Some(true));

        window.click(start_id, cx);
        window.within("quiet-hours-start-field").click("hour", cx);
        assert_eq!(window.find(start_id).focused(), Some(true));
        for key in ["0", "8", "4", "5"] {
            window.press(key, cx);
        }
    })
    .expect("edit the quiet-hours time with the segmented keyboard control");
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(start_id).value(), Some("08:45"));
    })
    .expect("render the updated quiet-hours time");

    cx.update(|cx| {
        let state = desktop.read(cx);
        assert_eq!(state.settings.draft["friendQuietHours"]["start"], "08:45");
        assert_eq!(state.settings.draft["friendQuietHours"]["end"], "06:15");
    });

    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.set_setting_path_value("friendQuietHours/enabled", serde_json::json!(false), cx);
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(start_id, cx);
        window.press("up", cx);
        window.render_frame(cx);
        assert_eq!(window.find(start_id).value(), Some("08:45"));
        assert_eq!(window.find(end_id).value(), Some("06:15"));
    })
    .expect("disable both time fields with the Vue quiet-hours switch");
}
