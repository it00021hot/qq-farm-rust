use super::super::*;

pub(super) fn metric(label: &str, value: String, change: String, cx: &Context<FarmDesktop>) -> Div {
    metric_colored(label, value, change, cx.theme().foreground, cx)
}

/// 带 value 颜色的指标块（对齐 Tauri：金币橙 / 点券绿 / 钻石蓝 / 金豆豆橙）
pub(super) fn metric_colored(
    label: &str,
    value: String,
    change: String,
    value_color: gpui_kit::Hsla,
    cx: &Context<FarmDesktop>,
) -> Div {
    div()
        .v_flex()
        .min_w(px(88.))
        .gap_1()
        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(label.to_string()))
        .child(div().text_lg().font_semibold().text_color(value_color).child(value))
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

/// 服务端等级升级预测（bot 62e1880 LevelForecast.vue）渲染行：
/// [预测文案 + 依据, 已种植排期（有排期时）, 手动提示（非自动时）]
pub(super) fn level_forecast_lines(
    forecast: Option<&serde_json::Value>,
    language: &str,
) -> Vec<String> {
    let english = language == "en-US";
    let Some(forecast) = forecast else { return Vec::new() };
    let plots = forecast.get("plotCount").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let automatic = forecast.get("automatic").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let scheduled = forecast.get("scheduledExp").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let text = match forecast.get("estimatedAt").and_then(serde_json::Value::as_i64) {
        None => {
            if plots > 0 {
                if english {
                    "Scheduled harvests not enough to level up".to_string()
                } else {
                    "本轮收获不足升级".to_string()
                }
            } else if english {
                "Waiting for forecast data".to_string()
            } else {
                "等待预测数据".to_string()
            }
        }
        Some(estimated_at) => {
            let source = forecast.get("source").and_then(serde_json::Value::as_str).unwrap_or("");
            if source == "ready" {
                if english {
                    "Ready to level up after harvest".to_string()
                } else {
                    "收获后可升级".to_string()
                }
            } else {
                let generated_at =
                    forecast.get("generatedAt").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let minutes =
                    (((estimated_at - generated_at).max(0) as f64) / 60_000.).ceil().max(1.);
                let duration = if minutes < 60. {
                    format!("{} min", minutes as i64)
                } else {
                    format!("{:.1} h", minutes / 60.)
                };
                if english {
                    if source == "planted" {
                        format!("Level up after harvest in ~{duration}")
                    } else {
                        format!("Level up in ~{duration}")
                    }
                } else if source == "planted" {
                    format!("约 {duration}后收获可升级")
                } else {
                    format!("约 {duration}后升级")
                }
            }
        }
    };
    let basis = match forecast.get("source").and_then(serde_json::Value::as_str) {
        Some("planted") => {
            if english {
                "by current crops"
            } else {
                "按当前作物"
            }
        }
        Some("ready") => {
            if english {
                "ready to harvest"
            } else {
                "待收获"
            }
        }
        Some("mixed") => {
            if english {
                "harvest schedule + history rate"
            } else {
                "收获排期 + 历史均值"
            }
        }
        Some("history") => {
            if english {
                "history rate estimate"
            } else {
                "历史均值估算"
            }
        }
        _ => "",
    };
    let mut lines =
        vec![if basis.is_empty() { text.to_string() } else { format!("{text} · {basis}") }];
    if plots > 0 {
        lines.push(if english {
            format!("{plots} plots planted · ~{} EXP scheduled", format_number(scheduled))
        } else {
            format!("已种植 {plots} 处 · 预计 {} 经验", format_number(scheduled))
        });
        if !automatic {
            lines.push(if english {
                "Harvest timing depends on manual actions or resuming auto farm".into()
            } else {
                "收获时间取决于手动操作或恢复自动农场".into()
            });
        }
    }
    lines
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
        account_avatar_url, account_display_name, bag_count, check_value, format_clock,
        format_number, int_at, level_forecast_lines, operation_label,
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
    fn level_forecast_lines_match_bot_level_forecast_component() {
        let generated = 1_800_000_000_000i64;
        // 已排期够升：planted → 「约 X 后收获可升级 · 按当前作物」+ 排期行
        let planted = json!({
            "generatedAt": generated,
            "remainingExp": 12_660,
            "scheduledExp": 20_544,
            "plotCount": 24,
            "readyExp": 0,
            "estimatedAt": generated + 21_614_000i64,
            "source": "planted",
            "automatic": true,
        });
        assert_eq!(
            level_forecast_lines(Some(&planted), "zh-CN"),
            vec!["约 6.0 h后收获可升级 · 按当前作物", "已种植 24 处 · 预计 20,544 经验",]
        );
        // ready：不显示时长
        let ready = json!({ "generatedAt": generated, "estimatedAt": generated, "plotCount": 1,
            "scheduledExp": 500, "source": "ready", "automatic": false });
        assert_eq!(
            level_forecast_lines(Some(&ready), "zh-CN"),
            vec![
                "收获后可升级 · 待收获",
                "已种植 1 处 · 预计 500 经验",
                "收获时间取决于手动操作或恢复自动农场",
            ]
        );
        // 不足升级 / 等待数据
        let not_enough = json!({ "plotCount": 3, "scheduledExp": 100, "source": "unavailable", "automatic": true });
        assert_eq!(
            level_forecast_lines(Some(&not_enough), "en-US"),
            vec![
                "Scheduled harvests not enough to level up",
                "3 plots planted · ~100 EXP scheduled"
            ]
        );
        let waiting = json!({ "plotCount": 0, "source": "unavailable" });
        assert_eq!(level_forecast_lines(Some(&waiting), "zh-CN"), vec!["等待预测数据"]);
        assert_eq!(level_forecast_lines(None, "zh-CN"), Vec::<String>::new());
        // 历史外推分钟取整（bot：ceil 到分钟，最少 1 分钟）
        let history = json!({
            "generatedAt": generated,
            "estimatedAt": generated + 90_000,
            "plotCount": 0,
            "source": "history",
            "automatic": true,
        });
        assert_eq!(
            level_forecast_lines(Some(&history), "en-US"),
            vec!["Level up in ~2 min · history rate estimate"]
        );
    }
}
