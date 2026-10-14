use super::super::super::*;

use super::shared::{number, panel, text};

fn enabled(action: &serde_json::Value, fallback: bool) -> bool {
    action.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(fallback)
}

pub(super) fn render(
    desktop: &FarmDesktop,
    activity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = &desktop.preferences.language;
    let english = language == "en-US";
    let active = activity.get("known").and_then(serde_json::Value::as_bool) == Some(true)
        && activity.get("active").and_then(serde_json::Value::as_bool) == Some(true);
    let started = activity.get("started").and_then(serde_json::Value::as_bool) == Some(true);
    let finished = activity.get("finished").and_then(serde_json::Value::as_bool) == Some(true);
    let round = number(activity.get("currentRound")).unwrap_or_default();
    let max_rounds = number(activity.get("maxRounds")).unwrap_or(3).max(1);
    let quotes = activity.get("quoteTotals").and_then(serde_json::Value::as_array);
    let prices = activity.get("quotePrices").and_then(serde_json::Value::as_array);
    let base_price = activity
        .get("guaranteedPrice")
        .or_else(|| activity.get("basePrice"))
        .map(value_to_action_string)
        .filter(|price| !price.is_empty())
        .unwrap_or_else(|| "0".into());
    let available = activity.get("ingredients").and_then(serde_json::Value::as_array);
    let selected = &desktop.activities.qingmei_selected_ingredients;
    let selected_total = available
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let uid = item.get("uid").map(value_to_action_string)?;
            let stock = number(item.get("count")).unwrap_or_default();
            selected.get(&uid).map(|count| (*count).min(stock))
        })
        .sum::<i64>();
    let has_selected_ingredient = selected_total > 0;
    let start_enabled =
        enabled(activity.pointer("/actions/start").unwrap_or(&serde_json::Value::Null), !started);
    let continue_enabled = enabled(
        activity.pointer("/actions/continue").unwrap_or(&serde_json::Value::Null),
        started && !finished && round < max_rounds,
    );
    let quotes_present = quotes.is_some_and(|quotes| !quotes.is_empty());
    let settle_enabled = enabled(
        activity.pointer("/actions/settle").unwrap_or(&serde_json::Value::Null),
        quotes_present || finished,
    );
    let status = if !started {
        crate::i18n::text(language, "尚未开始", "Not started")
    } else if finished {
        crate::i18n::text(language, "酿造完成，可以结算", "Brewing complete; ready to settle")
    } else {
        crate::i18n::text(
            language,
            "继续收集报价或提前结算",
            "Continue collecting quotes or settle early",
        )
    };

    let mut card = panel(cx).child(
        h_flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::i18n::text(language, "酿造进度", "Brewing progress")),
                    )
                    .child(div().text_sm().font_medium().child(if started {
                        if english {
                            format!("Round {round}/{max_rounds}")
                        } else {
                            format!("第 {round}/{max_rounds} 轮")
                        }
                    } else {
                        status.into()
                    })),
            )
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(if english {
                format!("Guaranteed unit price {base_price}")
            } else {
                format!("保底单价 {base_price}")
            })),
    );

    if !started {
        card = card.child(super::ingredients::render(desktop, activity, cx));
        card = card.child(
            h_flex()
                .items_center()
                .justify_between()
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(if english {
                    format!("{} ingredients selected", selected_total)
                } else {
                    format!("本次选择 {} 个原料", selected_total)
                }))
                .child(
                    Button::new("qingmei-start-brew")
                        .label(crate::i18n::text(language, "开始酿造", "Start brewing"))
                        .primary()
                        .loading(desktop.activity_action_is_pending("qingmei-start"))
                        .disabled(
                            desktop.loading
                                || desktop.selected_account.is_none()
                                || !active
                                || !start_enabled
                                || !has_selected_ingredient,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_activity_action("qingmei-start", cx)
                        })),
                ),
        );
        return card;
    }

    let mut quote_grid = div().grid().grid_cols(3).gap_3();
    for (index, total) in quotes.into_iter().flatten().enumerate() {
        let price = prices
            .and_then(|prices| prices.get(index))
            .map(value_to_action_string)
            .filter(|price| !price.is_empty())
            .unwrap_or_else(|| "0".into());
        quote_grid = quote_grid.child(
            div()
                .v_flex()
                .items_center()
                .gap_1()
                .rounded(cx.theme().radius)
                .bg(cx.theme().background)
                .p_3()
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(if english {
                    format!("Round {} quote", index + 1)
                } else {
                    format!("第 {} 轮报价", index + 1)
                }))
                .child(div().text_lg().font_semibold().child(value_to_action_string(total)))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(if english {
                    format!("Unit price {price}")
                } else {
                    format!("单价 {price}")
                })),
        );
    }
    for round_index in quotes.map_or(0, Vec::len)..(max_rounds as usize) {
        quote_grid = quote_grid.child(
            div()
                .v_flex()
                .items_center()
                .gap_1()
                .rounded(cx.theme().radius)
                .bg(cx.theme().background)
                .p_3()
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(if english {
                    format!("Round {} quote", round_index + 1)
                } else {
                    format!("第 {} 轮报价", round_index + 1)
                }))
                .child(
                    div()
                        .text_lg()
                        .font_semibold()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "等待报价", "Waiting for quote")),
                ),
        );
    }
    if quotes_present || max_rounds > 0 {
        card = card.child(quote_grid);
    }
    if let Some(message) = text(activity, "statusMessage") {
        card = card.child(div().text_sm().text_color(cx.theme().muted_foreground).child(message));
    }
    card.child(
        h_flex()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                Button::new("qingmei-continue-brew")
                    .label(crate::i18n::text(language, "继续酿造", "Continue brewing"))
                    .primary()
                    .loading(desktop.activity_action_is_pending("qingmei-continue"))
                    .disabled(
                        desktop.loading
                            || desktop.selected_account.is_none()
                            || !active
                            || !continue_enabled,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.run_activity_action("qingmei-continue", cx)
                    })),
            )
            .child(
                Button::new("qingmei-settle-brew")
                    .label(crate::i18n::text(language, "结算酿造", "Settle brewing"))
                    .ghost()
                    .loading(desktop.activity_action_is_pending("qingmei-settle"))
                    .disabled(
                        desktop.loading
                            || desktop.selected_account.is_none()
                            || !active
                            || !settle_enabled,
                    )
                    .on_click(
                        cx.listener(|this, _, _, cx| {
                            this.run_activity_action("qingmei-settle", cx)
                        }),
                    ),
            ),
    )
}
