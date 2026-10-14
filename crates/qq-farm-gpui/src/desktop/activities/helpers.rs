pub(crate) fn value_to_action_string(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .unwrap_or_default()
}

pub(crate) fn activity_action_enabled(raw: &serde_json::Value, key: &str) -> Option<bool> {
    raw.pointer(&format!("/actions/{key}/enabled"))
        .and_then(serde_json::Value::as_bool)
        .or_else(|| {
            raw.pointer(&format!("/petDiary/actions/{key}/enabled"))
                .and_then(serde_json::Value::as_bool)
        })
        .or_else(|| {
            raw.pointer(&format!("/petDiary/actions/{key}/available"))
                .and_then(serde_json::Value::as_bool)
        })
        .or_else(|| {
            raw.pointer(&format!("/capabilities/{key}")).and_then(serde_json::Value::as_bool)
        })
}

pub(crate) fn activity_title_for_section(section: &str, language: &str) -> &'static str {
    let (chinese, english) = match section {
        "directory" => ("活动目录", "Activity directory"),
        "season" => ("赛季战令", "Season pass"),
        "constellation" => ("星座", "Constellation"),
        "shop" => ("星砂商店", "Star sand shop"),
        "solarTerms" => ("二十四节气", "Solar terms"),
        "qingmei" => ("青梅酿造", "Green plum brewing"),
        "qixi" => ("七夕鹊桥", "Magpie bridge"),
        "charity" => ("公益活动", "Charity"),
        "petDiary" => ("萌宠成长日记", "Pet growth journal"),
        "autumn" => ("秋日活动", "Autumn event"),
        "weather" => ("天气活动", "Weather event"),
        _ => ("活动详情", "Activity details"),
    };
    crate::i18n::text(language, chinese, english)
}

#[cfg(test)]
mod tests {
    use super::activity_title_for_section;

    #[test]
    fn activity_section_titles_follow_the_selected_language() {
        assert_eq!(activity_title_for_section("directory", "zh-CN"), "活动目录");
        assert_eq!(activity_title_for_section("directory", "en-US"), "Activity directory");
        assert_eq!(activity_title_for_section("petDiary", "en-US"), "Pet growth journal");
        assert_eq!(activity_title_for_section("unknown", "en-US"), "Activity details");
    }
}
