use super::super::super::*;

pub(super) fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

pub(super) fn remaining_label(base: i64, now: i64, at: i64, language: &str) -> String {
    let value = base.saturating_sub(now.saturating_sub(at).max(0)).max(0);
    if value <= 0 {
        return crate::i18n::text(language, "进行中", "In progress").into();
    }
    let hours = value / 3600;
    let minutes = value % 3600 / 60;
    let seconds = value % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn start_weather_clock(&mut self, cx: &mut Context<Self>) {
        self.activities.weather.tick_generation =
            self.activities.weather.tick_generation.wrapping_add(1);
        let generation = self.activities.weather.tick_generation;
        let now = now_seconds();
        self.activities.weather.now_sec = now;
        self.activities.weather.loaded_at_sec = now;
        cx.notify();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let active = this.update(cx, |this, cx| {
                if generation != this.activities.weather.tick_generation
                    || this.page != Page::Activities
                    || this.page_section != "weather"
                {
                    return false;
                }
                this.activities.weather.now_sec = now_seconds();
                cx.notify();
                true
            });
            if !matches!(active, Ok(true)) {
                break;
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::remaining_label;
    #[test]
    fn countdown_uses_each_scan_timestamp_and_handles_expiry_and_clock_skew() {
        assert_eq!(remaining_label(65, 102, 100, "zh-CN"), "01:03");
        assert_eq!(remaining_label(65, 102, 102, "zh-CN"), "01:05");
        assert_eq!(remaining_label(1, 104, 100, "zh-CN"), "进行中");
        assert_eq!(remaining_label(1, 104, 100, "en-US"), "In progress");
        assert_eq!(remaining_label(59, 95, 100, "zh-CN"), "00:59");
        assert_eq!(remaining_label(3661, 100, 100, "zh-CN"), "1:01:01");
    }
}
