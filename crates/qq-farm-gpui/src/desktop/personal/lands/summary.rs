use super::super::super::*;
use gpui_kit::TestSupportExt;

pub(super) fn render_career(career: &serde_json::Value, language: &str, cx: &App) -> Div {
    let harvest = json_number(career.get("harvest"));
    let steal = json_number(career.get("steal"));
    let ratio = if harvest <= 0 {
        if steal > 0 {
            "--".into()
        } else {
            "0%".into()
        }
    } else {
        format!("{:.1}%", steal as f64 / harvest as f64 * 100.0)
    };

    h_flex()
        .flex_wrap()
        .items_center()
        .gap_4()
        .rounded(cx.theme().radius)
        .bg(cx.theme().magenta.opacity(0.12))
        .px_4()
        .py_3()
        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
            language,
            "生涯统计",
            "Career",
        )))
        .child(career_value(
            "career-harvest",
            crate::i18n::text(language, "收获", "Harvested"),
            format_career_count(harvest, language),
            cx,
        ))
        .child(career_value(
            "career-steal",
            crate::i18n::text(language, "偷取", "Stolen"),
            format_career_count(steal, language),
            cx,
        ))
        .child(career_value(
            "career-ratio",
            crate::i18n::text(language, "收偷比", "Steal rate"),
            ratio,
            cx,
        ))
}

fn career_value(
    id: &'static str,
    label: &'static str,
    value: String,
    cx: &App,
) -> impl IntoElement {
    h_flex()
        .id(id)
        .items_center()
        .gap_2()
        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(label))
        .child(div().font_semibold().child(value))
        .test_support()
}

pub(super) fn render_land_summary(summary: &serde_json::Value, language: &str, cx: &App) -> Div {
    h_flex().flex_wrap().gap_2().children(
        [
            ("harvestable", "可收", "Harvestable"),
            ("growing", "生长", "Growing"),
            ("empty", "空闲", "Empty"),
            ("dead", "枯萎", "Dead"),
        ]
        .into_iter()
        .map(|(key, chinese, english)| {
            h_flex()
                .id(format!("land-summary-{key}"))
                .items_center()
                .gap_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .px_3()
                .py_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{}:", crate::i18n::text(language, chinese, english))),
                )
                .child(div().font_semibold().child(json_number(summary.get(key)).to_string()))
                .test_support()
        }),
    )
}

fn json_number(value: Option<&serde_json::Value>) -> i64 {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse::<i64>().ok()))
        .unwrap_or_default()
}

fn format_career_count(count: i64, language: &str) -> String {
    if count < 10_000 {
        return count.to_string();
    }
    if language == "en-US" {
        let (value, suffix) = if count >= 1_000_000 {
            (count as f64 / 1_000_000.0, "M")
        } else {
            (count as f64 / 1_000.0, "K")
        };
        let value = format!("{value:.1}");
        return format!("{}{}", value.trim_end_matches('0').trim_end_matches('.'), suffix);
    }
    let value = format!("{:.2}", count as f64 / 10_000.0);
    format!("{}万", value.trim_end_matches('0').trim_end_matches('.'))
}
