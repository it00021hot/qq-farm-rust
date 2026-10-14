use super::super::super::*;
use serde_json::Value;

use super::common::{item_label, items_line, section_card_with_extra, text, timestamp};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlunderExtra {
    Lost,
    Fake,
}

fn plunder_extras(log: &Value) -> Vec<PlunderExtra> {
    let mut extras = Vec::new();
    if log.get("lost").and_then(Value::as_array).is_some_and(|values| !values.is_empty()) {
        extras.push(PlunderExtra::Lost);
    }
    if log.get("fake").and_then(Value::as_bool) == Some(true) {
        extras.push(PlunderExtra::Fake);
    }
    extras
}

fn items_text(items: Option<&Value>, language: &str) -> String {
    let separator = if language == "en-US" { ", " } else { "、" };
    items
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|item| item_label(item, language)).collect::<Vec<_>>())
        .unwrap_or_default()
        .join(separator)
}

pub(super) fn render(this: &FarmDesktop, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let english = language == "en-US";
    let kind = this.activities.pet_diary_log_kind.clone();
    let kind_for_refresh = kind.clone();
    let logs = this
        .activities
        .pet_diary_records
        .get(&kind)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let refresh = Button::new("pet-diary-log-refresh")
        .label(crate::i18n::text(language, "刷新日志", "Refresh logs"))
        .ghost()
        .disabled(this.loading)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.load_pet_diary_records(
                if kind_for_refresh == "plunder" { "plunder" } else { "interact" },
                cx,
            )
        }));
    let mut panel = section_card_with_extra(this, "互动与被夺日志", "logs", refresh, cx);
    if !this.activities.pet_diary_expanded_sections.contains("logs") {
        return panel;
    }
    panel = panel.child(
        h_flex()
            .items_center()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("pet-diary-log-interact")
                    .label(crate::i18n::text(language, "互动日志", "Interaction log"))
                    .when(kind == "interact", |button| button.primary())
                    .when(kind != "interact", |button| button.ghost())
                    .disabled(this.loading)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.load_pet_diary_records("interact", cx)),
                    ),
            )
            .child(
                Button::new("pet-diary-log-plunder")
                    .label(crate::i18n::text(language, "被夺日志", "Plunder log"))
                    .when(kind == "plunder", |button| button.primary())
                    .when(kind != "plunder", |button| button.ghost())
                    .disabled(this.loading)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.load_pet_diary_records("plunder", cx)),
                    ),
            ),
    );

    if logs.is_empty() {
        panel = panel.child(div().text_sm().text_color(cx.theme().muted_foreground).child(
            if kind == "plunder" {
                crate::i18n::text(language, "暂无被夺日志", "No plunder records yet.")
            } else {
                crate::i18n::text(language, "暂无互动日志", "No interaction records yet.")
            },
        ));
    } else {
        for (index, log) in logs.iter().enumerate() {
            let time = timestamp(log.get("time")).unwrap_or_else(|| "--".into());
            if kind == "interact" {
                let kind_text = crate::i18n::text(language, "类型", "Type");
                let mut label = format!("{time} · {kind_text} {}", text(log.get("type"), "?"));
                let mut row = div()
                    .v_flex()
                    .gap_1()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_3()
                    .child(format!("{time} · {kind_text} {}", text(log.get("type"), "?")));
                if log.get("costs").and_then(Value::as_array).is_some_and(|v| !v.is_empty()) {
                    let cost_text = crate::i18n::text(language, "消耗", "Cost");
                    label.push_str(&format!(
                        " · {cost_text} {}",
                        items_text(log.get("costs"), language)
                    ));
                    row = row.child(
                        h_flex()
                            .flex_wrap()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(cost_text),
                            )
                            .child(items_line(log.get("costs"), language, cx)),
                    );
                }
                if log.get("rewards").and_then(Value::as_array).is_some_and(|v| !v.is_empty()) {
                    let reward_text = crate::i18n::text(language, "获得", "Rewards");
                    label.push_str(&format!(
                        " · {reward_text} {}",
                        items_text(log.get("rewards"), language)
                    ));
                    row = row.child(
                        h_flex()
                            .flex_wrap()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(reward_text),
                            )
                            .child(items_line(log.get("rewards"), language, cx)),
                    );
                }
                let row = row.id(format!("pet-diary-interact-row-{index}")).aria_label(label);
                #[cfg(test)]
                let row = row.test_support();
                panel = panel.child(row);
            } else {
                let won = log.get("won").and_then(Value::as_bool) == Some(true);
                let name = text(
                    log.get("name"),
                    crate::i18n::text(language, "未知玩家", "Unknown player"),
                );
                let level = text(log.get("level"), "?");
                let status = if won {
                    crate::i18n::text(language, "对方获胜", "Opponent won")
                } else {
                    crate::i18n::text(language, "对方失败", "Opponent lost")
                };
                let headline = if english {
                    format!("{time} · {name} (Lv. {level})")
                } else {
                    format!("{time} · {name}（Lv.{level}）")
                };
                let mut label = format!("{headline} · {status}");
                let mut row = div()
                    .v_flex()
                    .gap_1()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_3()
                    .child(headline)
                    .child(
                        div()
                            .text_xs()
                            .text_color(if won { cx.theme().danger } else { cx.theme().success })
                            .child(status),
                    );
                for extra in plunder_extras(log) {
                    match extra {
                        PlunderExtra::Lost => {
                            let lost = crate::i18n::text(language, "损失", "Lost");
                            label.push_str(&format!(
                                " · {lost} {}",
                                items_text(log.get("lost"), language)
                            ));
                            row = row.child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(lost),
                                    )
                                    .child(items_line(log.get("lost"), language, cx)),
                            );
                        }
                        PlunderExtra::Fake => {
                            let fake = crate::i18n::text(
                                language,
                                "移花接木假宝藏",
                                "Fake treasure from misdirection",
                            );
                            label.push_str(&format!(" · {fake}"));
                            row = row.child(
                                div().text_xs().text_color(cx.theme().muted_foreground).child(fake),
                            );
                        }
                    }
                }
                let row = row.id(format!("pet-diary-plunder-row-{index}")).aria_label(label);
                #[cfg(test)]
                let row = row.test_support();
                panel = panel.child(row);
            }
        }
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::{plunder_extras, PlunderExtra};
    use serde_json::json;

    #[test]
    fn plunder_rows_only_render_the_fields_vue_shows() {
        let rich = json!({
            "time": 1_760_000_000_000_i64,
            "name": "玩家A",
            "level": 12,
            "won": true,
            "challenge": {"id": 1, "name": "挑战书"},
            "injected": [{"id": 2, "name": "假宝藏"}],
            "lost": [{"id": 3, "name": "星砂", "count": 2}],
            "fake": true
        });
        assert_eq!(plunder_extras(&rich), vec![PlunderExtra::Lost, PlunderExtra::Fake]);

        let hidden_only = json!({"challenge": {"id": 1}, "injected": [{"id": 2}]});
        assert!(plunder_extras(&hidden_only).is_empty());
    }
}
