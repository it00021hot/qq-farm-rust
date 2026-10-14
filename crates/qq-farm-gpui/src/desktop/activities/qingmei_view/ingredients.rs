use super::super::super::*;

use super::shared::{image, number, panel, text};

pub(super) fn render(
    desktop: &FarmDesktop,
    activity: &serde_json::Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = &desktop.preferences.language;
    let ingredients = activity.get("ingredients").and_then(serde_json::Value::as_array);
    let available = ingredients
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let uid = item.get("uid").map(value_to_action_string)?;
            let stock = number(item.get("count")).unwrap_or_default();
            (!uid.is_empty() && stock > 0).then_some((uid, stock, item.clone()))
        })
        .collect::<Vec<_>>();
    let selected = &desktop.activities.qingmei_selected_ingredients;
    let all_selected =
        !available.is_empty() && available.iter().all(|(uid, _, _)| selected.contains_key(uid));
    let selected_total = available
        .iter()
        .filter_map(|(uid, stock, _)| selected.get(uid).map(|count| (*count).min(*stock)))
        .sum::<i64>();
    let selected_count = available.iter().filter(|(uid, _, _)| selected.contains_key(uid)).count();
    let qingmei_busy = desktop.loading
        || ["qingmei", "qingmei-start", "qingmei-continue", "qingmei-settle"]
            .iter()
            .any(|action| desktop.activity_action_is_pending(action));

    let mut list = div().v_flex().gap_2().max_h(px(460.)).overflow_y_scrollbar();
    if available.is_empty() {
        list = list.child(
            div().py_8().text_center().text_sm().text_color(cx.theme().muted_foreground).child(
                crate::i18n::text(
                    language,
                    "背包中没有可用的青梅原料",
                    "No green plum ingredients are available in the bag.",
                ),
            ),
        );
    } else {
        for (index, (uid, stock, item)) in available.iter().enumerate() {
            let selected_count = selected.get(uid).copied();
            let name = text(item, "name").unwrap_or_else(|| {
                crate::i18n::text(language, "青梅原料", "Green plum ingredient").into()
            });
            let mutant_types = item
                .get("mutantTypes")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .map(value_to_action_string)
                .collect::<Vec<_>>();
            let detail = if mutant_types.is_empty() {
                if language == "en-US" {
                    format!("Ingredient {uid} · In stock {stock}")
                } else {
                    format!("原料 {uid} · 持有 {stock}")
                }
            } else {
                if language == "en-US" {
                    format!(
                        "Ingredient {uid} · In stock {stock} · Mutations {}",
                        mutant_types.join("+")
                    )
                } else {
                    format!("原料 {uid} · 持有 {stock} · 变异 {}", mutant_types.join("+"))
                }
            };
            let action_uid = uid.clone();
            let stock_value = *stock;
            let count = selected_count.unwrap_or(*stock).min(*stock);
            let mut card = panel(cx).child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(image(item, 40., language, cx))
                    .child(
                        div()
                            .v_flex()
                            .min_w_0()
                            .flex_1()
                            .gap_1()
                            .child(div().truncate().text_sm().font_medium().child(name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(detail),
                            ),
                    )
                    .child(
                        Button::new(("qingmei-ingredient-toggle", index))
                            .label(if selected_count.is_some() {
                                crate::i18n::text(language, "已选择", "Selected")
                            } else {
                                crate::i18n::text(language, "选择原料", "Select ingredient")
                            })
                            .when(selected_count.is_some(), |button| button.primary())
                            .when(selected_count.is_none(), |button| button.ghost())
                            .disabled(qingmei_busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_qingmei_ingredient(action_uid.clone(), stock_value, cx)
                            })),
                    ),
            );
            if selected_count.is_some() {
                let down_uid = uid.clone();
                let up_uid = uid.clone();
                let max_uid = uid.clone();
                card = card.child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div().flex_1().text_xs().text_color(cx.theme().muted_foreground).child(
                                if language == "en-US" {
                                    format!("Brew quantity · Stock {stock}")
                                } else {
                                    format!("酿造数量 · 库存 {stock}")
                                },
                            ),
                        )
                        .child(
                            Button::new(("qingmei-count-down", index))
                                .label("−")
                                .ghost()
                                .disabled(qingmei_busy || count <= 1)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.adjust_qingmei_ingredient(
                                        down_uid.clone(),
                                        stock_value,
                                        -1,
                                        cx,
                                    )
                                })),
                        )
                        .child(
                            div().min_w(px(36.)).text_center().text_sm().child(count.to_string()),
                        )
                        .child(
                            Button::new(("qingmei-count-up", index))
                                .label("+")
                                .ghost()
                                .disabled(qingmei_busy || count >= *stock)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.adjust_qingmei_ingredient(
                                        up_uid.clone(),
                                        stock_value,
                                        1,
                                        cx,
                                    )
                                })),
                        )
                        .child(
                            Button::new(("qingmei-count-all", index))
                                .label(crate::i18n::text(language, "全部", "All"))
                                .ghost()
                                .disabled(qingmei_busy || count >= *stock)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_qingmei_ingredient_count(
                                        max_uid.clone(),
                                        stock_value,
                                        stock_value,
                                        cx,
                                    )
                                })),
                        ),
                );
            }
            list = list.child(card);
        }
    }

    panel(cx)
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().font_medium().child(crate::i18n::text(
                    language,
                    "选择酿造原料",
                    "Select brewing ingredients",
                )))
                .child(
                    Button::new("qingmei-select-all")
                        .label(if all_selected {
                            crate::i18n::text(language, "取消全选", "Deselect all")
                        } else {
                            crate::i18n::text(language, "全选原料", "Select all")
                        })
                        .ghost()
                        .disabled(qingmei_busy || available.is_empty())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if all_selected {
                                this.clear_qingmei_selection(cx);
                            } else {
                                this.set_qingmei_all(
                                    &available
                                        .iter()
                                        .map(|(uid, stock, _)| (uid.clone(), *stock))
                                        .collect::<Vec<_>>(),
                                    cx,
                                );
                            }
                        })),
                ),
        )
        .child(list)
        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
            if language == "en-US" {
                format!("{selected_count} ingredient types selected, {selected_total} total")
            } else {
                format!("已选 {selected_count} 种原料，共 {selected_total} 个")
            },
        ))
}
