use super::super::super::*;

use super::shared::{display_count, item_image, item_label, panel};

pub(super) fn balances(activity: &serde_json::Value, language: &str, cx: &App) -> Div {
    let balances = activity.get("balances").unwrap_or(&serde_json::Value::Null);
    let known = balances.get("known").and_then(serde_json::Value::as_bool) != Some(false);
    let sent = activity
        .pointer("/gift/sentCount")
        .map(|value| display_count(Some(value)))
        .unwrap_or_else(|| "0".into());
    let values = [
        ("feather", "feather", "鹊羽", "Magpie feather", false),
        ("sachet", "sachet", "鹊羽香囊", "Feather sachet", false),
        ("receivedSachet", "receivedSachet", "获赠香囊", "Received sachets", true),
    ];
    let cards = values.into_iter().enumerate().map(
        |(index, (key, item_key, fallback, english_fallback, fixed_label))| {
            let item = activity.get(item_key).unwrap_or(&serde_json::Value::Null);
            let balance = if known {
                balances
                    .get(key)
                    .map(|value| display_count(Some(value)))
                    .unwrap_or_else(|| "0".into())
            } else {
                "--".into()
            };
            let label = if fixed_label {
                crate::i18n::text(language, fallback, english_fallback).to_string()
            } else {
                item_label(item, fallback, english_fallback, language)
            };
            let label_element = div()
                .id(("qixi-balance-label", index))
                .aria_label(label.clone())
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label);
            #[cfg(test)]
            let label_element = label_element.test_support();
            panel(cx)
                .h_flex()
                .items_center()
                .gap_3()
                .child(item_image(item, 40., language, cx))
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .child(label_element)
                        .child(div().text_lg().font_semibold().child(balance)),
                )
        },
    );
    let sent_label = crate::i18n::text(language, "已赠好友", "Friends gifted");
    let sent_label_element = div()
        .id("qixi-sent-label")
        .aria_label(sent_label)
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(sent_label);
    #[cfg(test)]
    let sent_label_element = sent_label_element.test_support();
    let sent_card = panel(cx)
        .v_flex()
        .justify_center()
        .child(sent_label_element)
        .child(div().text_lg().font_semibold().child(sent));
    div().grid().grid_cols(4).gap_3().children(cards).child(sent_card)
}
