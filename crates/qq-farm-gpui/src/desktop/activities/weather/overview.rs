use super::super::super::*;
use super::shared::{
    empty, flag, gate, image, item_label, item_summary, list, muted, number, panel, text,
};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::StatefulInteractiveElement;

pub(super) fn header(
    desktop: &FarmDesktop,
    snapshot: &serde_json::Value,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let own = snapshot.get("ownWeather").unwrap_or(&serde_json::Value::Null);
    let active = flag(own, "active");
    let summon_reason = text(
        snapshot.pointer("/actions/summonThunderstorm").unwrap_or(&serde_json::Value::Null),
        "reason",
    )
    .map(|reason| crate::i18n::weather_reason(&reason, language));
    let name = if !active {
        crate::i18n::text(language, "无特殊天气", "No special weather").to_string()
    } else if flag(own, "isThunderstorm") {
        crate::i18n::text(language, "雷雨", "Thunderstorm").to_string()
    } else {
        let weather_type = number(own.get("type")).map_or_else(|| "--".into(), |v| v.to_string());
        if language == "en-US" {
            format!("Special weather (type {weather_type})")
        } else {
            format!("特殊天气（类型 {weather_type}）")
        }
    };
    let remaining = super::clock::remaining_label(
        number(own.get("remainingSec")).unwrap_or_default(),
        desktop.activities.weather.now_sec,
        desktop.activities.weather.loaded_at_sec,
        language,
    );
    panel(cx)
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .gap_3()
                .child(
                    div().text_lg().font_semibold().child(
                        text(snapshot.get("activity").unwrap_or(&serde_json::Value::Null), "name")
                            .unwrap_or_else(|| {
                                crate::i18n::text(language, "雨落成诗", "Poem in the Rain").into()
                            }),
                    ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(muted(
                            if flag(snapshot, "active") {
                                crate::i18n::text(language, "进行中", "In progress")
                            } else {
                                crate::i18n::text(language, "未开放 / 已结束", "Not open / ended")
                            },
                            cx,
                        ))
                        .child(
                            Button::new("weather-refresh")
                                .label(crate::i18n::text(language, "刷新", "Refresh"))
                                .ghost()
                                .loading(desktop.loading)
                                .disabled(desktop.loading || desktop.selected_account.is_none())
                                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                        ),
                ),
        )
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .gap_3()
                .child(div().v_flex().gap_1().child(div().font_medium().child(name)).child(muted(
                    if active {
                        if language == "en-US" {
                            format!(
                                "Remaining {remaining}{}",
                                if flag(own, "collectedThisCycle") {
                                    " · collected this cycle"
                                } else {
                                    ""
                                }
                            )
                        } else {
                            format!(
                                "剩余 {remaining}{}",
                                if flag(own, "collectedThisCycle") {
                                    " · 本轮已采"
                                } else {
                                    ""
                                }
                            )
                        }
                    } else {
                        crate::i18n::text(
                            language,
                            "使用雷雨召唤瓶为农场召唤一场雷雨",
                            "Use a thunderstorm bottle to summon a storm for your farm.",
                        )
                        .into()
                    },
                    cx,
                )))
                .child(
                    Button::new("weather-summon")
                        .label(crate::i18n::text(language, "召唤雷雨", "Summon thunderstorm"))
                        .primary()
                        .loading(desktop.activities.weather.pending_actions.contains("summon"))
                        .disabled(desktop.loading || !gate(snapshot, "summonThunderstorm"))
                        .when_some(summon_reason, |button, reason| button.tooltip(reason))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_weather_action("summon", None, cx)
                        })),
                ),
        )
}

pub(super) fn supply(
    desktop: &FarmDesktop,
    snapshot: &serde_json::Value,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let mut view = panel(cx).child(div().font_medium().child(crate::i18n::text(
        language,
        "天气瓶补给",
        "Weather bottle supply",
    )));
    let Some(shop) = snapshot.get("shop").filter(|shop| shop.is_object()) else {
        return view.child(empty(
            crate::i18n::text(language, "暂未读取到补给目录", "Supply catalog has not loaded"),
            cx,
        ));
    };
    let item = shop.get("item").unwrap_or(&serde_json::Value::Null);
    let cost = shop.get("cost").unwrap_or(&serde_json::Value::Null);
    let available = flag(shop, "available");
    view = view
        .child(
            h_flex().items_center().gap_3().child(image(item, cx)).child(
                div()
                    .v_flex()
                    .gap_1()
                    .min_w_0()
                    .flex_1()
                    .child(div().text_sm().font_medium().child(item_summary(item, language)))
                    .child(muted(
                        format!(
                            "{} {}",
                            crate::i18n::text(language, "消耗", "Cost"),
                            item_summary(cost, language)
                        ),
                        cx,
                    ))
                    .child(muted(
                        format!(
                            "{} {}",
                            crate::i18n::text(language, "持有", "Owned"),
                            shop.get("balance")
                                .map(value_to_action_string)
                                .unwrap_or_else(|| "--".into())
                        ),
                        cx,
                    )),
            ),
        )
        .when(flag(shop, "owned"), |view| {
            view.child(muted(crate::i18n::text(language, "今日已兑换", "Exchanged today"), cx))
        })
        .child(
            Button::new("weather-exchange")
                .label(if !available {
                    text(shop, "reason")
                        .map(|reason| crate::i18n::weather_reason(&reason, language))
                        .unwrap_or_else(|| {
                            crate::i18n::text(language, "兑换采集瓶", "Exchange collector").into()
                        })
                } else {
                    crate::i18n::text(language, "兑换采集瓶", "Exchange collector").into()
                })
                .primary()
                .loading(desktop.activities.weather.pending_actions.contains("exchange"))
                .disabled(desktop.loading || !available || !gate(snapshot, "exchangeCollector"))
                .on_click(
                    cx.listener(|this, _, _, cx| this.run_weather_action("exchange", None, cx)),
                ),
        );
    view
}

pub(super) fn tasks(snapshot: &serde_json::Value, language: &str, cx: &App) -> Div {
    let tasks = list(snapshot, "tasks");
    let badge = snapshot
        .pointer("/research/badgeBalance")
        .map(value_to_action_string)
        .unwrap_or_else(|| "0".into());
    panel(cx)
        .child(
            h_flex()
                .justify_between()
                .gap_3()
                .child(div().font_medium().child(crate::i18n::text(
                    language,
                    "活动任务",
                    "Activity tasks",
                )))
                .child(muted(
                    format!(
                        "{} {badge}",
                        crate::i18n::text(language, "雷电徽章", "Thunder badges")
                    ),
                    cx,
                )),
        )
        .when(tasks.is_empty(), |panel| {
            panel.child(empty(crate::i18n::text(language, "暂无活动任务", "No activity tasks"), cx))
        })
        .children(tasks.iter().enumerate().map(|(index, task)| {
            let current = number(task.get("current")).unwrap_or_default().max(0);
            let limit = number(task.get("dailyLimit")).unwrap_or_default();
            let progress =
                if limit > 0 { (current as f32 / limit as f32).clamp(0., 1.) } else { 0. };
            let reward = task.get("reward").unwrap_or(&serde_json::Value::Null);
            let title = text(task, "title").unwrap_or_default();
            let display_title = if title.is_empty() {
                format!(
                    "{} {}",
                    crate::i18n::text(language, "任务", "Task"),
                    task.get("id").map(value_to_action_string).unwrap_or_default()
                )
            } else {
                title.clone()
            };
            let tooltip_title = title;
            h_flex()
                .id(("weather-task", index))
                .test_support()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .id(("weather-task-title", index))
                                .truncate()
                                .text_sm()
                                .child(display_title)
                                .tooltip(move |window, cx| {
                                    Tooltip::new(tooltip_title.clone()).build(window, cx)
                                }),
                        )
                        .child(
                            div()
                                .h(rems(0.3))
                                .w_full()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().muted)
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(progress))
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().primary),
                                ),
                        )
                        .child(muted(
                            format!(
                                "{current} / {}",
                                if limit > 0 { limit.to_string() } else { "--".into() }
                            ),
                            cx,
                        )),
                )
                .child(image(reward, cx))
                .child(div().text_sm().child(item_summary(reward, language)))
        }))
}

pub(super) fn inventory(snapshot: &serde_json::Value, language: &str, cx: &App) -> Div {
    let items = list(snapshot, "inventory");
    if items.is_empty() {
        return div();
    }
    panel(cx)
        .child(div().font_medium().child(crate::i18n::text(language, "活动物品", "Activity items")))
        .child(h_flex().gap_3().flex_wrap().children(items.iter().enumerate().map(
            |(index, item)| {
                let tooltip = item_label(item, language);
                h_flex()
                    .id(("weather-inventory-item", index))
                    .gap_2()
                    .items_center()
                    .child(image(item, cx))
                    .child(div().text_sm().child(item_summary(item, language)))
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            },
        )))
}
