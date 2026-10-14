use std::time::Duration;

use super::super::super::*;

fn unix_time_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

impl FarmDesktop {
    pub(in crate::desktop) fn invalidate_personal_lands_clock(&mut self) {
        self.farm.lands.tick_generation = self.farm.lands.tick_generation.wrapping_add(1);
        self.farm.lands.refresh_generation = self.farm.lands.refresh_generation.wrapping_add(1);
        self.farm.lands.refreshing = false;
    }

    pub(in crate::desktop) fn start_personal_lands_clock(&mut self, cx: &mut Context<Self>) {
        if self.test_mode
            || self.page != Page::Personal
            || self.page_section != "lands"
            || self.page_raw.pointer("/lands/lands").and_then(serde_json::Value::as_array).is_none()
        {
            return;
        }

        self.invalidate_personal_lands_clock();
        let generation = self.farm.lands.tick_generation;
        let now = unix_time_seconds();
        self.farm.lands.now_sec = now;
        self.farm.lands.loaded_at_sec = now;

        cx.spawn(async move |this, cx| {
            let mut last_refresh_at = now;
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let now = unix_time_seconds();
                let refresh_due = now.saturating_sub(last_refresh_at) >= 60;
                let active = this.update(cx, |this, cx| {
                    if generation != this.farm.lands.tick_generation
                        || this.page != Page::Personal
                        || this.page_section != "lands"
                    {
                        return false;
                    }
                    this.farm.lands.now_sec = now;
                    if refresh_due && !this.loading {
                        this.refresh_personal_lands(cx);
                    }
                    cx.notify();
                    true
                });
                if !matches!(active, Ok(true)) {
                    break;
                }
                if refresh_due {
                    last_refresh_at = now;
                }
            }
        })
        .detach();
    }

    pub(in crate::desktop) fn refresh_personal_lands(&mut self, cx: &mut Context<Self>) {
        if self.test_mode
            || self.loading
            || self.farm.lands.refreshing
            || self.page != Page::Personal
            || self.page_section != "lands"
        {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else {
            return;
        };

        self.farm.lands.refresh_generation = self.farm.lands.refresh_generation.wrapping_add(1);
        let generation = self.farm.lands.refresh_generation;
        let revision = self.request_revision;
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        self.farm.lands.refreshing = true;
        self.farm.lands.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::farm::lands(&app, &account_id)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.farm.lands.refresh_generation {
                    return;
                }
                this.farm.lands.refreshing = false;
                if revision != this.request_revision
                    || this.page != Page::Personal
                    || this.page_section != "lands"
                {
                    if this.farm.lands.finish_operation_after_refresh {
                        this.farm.lands.finish_operation_after_refresh = false;
                        this.farm.lands.operating = false;
                    }
                    cx.notify();
                    return;
                }
                if this.farm.lands.finish_operation_after_refresh {
                    this.farm.lands.finish_operation_after_refresh = false;
                    this.farm.lands.operating = false;
                }
                match result {
                    Ok(Ok(lands)) => {
                        this.page_raw["lands"] =
                            serde_json::to_value(lands).unwrap_or(serde_json::Value::Null);
                        this.farm.lands.error = None;
                        this.start_personal_lands_clock(cx);
                    }
                    Ok(Err(error)) => this.farm.lands.error = Some(error),
                    Err(error) => {
                        this.farm.lands.error = Some(if this.preferences.language == "en-US" {
                            format!("Background task failed: {error}")
                        } else {
                            format!("后台任务已结束：{error}")
                        });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn personal_land_remaining_seconds(&self, land: &serde_json::Value) -> i64 {
        let base = land
            .get("matureInSec")
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse::<i64>().ok()))
            .unwrap_or_default();
        remaining_seconds(base, self.farm.lands.loaded_at_sec, self.farm.lands.now_sec)
    }
}

fn remaining_seconds(base: i64, loaded_at: i64, now: i64) -> i64 {
    let elapsed = now.saturating_sub(loaded_at).max(0);
    base.saturating_sub(elapsed).max(0)
}

pub(super) fn format_land_countdown(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = seconds % 3600 / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::{format_land_countdown, remaining_seconds};

    #[test]
    fn land_countdown_uses_vue_clock_format() {
        assert_eq!(format_land_countdown(65), "01:05");
        assert_eq!(format_land_countdown(3605), "1:00:05");
        assert_eq!(format_land_countdown(-1), "00:00");
    }

    #[test]
    fn land_countdown_decreases_from_load_time_and_stops_at_zero() {
        assert_eq!(remaining_seconds(30, 100, 110), 20);
        assert_eq!(remaining_seconds(5, 100, 120), 0);
        assert_eq!(remaining_seconds(5, 120, 100), 5);
    }
}
