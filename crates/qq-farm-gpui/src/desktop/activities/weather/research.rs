use super::super::super::*;
use super::shared::{flag, image, item_summary, list, muted, number, panel, text};

pub(super) fn render(
    desktop: &FarmDesktop,
    snapshot: &serde_json::Value,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let research = snapshot.get("research").unwrap_or(&serde_json::Value::Null);
    let nodes = list(research, "nodes");
    if nodes.is_empty() {
        return div();
    }
    panel(cx)
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().font_medium().child(crate::i18n::text(
                    language,
                    "气象研究",
                    "Weather research",
                )))
                .child(muted(
                    format!(
                        "{} {} · {} {}",
                        crate::i18n::text(language, "雷电徽章", "Thunder badges"),
                        research
                            .get("badgeBalance")
                            .map(value_to_action_string)
                            .unwrap_or_else(|| "0".into()),
                        crate::i18n::text(language, "第", "Stage"),
                        research
                            .get("currentStage")
                            .map(value_to_action_string)
                            .unwrap_or_else(|| "--".into())
                    ),
                    cx,
                )),
        )
        .child(h_flex().items_stretch().gap_3().flex_wrap().children(nodes.iter().map(|node| {
            let id = number(node.get("id")).unwrap_or_default();
            let available = flag(node, "availableByStatus") && !flag(node, "completed");
            let reward = node.get("reward").unwrap_or(&serde_json::Value::Null);
            let cost = node.get("cost").unwrap_or(&serde_json::Value::Null);
            panel(cx)
                .min_w(rems(12.))
                .flex_1()
                .child(h_flex().justify_between().gap_2().child(muted(format!("#{id}"), cx)).child(
                    muted(
                        if flag(node, "completed") {
                            crate::i18n::text(language, "已完成", "Completed")
                        } else if available {
                            crate::i18n::text(language, "可推进", "Available")
                        } else {
                            crate::i18n::text(language, "未解锁", "Locked")
                        },
                        cx,
                    ),
                ))
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(image(reward, cx))
                        .child(div().text_sm().child(item_summary(reward, language))),
                )
                .child(muted(
                    format!(
                        "{} {}",
                        crate::i18n::text(language, "消耗", "Cost"),
                        item_summary(cost, language)
                    ),
                    cx,
                ))
                .when(available, |panel| {
                    panel.child(
                        Button::new(format!("weather-research-{id}"))
                            .label(crate::i18n::text(language, "推进", "Advance"))
                            .primary()
                            .loading(
                                desktop
                                    .activities
                                    .weather
                                    .pending_actions
                                    .contains(&format!("research:{id}")),
                            )
                            .disabled(desktop.loading || !flag(node, "affordable"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.advance_weather_research(id, cx)
                            })),
                    )
                })
        })))
        .when_some(
            text(
                snapshot.pointer("/actions/advanceResearch").unwrap_or(&serde_json::Value::Null),
                "reason",
            ),
            |panel, reason| panel.child(muted(crate::i18n::weather_reason(&reason, language), cx)),
        )
}
