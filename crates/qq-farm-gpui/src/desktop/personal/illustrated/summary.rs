use super::super::super::*;
use gpui_kit::TestSupportExt;

pub(super) fn progress_ratio(book: &serde_json::Value) -> f32 {
    let progress = book.get("progress").and_then(serde_json::Value::as_f64).unwrap_or(0.);
    let target = book.get("nextLevelProgress").and_then(serde_json::Value::as_f64).unwrap_or(0.);
    if target > 0. {
        ((progress / target * 100.).round().clamp(0., 100.) / 100.) as f32
    } else {
        0.
    }
}

impl FarmDesktop {
    pub(super) fn render_illustrated_summary(
        &self,
        book: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> Div {
        let progress = book.get("progress").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let target =
            book.get("nextLevelProgress").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let ratio = progress_ratio(book);
        let progress_text = format!("{progress} / {target}");
        div()
            .v_flex()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().h_2().rounded_full().bg(cx.theme().muted).child(
                        div().h_full().w(relative(ratio)).rounded_full().bg(cx.theme().primary),
                    ))
                    .child(
                        div()
                            .id("illustrated-progress")
                            .aria_label(progress_text.clone())
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(progress_text)
                            .test_support(),
                    ),
            )
            .child(
                h_flex().flex_wrap().gap_2().children(
                    book.get("currentBuffs")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .enumerate()
                        .map(|(index, buff)| {
                            let name = buff
                                .get("name")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string)
                                .unwrap_or_else(|| {
                                    buff.get("name").map(ToString::to_string).unwrap_or_default()
                                });
                            let value =
                                buff.get("value").map(ToString::to_string).unwrap_or_default();
                            let percent = buff.get("valueType").and_then(serde_json::Value::as_str)
                                == Some("probability");
                            let label =
                                format!("{name} +{value}{}", if percent { "%" } else { "" });
                            let pill = div()
                                .id(format!("illustrated-buff-{index}"))
                                .aria_label(label.clone())
                                .rounded_full()
                                .bg(cx.theme().warning.opacity(0.16))
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_color(cx.theme().warning)
                                .child(label);
                            #[cfg(test)]
                            let pill = pill.test_support();
                            pill
                        }),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn collection_progress_rounds_and_caps_like_vue() {
        assert_eq!(
            super::progress_ratio(&serde_json::json!({"progress":1,"nextLevelProgress":3})),
            0.33
        );
        assert_eq!(
            super::progress_ratio(&serde_json::json!({"progress":10,"nextLevelProgress":3})),
            1.
        );
        assert_eq!(
            super::progress_ratio(&serde_json::json!({"progress":10,"nextLevelProgress":0})),
            0.
        );
    }
}
