use super::super::*;

pub(super) fn metric(label: &str, value: String, change: String, cx: &Context<FarmDesktop>) -> Div {
    div()
        .v_flex()
        .min_w(px(88.))
        .gap_1()
        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(label.to_string()))
        .child(div().text_lg().font_semibold().child(value))
        .when(!change.is_empty() && change != "0", |metric| {
            metric.child(
                div()
                    .text_xs()
                    .text_color(if change.starts_with('-') {
                        cx.theme().danger
                    } else {
                        cx.theme().green
                    })
                    .child(if change.starts_with('-') { change } else { format!("+{change}") }),
            )
        })
}

pub(super) fn check_row(
    language: &str,
    label: &str,
    next_checks: &serde_json::Value,
    remain_key: &str,
    quiet_key: &str,
    online: bool,
    cx: &Context<FarmDesktop>,
) -> Div {
    let quiet = next_checks.get(quiet_key).and_then(serde_json::Value::as_bool) == Some(true);
    let remaining = next_checks.get(remain_key).and_then(number_i64).unwrap_or_default();
    let value = check_value(language, quiet, online, remaining);
    h_flex().justify_between().items_center().child(label.to_string()).child(
        div()
            .text_sm()
            .font_medium()
            .text_color(if quiet { cx.theme().warning } else { cx.theme().foreground })
            .child(value),
    )
}

fn check_value(language: &str, quiet: bool, online: bool, remaining: i64) -> String {
    if quiet {
        crate::i18n::text(language, "静默中", "Quiet").to_string()
    } else if !online {
        crate::i18n::text(language, "账号未登录", "Not logged in").to_string()
    } else if remaining <= 0 {
        crate::i18n::text(language, "巡查中…", "Checking…").to_string()
    } else {
        format_clock(remaining, language)
    }
}

pub(super) fn int_at(value: &serde_json::Value, keys: &[&str]) -> i64 {
    keys.iter()
        .find_map(|key| value.pointer(&format!("/{}", key.replace('.', "/"))).and_then(number_i64))
        .unwrap_or_default()
}

pub(super) fn number_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
}

pub(super) fn string_at(value: &serde_json::Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            value
                .pointer(&format!("/{}", key.replace('.', "/")))
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or("")
        .to_string()
}

pub(super) fn account_display_name(nickname: &str, account_name: &str, language: &str) -> String {
    match (nickname.is_empty(), account_name.is_empty()) {
        (true, true) => crate::i18n::text(language, "未登录", "Not logged in").to_string(),
        (true, false) => account_name.to_string(),
        (false, true) => nickname.to_string(),
        (false, false) if nickname != account_name => format!("{nickname} ({account_name})"),
        (false, false) => nickname.to_string(),
    }
}

pub(super) fn account_avatar_url(
    status_avatar: &str,
    saved_avatar: Option<&str>,
) -> Option<String> {
    [status_avatar, saved_avatar.unwrap_or_default()]
        .into_iter()
        .map(str::trim)
        .find(|url| {
            url.starts_with("https://")
                || url.starts_with("http://")
                || url.starts_with("//")
                || url.starts_with("data:image/")
        })
        .map(|url| if url.starts_with("//") { format!("https:{url}") } else { url.into() })
}

pub(super) fn bag_count(bag: &serde_json::Value, id: i64) -> i64 {
    bag.pointer("/items")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            bag.pointer("/systemItems").and_then(serde_json::Value::as_array).into_iter().flatten(),
        )
        .find(|item| item.get("id").and_then(number_i64) == Some(id))
        .and_then(|item| item.get("count"))
        .and_then(number_i64)
        .unwrap_or_default()
}

pub(super) fn bucket_time(bag: &serde_json::Value, id: i64) -> String {
    let item = bag
        .pointer("/items")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            bag.pointer("/systemItems").and_then(serde_json::Value::as_array).into_iter().flatten(),
        )
        .find(|item| item.get("id").and_then(number_i64) == Some(id));
    let Some(item) = item else { return "0.0h".into() };
    if let Some(text) =
        item.get("hoursText").and_then(serde_json::Value::as_str).filter(|text| !text.is_empty())
    {
        text.replace("小时", "h")
    } else {
        format!("{:.1}h", item.get("count").and_then(number_i64).unwrap_or_default() as f64 / 3600.)
    }
}

pub(super) fn value_text(value: &serde_json::Value) -> String {
    number_i64(value).map_or_else(|| "-".into(), |number| number.to_string())
}

pub(super) fn format_clock(seconds: i64, language: &str) -> String {
    let seconds = seconds.max(0);
    let days = seconds / 86_400;
    let hours = seconds % 86_400 / 3600;
    let minutes = seconds % 3600 / 60;
    let seconds = seconds % 60;
    if days > 0 && language == "en-US" {
        format!("{days}d {hours:02}:{minutes:02}:{seconds:02}")
    } else if days > 0 {
        format!("{days}天 {hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}

pub(super) fn format_number(value: i64) -> String {
    let digits = value.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(ch);
    }
    output
}

pub(super) fn estimate_level_up(
    gained_exp: i64,
    uptime_seconds: i64,
    progress_current: i64,
    progress_needed: i64,
    language: &str,
) -> String {
    if progress_needed == 0 || uptime_seconds == 0 || gained_exp <= 0 {
        return String::new();
    }

    let rate_per_hour = gained_exp as f64 / (uptime_seconds as f64 / 3600.);
    if rate_per_hour <= 0. {
        return String::new();
    }
    let remaining_minutes =
        (progress_needed as f64 - progress_current as f64) / (rate_per_hour / 60.);
    if remaining_minutes < 60. {
        let remaining_minutes = remaining_minutes.ceil() as i64;
        if language == "en-US" {
            format!("About {remaining_minutes} min to level up")
        } else {
            format!("约 {remaining_minutes} 分钟后升级")
        }
    } else if language == "en-US" {
        format!("About {:.1} h to level up", remaining_minutes / 60.)
    } else {
        format!("约 {:.1} 小时后升级", remaining_minutes / 60.)
    }
}

pub(super) fn operation_label(key: &str, language: &str) -> String {
    let english = language == "en-US";
    match key {
        "harvest" => format!("🌾 {}", crate::i18n::text(language, "收获", "Harvest")),
        "farming" => format!("🧑‍🌾 {}", crate::i18n::text(language, "一键务农", "Farm all")),
        "fertilize" => format!("🧪 {}", crate::i18n::text(language, "施肥", "Fertilize")),
        "plant" => format!("🌱 {}", crate::i18n::text(language, "种植", "Plant")),
        "steal" => format!("🏃 {}", crate::i18n::text(language, "偷菜", "Steal")),
        "helpFarming" => format!("🤝 {}", crate::i18n::text(language, "帮务农", "Help")),
        "taskClaim" => format!("✅ {}", crate::i18n::text(language, "任务", "Tasks")),
        "sell" => format!("💰 {}", crate::i18n::text(language, "出售", "Sell")),
        _ if english => key.replace('_', " "),
        _ => key.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        account_avatar_url, account_display_name, bag_count, check_value, estimate_level_up,
        format_clock, format_number, int_at, operation_label,
    };
    use serde_json::json;

    #[test]
    fn dashboard_snapshot_numbers_accept_numeric_strings_like_vue() {
        let status = json!({"level":"12", "levelProgress":{"current":"75"}});
        assert_eq!(int_at(&status, &["level"]), 12);
        assert_eq!(int_at(&status, &["levelProgress.current"]), 75);
        assert_eq!(bag_count(&json!({"items":[{"id":"1002", "count":"31"}]}), 1002), 31);
        assert_eq!(
            format_number(bag_count(&json!({"items":[{"id":1002,"count":"1200"}]}), 1002)),
            "1,200"
        );
    }

    #[test]
    fn dashboard_labels_and_check_countdowns_follow_the_locale() {
        assert_eq!(format_clock(90_061, "zh-CN"), "1天 01:01:01");
        assert_eq!(format_clock(90_061, "en-US"), "1d 01:01:01");
        assert_eq!(operation_label("harvest", "en-US"), "🌾 Harvest");
        assert_eq!(operation_label("harvest", "zh-CN"), "🌾 收获");
        assert_eq!(check_value("en-US", true, true, 10), "Quiet");
        assert_eq!(check_value("en-US", false, false, 10), "Not logged in");
        assert_eq!(check_value("en-US", false, true, 0), "Checking…");
        assert_eq!(check_value("zh-CN", false, false, 10), "账号未登录");
        assert_eq!(check_value("zh-CN", false, true, 0), "巡查中…");
    }

    #[test]
    fn dashboard_account_name_uses_vue_nickname_account_and_empty_fallbacks() {
        assert_eq!(
            account_display_name("Farm Nick", "Saved Account", "zh-CN"),
            "Farm Nick (Saved Account)"
        );
        assert_eq!(account_display_name("", "Saved Account", "zh-CN"), "Saved Account");
        assert_eq!(account_display_name("Farm Nick", "", "en-US"), "Farm Nick");
        assert_eq!(account_display_name("", "", "en-US"), "Not logged in");
    }

    #[test]
    fn dashboard_avatar_falls_back_to_the_saved_account_profile_like_vue() {
        assert_eq!(
            account_avatar_url("", Some("https://example.invalid/saved.png")),
            Some("https://example.invalid/saved.png".into())
        );
        assert_eq!(
            account_avatar_url("//example.invalid/live.png", Some("https://saved.invalid/a.png")),
            Some("https://example.invalid/live.png".into())
        );
        assert_eq!(account_avatar_url("invalid", Some(" ")), None);
    }

    #[test]
    fn level_up_estimate_matches_vue_fractional_rate_and_progress_edges() {
        assert_eq!(estimate_level_up(1, 7200, 99, 100, "en-US"), "About 2.0 h to level up");
        assert_eq!(estimate_level_up(1, 3600, 101, 100, "zh-CN"), "约 -60 分钟后升级");
        assert_eq!(estimate_level_up(2, 3600, 99, 100, "zh-CN"), "约 30 分钟后升级");
        assert_eq!(estimate_level_up(0, 3600, 99, 100, "en-US"), "");
    }
}
