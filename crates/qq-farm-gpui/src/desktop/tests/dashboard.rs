use super::super::Page;
use super::support::open_test_desktop;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext as GpuiContextExt, TestAppContext};
use std::time::Duration;

#[gpui_kit::test]
async fn dashboard_log_search_filters_rows_in_the_dashboard_view(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
            desktop.update(cx, |this, _| {
                this.page = Page::Dashboard;
                this.selected_account = Some("7".into());
                this.accounts.records = vec![("7".into(), "测试账号".into(), true)];
                this.page_raw = serde_json::json!({
                    "status": {
                        "accountId": "7", "nick": "测试账号", "level": 8, "exp": 24,
                        "gold": 100, "runStatus": 1, "online": true, "uptime": 180,
                        "levelProgress": {"current": 24, "needed": 100},
                        "operations": {"harvest": 2}, "nextChecks": {}
                    },
                    "logs": [
                        {"ts": 1, "time": "12:00:00", "tag": "农场", "msg": "收获苹果", "isWarn": false, "meta": {"event": "harvest_crop"}},
                        {"ts": 2, "time": "12:00:01", "tag": "系统", "msg": "连接关闭", "isWarn": true, "meta": {"event": "ws_close"}}
                    ],
                    "bag": {"items": [], "systemItems": []}, "diamond": 0
                });
                this.rows = crate::pages::page_data(Page::Dashboard, this.page_raw.clone()).rows;
            });
        });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("farm-cycle").is_none());
        assert!(window.find("dashboard-log-search").visible());
        assert!(window.find("dashboard-event-filter").visible());
        window.click("dashboard-module-filter", cx);
        window.within("popup-menu").click(1_usize, cx);
    })
    .expect("select the dashboard log module");
    cx.wait_for(window, Duration::from_secs(1), |window, _| {
        window.try_find("popup-menu").is_none()
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        window.click("dashboard-level-filter", cx);
        window.within("popup-menu").click(2_usize, cx);
    })
    .expect("select the dashboard log level");
    cx.wait_for(window, Duration::from_secs(1), |window, _| {
        window.try_find("popup-menu").is_none()
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        window.click("dashboard-log-search", cx);
        window.input("收获", cx);
        window.render_frame(cx);
        assert_eq!(window.find("dashboard-log-search").value(), Some("收获"));
        assert!(window.find("clean").visible());
        window.click("clean", cx);
        window.render_frame(cx);
    })
    .expect("interact with dashboard view");
    let (module_filter, level_filter, keyword) = cx.update(|cx| {
        let desktop = desktop.read(cx);
        (
            desktop.dashboard.module_filter.clone(),
            desktop.dashboard.level_filter.clone(),
            desktop.dashboard.log_keyword.read(cx).value().to_string(),
        )
    });
    assert_eq!(module_filter, "farm");
    assert_eq!(level_filter, "warn");
    assert_eq!(keyword, "");
}

#[gpui_kit::test]
fn dashboard_labels_and_event_filter_follow_the_english_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Dashboard;
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "status": {
                    "accountId":"7", "nick":"Farmer", "level":"8", "exp":"24",
                    "gold":"100", "runStatus":1, "online":true, "uptime":"180",
                    "levelProgress":{"current":"24","needed":"100"},
                    "operations":{"harvest":"2"}, "nextChecks":{"farmRemainSec":"0"}
                },
                "logs":[
                    {"ts":1,"time":"12:00:00","tag":"农场","msg":"source=harvest_crop","isWarn":false,"meta":{"event":"harvest_crop"}}
                ],
                "bag":{"items":[{"id":"1002","count":"31"}],"systemItems":[]},
                "diamond":"5"
            });
            this.rows = crate::pages::page_data(Page::Dashboard, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("dashboard-module-filter").label(), Some("All modules"));
        assert_eq!(window.find("dashboard-level-filter").label(), Some("All levels"));
        assert_eq!(window.find("dashboard-event-filter").label(), Some("All events"));
        assert_eq!(window.find("dashboard-clear-logs").label(), Some("Clear"));
        assert!(window.find("dashboard-log-search").visible());
        assert_eq!(
            desktop.read(cx).dashboard.log_keyword.read(cx).presentation().placeholder().as_ref(),
            "Keyword"
        );
    })
    .expect("render the dashboard in English with numeric strings from the snapshot");
}

#[gpui_kit::test]
fn dashboard_keeps_logs_and_status_cards_available_without_an_account(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Dashboard;
            this.selected_account = None;
            this.page_raw = serde_json::json!({
                "logs": [
                    {"ts": 1, "time": "12:00:00", "tag": "系统", "msg": "待清理日志", "isWarn": false, "meta": {"event": "engine_started"}}
                ]
            });
            this.rows = crate::pages::page_data(Page::Dashboard, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("dashboard-no-account").visible());
        assert!(window.find("dashboard-logs-panel").visible());
        assert!(window.find("dashboard-schedule-card").visible());
        assert!(window.find("dashboard-operations-offline").visible());
        assert!(window.find("dashboard-log-search").visible());
        assert!(window.find("dashboard-clear-logs").visible());
        window.click("dashboard-clear-logs", cx);
        window.render_frame(cx);
        assert_eq!(window.find("dashboard-empty-logs").label(), Some("暂无实时事件"));
    })
    .expect("show dashboard panels without an account and clear global logs");

    let remaining_logs = cx.update(|cx| {
        desktop
            .read(cx)
            .rows
            .iter()
            .filter(|row| row.section == "logs" || row.section == "warnings")
            .count()
    });
    assert_eq!(remaining_logs, 0);
}

#[gpui_kit::test]
fn dashboard_caps_rendered_logs_and_reveals_earlier_ones_on_demand(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let logs: Vec<serde_json::Value> = (1..=305)
        .map(|index| {
            serde_json::json!({
                "ts": index,
                "time": "12:00:00",
                "tag": "农场",
                "msg": format!("运行事件 #{index}"),
                "isWarn": false,
                "meta": {"event": "harvest_crop"},
            })
        })
        .collect();
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Dashboard;
            this.selected_account = Some("7".into());
            this.accounts.records = vec![("7".into(), "测试账号".into(), true)];
            this.page_raw = serde_json::json!({
                "status": {
                    "accountId": "7", "nick": "测试账号", "level": 8, "exp": 24,
                    "gold": 100, "runStatus": 1, "online": true, "uptime": 180,
                    "levelProgress": {"current": 24, "needed": 100},
                    "operations": {"harvest": 2}, "nextChecks": {}
                },
                "logs": logs,
                "bag": {"items": [], "systemItems": []}, "diamond": 0
            });
            this.rows = crate::pages::page_data(Page::Dashboard, this.page_raw.clone()).rows;
        });
    });
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.dashboard.logs_scroll.scroll_to_bottom();
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        let offset = desktop.read(cx).dashboard.logs_scroll.offset().y.as_f32();
        let max = desktop.read(cx).dashboard.logs_scroll.max_offset().y.as_f32();
        assert!(offset < 0.0, "offsets after scroll_to_bottom: offset={offset} max={max}");
        assert!(window.find("dashboard-show-older-logs").visible());
        assert_eq!(window.find("dashboard-show-older-logs").label(), Some("显示更早的 5 条日志"));
        window.click("dashboard-show-older-logs", cx);
        window.render_frame(cx);
        assert!(window.try_find("dashboard-collapse-logs").is_some());
        assert!(window.try_find("dashboard-show-older-logs").is_none());
    })
    .expect("cap rendered logs and reveal the earlier ones on demand");
}
