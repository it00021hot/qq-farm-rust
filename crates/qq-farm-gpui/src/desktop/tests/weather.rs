use super::super::{activities::weather_fixture, FarmDesktop, Page};
use super::support::open_test_desktop;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{point, px, AppContext as _, Context, ScrollDelta, TestAppContext};
use serde_json::json;

fn seed_weather(this: &mut FarmDesktop, cx: &mut Context<FarmDesktop>) {
    this.page = Page::Activities;
    this.page_section = "weather".into();
    this.selected_account = Some("7".into());
    this.page_raw = weather_fixture();
    this.apply_weather_result(
        "scan",
        &json!({"friends":[
        {"gid":"12", "availability":"available", "canCollect":true,
            "eligibleCloudLandIds":["3"], "weather":{"active":true,"remainingSec":65}},
        {"gid":"34", "availability":"unavailable", "canCollect":false, "eligibleCloudLandIds":[]}
    ], "deferredGids":["56"]}),
    );
    this.activities.weather.now_sec = this.activities.weather.scanned_friends["12"].at_sec + 2;
    cx.notify();
}

#[gpui_kit::test]
fn weather_uses_friend_selection_detail_search_and_real_dto_gates(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, desktop, _restore) = open_test_desktop(cx);
    cx.update(|cx| desktop.update(cx, seed_weather));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("weather-summon").focused().is_none());
        assert!(window.find("weather-exchange").focused().is_none());
        assert!(window.find("weather-research-2").focused().is_none());
        assert!(window.try_find("weather-research-1").is_none());
        assert!(window.try_find("weather-research-3").is_none());
        assert!(window.try_find("weather-friend-gid").is_none());
        assert!(window.try_find("page-search").is_none());
        window.click("weather-summon", cx);
        window.click("weather-exchange", cx);
        window.click("weather-research-2", cx);
        assert!(desktop.read(cx).activities.weather.pending_actions.is_empty());
        assert!(desktop.read(cx).activities.weather.error.is_none());
        if !window.find("weather-friend-12").visible() {
            window.scroll("weather-research-2", ScrollDelta::Pixels(point(px(0.), px(-600.))), cx);
        }
        window.click("weather-friend-12", cx);
        assert!(window.find("weather-collect-selected").focused().is_some());
        assert!(window.find("weather-frog-selected").focused().is_none());
        assert!(window.find("weather-cloud-selected").focused().is_some());
        assert!(window.try_find("weather-friend-back").is_none());
        window.click("weather-friend-34", cx);
        assert!(window.find("weather-collect-selected").focused().is_none());
        assert!(window.find("weather-cloud-selected").focused().is_none());
        window.press("secondary-f", cx);
    })
    .expect("select weather friend and dispatch search command");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("weather-friend-search").focused(), Some(true));
        window.input("alice", cx);
        assert!(window.try_find("weather-friend-34").is_none());
        assert!(window.find("weather-friend-12").visible());
    })
    .expect("weather master and detail interaction");
    cx.update(|cx| {
        let this = desktop.read(cx);
        assert_eq!(this.activities.weather.selected_gid, "34");
        assert!(
            this.activities.weather.scanning_gids.is_empty(),
            "cached selection should not scan again"
        );
        assert!(!this.weather_action_available("summon", None));
        assert!(!this.weather_action_available("exchange", None));
        assert!(this.weather_action_available("collect", Some(12)));
        assert!(!this.weather_action_available("cloud", Some(34)));
        assert!(!this.weather_action_available("frog", Some(12)));
    });
}

#[gpui_kit::test]
fn weather_compact_detail_returns_to_list_and_restores_search_focus(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, desktop, _restore) = open_test_desktop(cx);
    cx.update(|cx| desktop.update(cx, seed_weather));
    cx.simulate_window_resize(handle, gpui_kit::size(px(1100.), px(720.)));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        if !window.find("weather-friend-12").visible() {
            window.scroll("weather-research-2", ScrollDelta::Pixels(point(px(0.), px(-600.))), cx);
        }
        window.click("weather-friend-12", cx);
        assert!(window.find("weather-friend-back").visible());
        assert!(window.try_find("weather-friend-search").is_none());
        window.click("weather-friend-back", cx);
    })
    .expect("return from weather detail");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("weather-friend-search").focused(), Some(true));
        window.click("weather-friend-12", cx);
        window.press("escape", cx);
    })
    .expect("dismiss compact detail with Escape");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("weather-friend-back").is_none());
        assert_eq!(window.find("weather-friend-search").focused(), Some(true));
    })
    .expect("compact weather detail has a return route");
}

#[gpui_kit::test]
fn weather_controls_and_feedback_follow_english_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, desktop, _restore) = open_test_desktop(cx);
    cx.update_window(handle, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.set_language("en-US", window, cx);
            seed_weather(this, cx);
        });
        window.render_frame(cx);
        assert_eq!(window.find("weather-summon").label(), Some("Summon thunderstorm"));
        assert_eq!(window.find("weather-rules-toggle").label(), Some("Activity rules"));
        assert_eq!(window.find("weather-exchange").label(), Some("Not enough gold"));
        assert_eq!(window.find("weather-research-2").label(), Some("Advance"));
        assert_eq!(
            desktop
                .read(cx)
                .activities
                .weather
                .search
                .read(cx)
                .presentation()
                .placeholder()
                .as_ref(),
            "Search friends by name or GID"
        );
        if !window.find("weather-friend-12").visible() {
            window.scroll("weather-research-2", ScrollDelta::Pixels(point(px(0.), px(-600.))), cx);
        }
        window.click("weather-friend-12", cx);
        assert_eq!(window.find("weather-collect-selected").label(), Some("Collect rain"));
        assert_eq!(window.find("weather-frog-selected").label(), Some("Frog mischief"));
        assert_eq!(window.find("weather-cloud-selected").label(), Some("Cloud mischief"));
        assert_eq!(window.find("weather-scan-selected").label(), Some("Rescan current weather"));

        desktop.update(cx, |this, _| {
            this.apply_weather_result(
                "cloud",
                &json!({
                    "landId": "3",
                    "use": {"rewards": [{"id": "1", "name": "Coins", "count": "20"}]}
                }),
            );
        });
        assert_eq!(
            desktop.read(cx).activities.weather.notice.as_deref(),
            Some("Cloud mischief completed (Plot 3): Coins ×20")
        );
    })
    .expect("weather page controls, status and feedback follow the selected locale");
}

#[gpui_kit::test]
fn weather_writes_merge_snapshot_and_friend_without_page_reload(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_handle, desktop, _restore) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            seed_weather(this, cx);
            let revision = this.request_revision;
            this.activities.weather.selected_gid = "12".into();
            this.apply_weather_result(
                "cloud",
                &json!({
                    "landId":"3", "use":{"rewards":[{"id":"1", "name":"金币", "count":"20"}]},
                    "friend":{"gid":"12", "canCollect":false, "availability":"collected"},
                    "snapshot":{"active":true, "ownWeather":{"active":true, "remainingSec":900},
                        "actions":{"collectWeather":{"enabled":false}}}
                }),
            );
            assert_eq!(this.request_revision, revision);
            assert_eq!(this.activities.weather.selected_gid, "12");
            assert_eq!(
                this.activities.weather.notice.as_deref(),
                Some("乌云使坏成功（地块 3）：金币 ×20")
            );
            assert!(!this.weather_action_available("collect", Some(12)));
            assert_eq!(
                this.page_raw.pointer("/weather/snapshot/ownWeather/remainingSec"),
                Some(&json!(900))
            );
            let generation = this.activities.weather.operation_generation;
            this.activities.weather.pending_actions.insert("cloud".into());
            this.activities.weather.invalidate_requests();
            assert_ne!(this.activities.weather.operation_generation, generation);
            assert!(this.activities.weather.pending_actions.is_empty());
            assert!(this.activities.weather.scanning_gids.is_empty());
        })
    });
}

#[gpui_kit::test]
fn reopening_weather_drops_unmounted_view_state_and_stale_snapshot(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore) = open_test_desktop(cx);
    cx.update(|cx| desktop.update(cx, seed_weather));
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| {
            this.activities.weather.rules_open = true;
            this.activities.weather.notice = Some("old notice".into());
            this.activities.weather.error = Some("old error".into());
            this.reset_weather_view(window, cx);

            assert!(this.activities.weather.selected_gid.is_empty());
            assert!(this.activities.weather.search.read(cx).value().is_empty());
            assert!(this.activities.weather.scanned_friends.is_empty());
            assert!(this.activities.weather.deferred_gids.is_empty());
            assert!(this.activities.weather.notice.is_none());
            assert!(this.activities.weather.error.is_none());
            assert!(!this.activities.weather.rules_open);
            assert!(this.page_raw.get("weather").is_none());
        });
    })
    .expect("re-entering weather creates a fresh view state before reloading its data");
}

#[gpui_kit::test]
fn weather_refresh_keeps_the_loaded_view_data_and_scanned_friend_cache(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore) = open_test_desktop(cx);
    cx.update(|cx| desktop.update(cx, seed_weather));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("weather-refresh").visible());
        window.click("weather-refresh", cx);
        window.render_frame(cx);
    })
    .expect("refresh weather from its activity card");
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.activities.weather.selected_gid = "12".into();
            let previous_revision = this.request_revision;
            let previous_snapshot = this.page_raw.pointer("/weather/snapshot").cloned();
            let previous_scans = this.activities.weather.scanned_friends.len();

            this.refresh(cx);

            assert!(this.request_revision > previous_revision);
            assert!(!this.loading);
            assert_eq!(this.page_raw.pointer("/weather/snapshot").cloned(), previous_snapshot);
            assert_eq!(this.activities.weather.selected_gid, "12");
            assert_eq!(this.activities.weather.scanned_friends.len(), previous_scans);
        });
    });
}

#[gpui_kit::test]
fn weather_snapshot_failure_hides_stale_sections_and_keeps_retry(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            seed_weather(this, cx);
            this.error = Some("Weather request failed".into());
            if let Some(weather) =
                this.page_raw.get_mut("weather").and_then(serde_json::Value::as_object_mut)
            {
                weather.remove("snapshot");
            }
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("weather-error").visible());
        assert!(window.find("weather-refresh").visible());
        assert!(window.try_find("weather-summon").is_none());
        assert!(window.try_find("weather-friend-search").is_none());
        window.click("weather-refresh", cx);
        window.render_frame(cx);
        assert!(window.try_find("weather-error").is_none());
        assert!(window.find("weather-refresh").visible());
    })
    .expect("weather failure should hide stale controls and allow a local retry");

    cx.update(|cx| {
        let this = desktop.read(cx);
        assert!(this.error.is_none());
        assert!(!this.loading);
        assert_eq!(this.page_section, "weather");
    });
}
