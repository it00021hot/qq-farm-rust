#[derive(Clone)]
pub(crate) struct ActiveLanguage(pub(crate) String);

impl gpui_kit::Global for ActiveLanguage {}

pub(crate) fn text<'a>(language: &str, chinese: &'a str, english: &'a str) -> &'a str {
    if language == "en-US" {
        english
    } else {
        chinese
    }
}

pub(crate) fn background_task_error(language: &str, error: impl std::fmt::Display) -> String {
    error_with_detail(language, "后台任务已结束", "Background task failed", error)
}

pub(crate) fn error_with_detail(
    language: &str,
    chinese_prefix: &str,
    english_prefix: &str,
    detail: impl std::fmt::Display,
) -> String {
    let separator = if language == "en-US" { ": " } else { "：" };
    format!("{}{separator}{detail}", text(language, chinese_prefix, english_prefix))
}

pub(crate) fn weather_reason(reason: &str, language: &str) -> String {
    if language != "en-US" {
        return reason.to_string();
    }
    match reason {
        "金币不足" => "Not enough gold".into(),
        "雷电徽章不足" => "Not enough thunder badges".into(),
        "背包中没有可用的雷雨召唤瓶" => {
            "No thunderstorm bottle available in the inventory".into()
        }
        _ => reason.to_string(),
    }
}

pub(crate) fn activity_rule_title(title: &str, language: &str) -> String {
    match title {
        "活动说明" => text(language, "活动说明", "Activity rules").into(),
        "活动规则" => text(language, "活动规则", "Activity rules").into(),
        _ => title.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        activity_rule_title, background_task_error, error_with_detail, text, weather_reason,
    };

    #[test]
    fn unknown_languages_fall_back_to_chinese() {
        assert_eq!(text("en-US", "刷新", "Refresh"), "Refresh");
        assert_eq!(text("zh-CN", "刷新", "Refresh"), "刷新");
        assert_eq!(text("fr-FR", "刷新", "Refresh"), "刷新");
    }

    #[test]
    fn background_task_errors_follow_the_selected_language() {
        assert_eq!(background_task_error("zh-CN", "closed"), "后台任务已结束：closed");
        assert_eq!(background_task_error("en-US", "closed"), "Background task failed: closed");
    }

    #[test]
    fn contextual_errors_localize_the_prefix_and_separator() {
        assert_eq!(
            error_with_detail("zh-CN", "无法打开链接", "Could not open the link", "blocked"),
            "无法打开链接：blocked"
        );
        assert_eq!(
            error_with_detail("en-US", "无法打开链接", "Could not open the link", "blocked"),
            "Could not open the link: blocked"
        );
    }

    #[test]
    fn weather_gate_reasons_follow_the_selected_language() {
        assert_eq!(weather_reason("金币不足", "zh-CN"), "金币不足");
        assert_eq!(weather_reason("金币不足", "en-US"), "Not enough gold");
        assert_eq!(weather_reason("雷电徽章不足", "en-US"), "Not enough thunder badges");
        assert_eq!(
            weather_reason("背包中没有可用的雷雨召唤瓶", "en-US"),
            "No thunderstorm bottle available in the inventory"
        );
    }

    #[test]
    fn generic_activity_rule_titles_follow_the_selected_language() {
        assert_eq!(activity_rule_title("活动说明", "zh-CN"), "活动说明");
        assert_eq!(activity_rule_title("活动说明", "en-US"), "Activity rules");
        assert_eq!(activity_rule_title("雨落成诗规则", "en-US"), "雨落成诗规则");
    }
}
