use super::super::super::super::*;
use crate::desktop::friend_action_type_number;

fn integer(value: Option<&serde_json::Value>) -> Option<i64> {
    friend_action_type_number(value).filter(|value| value.is_finite()).map(|value| value as i64)
}

fn visitor_name(record: &serde_json::Value) -> String {
    record
        .get("nick")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let gid = record
                .get("visitorGid")
                .map(|gid| match gid {
                    serde_json::Value::String(value) => value.clone(),
                    _ => gid.to_string(),
                })
                .unwrap_or_else(|| "undefined".into());
            format!("GID:{gid}")
        })
}

fn format_visitor_time(timestamp_ms: i64, now_ms: i64, language: &str) -> String {
    use chrono::TimeZone as _;

    if timestamp_ms == 0 {
        return "--".into();
    }
    let local = chrono::Local;
    let Some(time) = local.timestamp_millis_opt(timestamp_ms).single() else {
        return "--".into();
    };
    let now = local.timestamp_millis_opt(now_ms).single().unwrap_or_else(chrono::Local::now);
    let elapsed_ms = now.timestamp_millis().saturating_sub(timestamp_ms);
    if (0..60_000).contains(&elapsed_ms) {
        return crate::i18n::text(language, "刚刚", "Just now").into();
    }
    if (60_000..3_600_000).contains(&elapsed_ms) {
        let minutes = elapsed_ms / 60_000;
        return if language == "en-US" {
            format!("{minutes} min ago")
        } else {
            format!("{minutes} 分钟前")
        };
    }
    if time.date_naive() == now.date_naive() {
        return format!(
            "{} {}",
            crate::i18n::text(language, "今天", "Today"),
            time.format("%H:%M")
        );
    }
    time.format("%m/%d %H:%M").to_string()
}

fn visitor_timestamp(record: &serde_json::Value) -> i64 {
    integer(record.get("serverTimeMs"))
        .filter(|timestamp| *timestamp != 0)
        .or_else(|| {
            integer(record.get("serverTimeSec")).map(|seconds| seconds.saturating_mul(1000))
        })
        .unwrap_or_default()
}

impl FarmDesktop {
    pub(super) fn render_visitor_row(
        &self,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = &self.preferences.language;
        let record = &row.record;
        let name = visitor_name(record);
        let nick =
            record.get("nick").and_then(serde_json::Value::as_str).filter(|name| !name.is_empty());
        let avatar_url = record
            .get("avatarUrl")
            .and_then(serde_json::Value::as_str)
            .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
        let avatar = super::avatar::render_avatar(
            avatar_url,
            nick.and_then(|value| value.chars().next()).unwrap_or('?'),
            format!("friend-visitor-avatar-{index}"),
            44.,
            cx,
        );

        let action_label = record
            .get("actionLabel")
            .and_then(serde_json::Value::as_str)
            .filter(|label| !label.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| crate::i18n::text(language, "互动", "Interact").into());
        let action_color = match integer(record.get("actionType")) {
            Some(1) => cx.theme().info,
            Some(2) => cx.theme().success,
            Some(3) => cx.theme().danger,
            _ => cx.theme().muted_foreground,
        };
        let name_view = div()
            .id(format!("friend-visitor-name-{index}"))
            .truncate()
            .font_medium()
            .child(name.clone());
        #[cfg(test)]
        let name_view = name_view.test_support();
        let action_view = div()
            .id(format!("friend-visitor-action-{index}"))
            .rounded(cx.theme().radius)
            .bg(action_color.opacity(0.12))
            .px_2()
            .py_1()
            .text_xs()
            .text_color(action_color)
            .child(action_label.clone());
        #[cfg(test)]
        let action_view = action_view.test_support();
        let mut identity =
            h_flex().items_center().flex_wrap().gap_2().child(name_view).child(action_view);

        if let Some(level) = record.get("level").filter(|value| visitor_truthy(value)) {
            let level_view = div()
                .id(format!("friend-visitor-level-{index}"))
                .rounded(cx.theme().radius)
                .bg(cx.theme().background)
                .px_2()
                .py_1()
                .text_xs()
                .child(format!("Lv{}", display_value(level)));
            #[cfg(test)]
            let level_view = level_view.test_support();
            identity = identity.child(level_view);
        }
        if let Some(gid) = record.get("visitorGid").filter(|value| visitor_truthy(value)) {
            let gid_view = div()
                .id(format!("friend-visitor-gid-{index}"))
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("GID {}", display_value(gid)));
            #[cfg(test)]
            let gid_view = gid_view.test_support();
            identity = identity.child(gid_view);
        }

        let action_detail = record
            .get("actionDetail")
            .and_then(serde_json::Value::as_str)
            .filter(|detail| !detail.is_empty())
            .or_else(|| {
                record
                    .get("actionLabel")
                    .and_then(serde_json::Value::as_str)
                    .filter(|label| !label.is_empty())
            });
        let level_label = record
            .get("level")
            .filter(|value| visitor_truthy(value))
            .map(|value| format!("Lv{}", display_value(value)));
        let gid_label = record
            .get("visitorGid")
            .filter(|value| visitor_truthy(value))
            .map(|value| format!("GID {}", display_value(value)));
        let mut metadata = div().v_flex().min_w_0().flex_1().gap_1().child(identity);
        if let Some(detail) = action_detail {
            let detail_view = div()
                .id(format!("friend-visitor-detail-{index}"))
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(detail.to_owned());
            #[cfg(test)]
            let detail_view = detail_view.test_support();
            metadata = metadata.child(detail_view);
        }

        let timestamp = format_visitor_time(
            visitor_timestamp(record),
            chrono::Local::now().timestamp_millis(),
            language,
        );
        let accessible_label = [
            Some(name),
            Some(action_label),
            level_label,
            gid_label,
            action_detail.map(str::to_owned),
            Some(timestamp.clone()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
        let time_view = div()
            .id(format!("friend-visitor-time-{index}"))
            .flex_shrink_0()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(timestamp);
        #[cfg(test)]
        let time_view = time_view.test_support();
        let row = div()
            .id(format!("friend-row-{}", row.id))
            .aria_label(accessible_label)
            .h_flex()
            .items_start()
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_3()
            .child(avatar)
            .child(metadata)
            .child(time_view);
        #[cfg(test)]
        let row = row.test_support();
        row.into_any_element()
    }
}

fn visitor_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        serde_json::Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::format_visitor_time;
    use chrono::{Local, TimeZone as _};

    #[test]
    fn visitor_timestamps_follow_vue_relative_format_in_each_language() {
        let now_local = Local
            .from_local_datetime(
                &Local::now().date_naive().and_hms_opt(18, 0, 0).expect("valid local time"),
            )
            .single()
            .expect("unambiguous local time");
        let now = now_local.timestamp_millis();
        assert_eq!(format_visitor_time(now - 30_000, now, "zh-CN"), "刚刚");
        assert_eq!(format_visitor_time(now - 30_000, now, "en-US"), "Just now");
        assert_eq!(format_visitor_time(now - 5 * 60_000, now, "zh-CN"), "5 分钟前");
        assert_eq!(format_visitor_time(now - 5 * 60_000, now, "en-US"), "5 min ago");

        let earlier_today = now - 90 * 60_000;
        let earlier_today_local = Local.timestamp_millis_opt(earlier_today).single().unwrap();
        let expected_time = earlier_today_local.format("%H:%M");
        assert_eq!(
            format_visitor_time(earlier_today, now, "zh-CN"),
            format!("今天 {expected_time}"),
        );
        assert_eq!(
            format_visitor_time(earlier_today, now, "en-US"),
            format!("Today {expected_time}"),
        );

        let previous_day = now - 36 * 60 * 60 * 1000;
        let previous_day_local = Local.timestamp_millis_opt(previous_day).single().unwrap();
        assert_eq!(
            format_visitor_time(previous_day, now, "en-US"),
            previous_day_local.format("%m/%d %H:%M").to_string(),
        );
        assert_eq!(format_visitor_time(0, now, "en-US"), "--");
    }
}
