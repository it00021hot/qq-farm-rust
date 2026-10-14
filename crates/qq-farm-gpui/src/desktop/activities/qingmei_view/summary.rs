use super::super::super::*;

use super::shared::{number, panel, text};

pub(super) fn header(activity: &serde_json::Value, language: &str, cx: &App) -> Div {
    let title = text(activity, "name")
        .unwrap_or_else(|| crate::i18n::text(language, "青梅酿造", "Green plum brewing").into());
    let known = activity.get("known").and_then(serde_json::Value::as_bool) == Some(true);
    let active = activity.get("active").and_then(serde_json::Value::as_bool) == Some(true);
    let started = activity.get("started").and_then(serde_json::Value::as_bool) == Some(true);
    let finished = activity.get("finished").and_then(serde_json::Value::as_bool) == Some(true);
    let claimed =
        activity.pointer("/dailySeed/claimed").and_then(serde_json::Value::as_bool) == Some(true);
    let status = if !known {
        crate::i18n::text(language, "状态未知", "Status unknown")
    } else if !active {
        crate::i18n::text(language, "暂未开放", "Not open")
    } else if finished {
        crate::i18n::text(language, "本轮已完成", "This round is complete")
    } else if started {
        crate::i18n::text(language, "酿造进行中", "Brewing in progress")
    } else if claimed {
        crate::i18n::text(language, "每日种子已领取", "Daily seed claimed")
    } else {
        crate::i18n::text(language, "可以开始", "Ready to start")
    };
    let status_color = if known && active { cx.theme().success } else { cx.theme().group_box };
    let mut header = panel(cx)
        .child(
            h_flex().items_center().justify_between().child(
                div().v_flex().gap_1().child(div().text_lg().font_semibold().child(title)).child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(
                            language,
                            "选择青梅原料进行多轮酿造，可继续加工或结算售价。",
                            "Choose green plums for several brewing rounds, then continue processing or settle the sale value.",
                        )),
                ),
            ),
        )
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .rounded(cx.theme().radius)
                        .bg(status_color)
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(if known && active {
                            cx.theme().success_foreground
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(status),
                )
                .when_some(text(activity, "activityId"), |row, id| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(if language == "en-US" {
                                format!("Event {id}")
                            } else {
                                format!("活动 {id}")
                            }),
                    )
                })
                .when_some(text(activity, "typeCode"), |row, code| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(if language == "en-US" {
                                format!("Type {code}")
                            } else {
                                format!("类型 {code}")
                            }),
                    )
                }),
        );
    if let Some(end_time) = number(activity.get("endTime")).filter(|time| *time > 0) {
        use chrono::TimeZone as _;
        if let Some(time) = chrono::Local.timestamp_millis_opt(end_time).single() {
            header = header.child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                if language == "en-US" {
                    format!("Ends {}", time.format("%m-%d %H:%M"))
                } else {
                    format!("活动截止 {}", time.format("%m-%d %H:%M"))
                },
            ));
        }
    }
    header
}

pub(super) fn seed_balance(
    desktop: &FarmDesktop,
    activity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = &desktop.preferences.language;
    let active = activity.get("known").and_then(serde_json::Value::as_bool) == Some(true)
        && activity.get("active").and_then(serde_json::Value::as_bool) == Some(true);
    let claimed =
        activity.pointer("/dailySeed/claimed").and_then(serde_json::Value::as_bool) == Some(true);
    let balance =
        if activity.get("balanceKnown").and_then(serde_json::Value::as_bool) == Some(false) {
            "--".into()
        } else {
            activity.get("balance").map(value_to_action_string).unwrap_or_else(|| "0".into())
        };
    panel(cx).child(
        h_flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .size(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .text_2xl()
                    .child("🫒"),
            )
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .gap_1()
                    .child(div().text_sm().font_medium().child(if language == "en-US" {
                        format!("Green plum stock: {balance}")
                    } else {
                        format!("青梅库存：{balance}")
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::i18n::text(
                                language,
                                "每日领取青梅种子，再选择库存中的原料开始酿造。",
                                "Claim the daily seed, then select ingredients from your stock to brew.",
                            )),
                    ),
            )
            .child(
                Button::new("qingmei-claim-seed")
                    .label(if claimed {
                        crate::i18n::text(language, "每日种子已领取", "Daily seed claimed")
                    } else {
                        crate::i18n::text(language, "领取每日种子", "Claim daily seed")
                    })
                    .primary()
                    .loading(desktop.activity_action_is_pending("qingmei"))
                    .disabled(
                        desktop.loading || desktop.selected_account.is_none() || !active || claimed,
                    )
                    .on_click(
                        cx.listener(|this, _, _, cx| this.run_activity_action("qingmei", cx)),
                    ),
            ),
    )
}
