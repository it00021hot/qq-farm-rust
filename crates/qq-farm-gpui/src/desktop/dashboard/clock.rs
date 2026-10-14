use super::super::*;
use std::time::Duration;

use super::format;

impl FarmDesktop {
    pub(in crate::desktop) fn start_dashboard_clock(&mut self, cx: &mut Context<Self>) {
        if self.test_mode {
            return;
        }
        self.dashboard.tick_generation = self.dashboard.tick_generation.wrapping_add(1);
        let generation = self.dashboard.tick_generation;
        self.dashboard.bag_refresh_in_secs = 30;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let active = this.update(cx, |this, cx| {
                if generation != this.dashboard.tick_generation || this.page != Page::Dashboard {
                    return false;
                }
                let mut refresh_bag = false;
                let status = &mut this.page_raw["status"];
                let online = format::int_at(status, &["runStatus"]) == 1;
                if online {
                    if let Some(uptime) = status.get_mut("uptime") {
                        if let Some(value) = uptime.as_u64() {
                            *uptime = serde_json::json!(value.saturating_add(1));
                        }
                    }
                    if let Some(next_checks) = status.get_mut("nextChecks") {
                        for key in ["farmRemainSec", "helpRemainSec", "stealRemainSec"] {
                            if let Some(value) = next_checks.get_mut(key) {
                                if let Some(seconds) = value.as_i64() {
                                    *value = serde_json::json!(seconds.saturating_sub(1).max(0));
                                }
                            }
                        }
                    }
                    if this.dashboard.bag_refresh_in_secs <= 1 {
                        this.dashboard.bag_refresh_in_secs = 30;
                        refresh_bag = true;
                    } else {
                        this.dashboard.bag_refresh_in_secs -= 1;
                    }
                    cx.notify();
                } else {
                    this.dashboard.bag_refresh_in_secs = 30;
                }
                if refresh_bag {
                    this.schedule_dashboard_bag_refresh(cx);
                }
                true
            });
            if !matches!(active, Ok(true)) {
                break;
            }
        })
        .detach();
    }
}
