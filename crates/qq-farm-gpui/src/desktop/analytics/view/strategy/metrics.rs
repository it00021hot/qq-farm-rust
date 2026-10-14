use serde_json::Value;

pub(super) const STRATEGIES: &[(&str, &str, &str, &str)] = &[
    ("max_exp", "最高经验", "expPerHour", "EXP/时"),
    ("max_profit", "最高收益", "profitPerHour", "金币/时"),
    ("max_fert_exp", "化肥经验", "normalFertilizerExpPerHour", "EXP/时"),
    ("max_fert_profit", "化肥收益", "normalFertilizerProfitPerHour", "金币/时"),
    ("level", "等级", "level", "Lv"),
];

pub(super) fn current_strategy_label(key: &str, language: &str) -> Option<&'static str> {
    let english = language == "en-US";
    match (key, english) {
        ("preferred", false) => Some("优先种植种子"),
        ("preferred", true) => Some("Preferred Seed"),
        ("level", false) => Some("最高等级作物"),
        ("level", true) => Some("Highest level crop"),
        ("max_exp", false) => Some("最大经验/时"),
        ("max_exp", true) => Some("Max Exp/h"),
        ("max_fert_exp", false) => Some("最大普通肥经验/时"),
        ("max_fert_exp", true) => Some("Max Fert Exp/h"),
        ("max_profit", false) => Some("最大净利润/时"),
        ("max_profit", true) => Some("Max Profit/h"),
        ("max_fert_profit", false) => Some("最大普通肥净利润/时"),
        ("max_fert_profit", true) => Some("Max Fert Profit/h"),
        ("bag_priority", false) => Some("背包种子优先"),
        ("bag_priority", true) => Some("Bag seed priority"),
        _ => None,
    }
}

pub(super) fn comparison_label(key: &str, language: &str) -> &'static str {
    let english = language == "en-US";
    match (key, english) {
        ("max_exp", false) => "经验/时",
        ("max_exp", true) => "Exp/h",
        ("max_profit", false) => "利润/时",
        ("max_profit", true) => "Profit/h",
        ("max_fert_exp", false) => "普肥经验/时",
        ("max_fert_exp", true) => "Fert Exp/h",
        ("max_fert_profit", false) => "普肥利润/时",
        ("max_fert_profit", true) => "Fert Profit/h",
        ("level", false) => "最高等级",
        ("level", true) => "Highest level",
        _ => "",
    }
}

pub(super) fn comparison_description(key: &str, language: &str) -> &'static str {
    let english = language == "en-US";
    match (key, english) {
        ("max_exp", false) => "每小时经验收益最高",
        ("max_exp", true) => "Highest experience per hour",
        ("max_profit", false) => "每小时净利润最高",
        ("max_profit", true) => "Highest net profit per hour",
        ("max_fert_exp", false) => "使用普通化肥后经验最高",
        ("max_fert_exp", true) => "Highest experience with normal fertilizer",
        ("max_fert_profit", false) => "使用普通化肥后利润最高",
        ("max_fert_profit", true) => "Highest profit with normal fertilizer",
        ("level", false) => "等级最高的作物",
        ("level", true) => "Highest-level plantable crop",
        _ => "",
    }
}

pub(super) fn comparison_unit(key: &str, language: &str) -> &'static str {
    let english = language == "en-US";
    match (key, english) {
        ("max_exp" | "max_fert_exp", false) => "EXP/时",
        ("max_exp" | "max_fert_exp", true) => "EXP/hour",
        ("max_profit" | "max_fert_profit", false) => "金币/时",
        ("max_profit" | "max_fert_profit", true) => "Coins/hour",
        ("level", _) => "Lv",
        _ => "",
    }
}

fn number(row: &Value, key: &str) -> Option<f64> {
    let value = row
        .get(key)
        .and_then(Value::as_f64)
        .or_else(|| row.get(key).and_then(Value::as_i64).map(|number| number as f64))
        .or_else(|| row.get(key).and_then(Value::as_bool).map(|value| u8::from(value) as f64))
        .or_else(|| {
            row.get(key).and_then(Value::as_str).map(str::trim).and_then(|value| {
                if value.is_empty() {
                    Some(0.)
                } else {
                    value.parse().ok()
                }
            })
        });
    value.filter(|value| value.is_finite())
}

pub(super) fn integer(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
            .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
    })
}

pub(super) fn reference_level(account_level: i64, typed_value: &str, user_override: bool) -> f64 {
    if !user_override {
        return account_level.clamp(1, 200) as f64;
    }

    match typed_value.trim().parse::<f64>().ok().filter(|value| value.is_finite()) {
        // Vue's `Number(strategyLevel.value || 0)` treats a cleared or invalid
        // input as zero; keep that distinct from the initial account-level sync.
        Some(value) => value.clamp(1., 200.),
        None => 0.,
    }
}

pub(super) fn value_text(row: &Value, key: &str) -> String {
    match row.get(key) {
        None | Some(Value::Null) => "-".into(),
        Some(Value::String(value)) => value.clone(),
        // Vue interpolates a JavaScript number, so an integral float such as
        // 12.0 is displayed as "12". `serde_json::Number::to_string()` keeps
        // the decimal suffix; normalize through f64 to match the page.
        Some(Value::Number(value)) => {
            value.as_f64().map_or_else(|| value.to_string(), |number| number.to_string())
        }
        Some(Value::Bool(value)) => value.to_string(),
        Some(value) => value.to_string(),
    }
}

pub(super) fn comparison_value_text(row: &Value, key: &str) -> String {
    match row.get(key) {
        None => "-".into(),
        Some(Value::Null) => "0".into(),
        Some(_) => number(row, key).map_or_else(|| "-".into(), |value| value.to_string()),
    }
}

pub(super) fn format_level(level: Option<&Value>, language: &str) -> String {
    let unknown = crate::i18n::text(language, "未知", "Unknown");
    match level {
        None | Some(Value::Null) => unknown.into(),
        Some(Value::String(value)) => {
            if value.is_empty() || value.trim().parse::<f64>().is_ok_and(|number| number < 0.) {
                unknown.into()
            } else {
                value.clone()
            }
        }
        Some(Value::Number(value)) => {
            let Some(number) = value.as_f64() else { return value.to_string() };
            if number < 0. {
                unknown.into()
            } else if number.fract() == 0. {
                format!("{number:.0}")
            } else {
                value.to_string()
            }
        }
        Some(value) => value.to_string(),
    }
}

pub(super) fn season_count(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => "1".into(),
        Some(Value::Number(number)) if number.as_f64() == Some(0.) => "1".into(),
        Some(Value::String(value)) if value.is_empty() => "1".into(),
        Some(Value::String(value)) => value.clone(),
        Some(value) => value.to_string(),
    }
}

pub(super) fn eligible(row: &Value, reference_level: f64) -> bool {
    match row.get("level") {
        None | Some(Value::Null) => true,
        Some(_) => number(row, "level").is_some_and(|required| required <= reference_level),
    }
}

pub(super) fn best_for<'a>(
    rankings: &'a [Value],
    metric: &str,
    reference_level: f64,
) -> Option<&'a Value> {
    let mut best: Option<(&Value, f64)> = None;
    for row in rankings.iter().filter(|row| eligible(row, reference_level)) {
        let value = if row.get(metric).is_none_or(Value::is_null) {
            -1.
        } else {
            number(row, metric).unwrap_or(f64::NEG_INFINITY)
        };
        if best.is_none_or(|(_, current)| value > current) {
            best = Some((row, value));
        }
    }
    best.map(|(row, _)| row)
}

#[cfg(test)]
mod tests {
    use super::{
        best_for, comparison_description, comparison_label, comparison_unit, comparison_value_text,
        current_strategy_label, eligible, format_level, reference_level, season_count, value_text,
    };

    #[test]
    fn strategy_copy_and_metric_units_follow_the_selected_language() {
        assert_eq!(current_strategy_label("max_exp", "en-US"), Some("Max Exp/h"));
        assert_eq!(current_strategy_label("preferred", "en-US"), Some("Preferred Seed"));
        assert_eq!(current_strategy_label("preferred", "zh-CN"), Some("优先种植种子"));
        assert_eq!(comparison_label("max_profit", "en-US"), "Profit/h");
        assert_eq!(
            comparison_description("max_fert_exp", "en-US"),
            "Highest experience with normal fertilizer"
        );
        assert_eq!(comparison_unit("max_profit", "en-US"), "Coins/hour");
    }

    #[test]
    fn tied_strategy_metrics_keep_the_first_ranking_like_vue_stable_sort() {
        let rows = serde_json::json!([
            {"name": "首项", "level": 1, "expPerHour": 100},
            {"name": "后项", "level": 1, "expPerHour": 100}
        ]);
        assert_eq!(best_for(rows.as_array().unwrap(), "expPerHour", 1.).unwrap()["name"], "首项");
    }

    #[test]
    fn missing_strategy_metric_uses_vue_minus_one_fallback() {
        let rows = serde_json::json!([
            {"name": "缺值", "level": 1},
            {"name": "负值", "level": 1, "expPerHour": -2}
        ]);
        assert_eq!(best_for(rows.as_array().unwrap(), "expPerHour", 1.).unwrap()["name"], "缺值");
    }

    #[test]
    fn analytics_values_accept_numeric_strings_and_vue_level_fallbacks() {
        let rows = serde_json::json!([
            {"name": "高经验", "level": "3", "expPerHour": "12.5"},
            {"name": "无效等级", "level": "未知", "expPerHour": 100}
        ]);
        let rows = rows.as_array().unwrap();
        assert_eq!(best_for(rows, "expPerHour", 3.).unwrap()["name"], "高经验");
        assert!(eligible(&rows[0], 3.));
        assert!(!eligible(&rows[1], 200.));
        assert_eq!(format_level(Some(&serde_json::json!(null)), "zh-CN"), "未知");
        assert_eq!(format_level(Some(&serde_json::json!(-1)), "zh-CN"), "未知");
        assert_eq!(format_level(Some(&serde_json::json!(12.0)), "zh-CN"), "12");
        assert_eq!(format_level(Some(&serde_json::json!("12.0")), "zh-CN"), "12.0");
        assert_eq!(format_level(Some(&serde_json::json!(null)), "en-US"), "Unknown");
        assert_eq!(value_text(&serde_json::json!({"metric": "12.5"}), "metric"), "12.5");
        assert_eq!(value_text(&serde_json::json!({"metric": 12.0}), "metric"), "12");
        assert_eq!(value_text(&serde_json::json!({"metric": 12}), "metric"), "12");
    }

    #[test]
    fn reference_level_matches_vue_for_account_sync_fractional_input_and_clear() {
        assert_eq!(reference_level(18, "1", false), 18.);
        assert_eq!(reference_level(18, "12.5", true), 12.5);
        assert_eq!(reference_level(18, "", true), 0.);
        assert_eq!(reference_level(18, "not a number", true), 0.);
        assert_eq!(reference_level(18, "250", true), 200.);
        assert_eq!(reference_level(18, "0", true), 1.);
    }

    #[test]
    fn cleared_and_fractional_reference_levels_filter_like_vue() {
        let rankings = [
            serde_json::json!({"level": 1}),
            serde_json::json!({"level": 12.5}),
            serde_json::json!({"level": null}),
            serde_json::json!({"level": 13}),
        ];

        let fractional = reference_level(18, "12.5", true);
        assert_eq!(rankings.iter().filter(|row| eligible(row, fractional)).count(), 3);

        let cleared = reference_level(18, "", true);
        assert_eq!(rankings.iter().filter(|row| eligible(row, cleared)).count(), 1);
    }

    #[test]
    fn season_count_preserves_vue_truthiness_for_zero_strings() {
        assert_eq!(season_count(None), "1");
        assert_eq!(season_count(Some(&serde_json::json!(0))), "1");
        assert_eq!(season_count(Some(&serde_json::json!(""))), "1");
        assert_eq!(season_count(Some(&serde_json::json!("0"))), "0");
        assert_eq!(season_count(Some(&serde_json::json!("2"))), "2");
    }

    #[test]
    fn comparison_metric_display_matches_vue_number_coercion_for_null_and_missing() {
        assert_eq!(comparison_value_text(&serde_json::json!({"metric": null}), "metric"), "0");
        assert_eq!(comparison_value_text(&serde_json::json!({}), "metric"), "-");
        assert_eq!(comparison_value_text(&serde_json::json!({"metric": "12.5"}), "metric"), "12.5");
        assert_eq!(
            comparison_value_text(&serde_json::json!({"metric": "not numeric"}), "metric"),
            "-"
        );
    }
}
