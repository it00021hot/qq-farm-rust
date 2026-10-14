use super::super::super::*;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::StatefulInteractiveElement;
use gpui_kit::TestSupportExt;

use super::helpers::{can_sell, can_use, category, category_label, number, text};

impl FarmDesktop {
    pub(super) fn render_personal_bag_card(
        &self,
        item: &serde_json::Value,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = &self.preferences.language;
        let id = number(item.get("id"));
        let count = number(item.get("count"));
        let uid = number(item.get("uid"));
        let category_key = category(item);
        let category_name = category_label(category_key, language);
        let name = text(item.get("name"), &format!("#{id}")).to_string();
        let can_sell = can_sell(item);
        let can_use = can_use(item);
        let selected = self.farm.bag.selected_ids.contains(&id);
        let mut card = div()
            .id(format!("bag-item-{id}-{index}"))
            .v_flex()
            .w(px(170.))
            .min_w(px(140.))
            .min_h(px(210.))
            .gap_2()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(if selected { cx.theme().primary } else { cx.theme().border })
            .bg(cx.theme().group_box)
            .p_3()
            .when(self.farm.bag.batch_mode && can_sell, |card| {
                card.on_click(cx.listener(move |this, _, _, cx| {
                    let selected = !this.farm.bag.selected_ids.contains(&id);
                    this.set_bag_selection(id, selected, cx);
                }))
            })
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("#{id}")),
                    )
                    .child(if self.farm.bag.batch_mode && can_sell {
                        Checkbox::new(format!("bag-select-{id}-{index}"))
                            .checked(selected)
                            .accessibility_label(crate::i18n::text(
                                language,
                                &format!("选择 {name} 出售"),
                                &format!("Select {name} to sell"),
                            ))
                            .on_change(cx.listener(move |this, checked, _, cx| {
                                cx.stop_propagation();
                                this.set_bag_selection(id, *checked, cx)
                            }))
                            .into_any_element()
                    } else {
                        bag_category_badge(category_name, cx).into_any_element()
                    }),
            );

        let preview = render_record_asset_preview_sized(item, 48., cx);
        card = card.child(
            div()
                .h(px(56.))
                .flex()
                .items_center()
                .justify_center()
                .when_some(preview, |row, preview| row.child(preview))
                .when(
                    item.get("image").and_then(serde_json::Value::as_str).is_none_or(str::is_empty),
                    |row| {
                        row.child(
                            div().text_xl().text_color(cx.theme().muted_foreground).child("📦"),
                        )
                    },
                ),
        );
        let tooltip_name = name.clone();
        let name_label = div()
            .id(format!("bag-item-name-{id}-{index}"))
            .aria_label(name.clone())
            .min_w_0()
            .truncate()
            .text_center()
            .font_medium()
            .child(name.clone())
            .tooltip(move |window, cx| Tooltip::new(tooltip_name.clone()).build(window, cx));
        #[cfg(test)]
        let name_label = name_label.test_support();
        let name_label = name_label.into_any_element();
        card = card.child(name_label).child(render_item_details(item, category_key, language, cx));

        let mutant_effects = item
            .get("mutantEffects")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        if !mutant_effects.is_empty() {
            card = card.child(
                div().text_center().text_xs().text_color(cx.theme().warning).child(
                    crate::i18n::text(
                        language,
                        &format!("变异：{}", mutant_effects.join("+")),
                        &format!("Mutations: {}", mutant_effects.join("+")),
                    )
                    .to_string(),
                ),
            );
        }

        let hours = text(item.get("hoursText"), "");
        let quantity = if hours.is_empty() { format!("x{count}") } else { hours.to_string() };
        card = card.child(div().text_center().font_medium().child(quantity));

        if !self.farm.bag.batch_mode || can_use {
            let mut actions = h_flex().items_center().justify_center().gap_2();
            if !self.farm.bag.batch_mode && can_sell {
                actions = actions.child(
                    Button::new(("bag-sell", index))
                        .label(crate::i18n::text(language, "出售", "Sell"))
                        .danger()
                        .loading(self.farm.bag.selling_ids.contains(&id))
                        .disabled(
                            self.loading
                                || self.farm.bag.selling_ids.contains(&id)
                                || self.farm.bag.batch_selling,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_bag_action(id, uid, true, cx)
                        })),
                );
            }
            if can_use {
                actions = actions.child(
                    Button::new(("bag-use", index))
                        .label(crate::i18n::text(language, "使用", "Use"))
                        .primary()
                        .loading(self.farm.bag.using_ids.contains(&id))
                        .disabled(self.loading || self.farm.bag.using_ids.contains(&id))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_bag_action(id, uid, false, cx)
                        })),
                );
            }
            card = card.child(actions);
        }

        card.test_support()
    }
}

fn render_item_details(item: &serde_json::Value, category: &str, language: &str, cx: &App) -> Div {
    let level = number(item.get("level"));
    let price = number(item.get("price"));
    let price_id = number(item.get("priceId"));
    let price_unit = text(item.get("priceUnit"), "");
    let mut details = h_flex().flex_wrap().items_center().justify_center().gap_2().text_xs();
    if level > 0 {
        details = details
            .child(div().text_color(cx.theme().muted_foreground).child(format!("Lv.{level}")));
    }
    if price > 0 {
        let color = match price_id {
            qq_farm_core::constants::GOLD_BEAN_ITEM_ID => cx.theme().warning,
            qq_farm_core::constants::COUPON_ITEM_ID => cx.theme().info,
            _ => cx.theme().muted_foreground,
        };
        details = details.child(div().text_color(color).child(format!("{price}{price_unit}")));
    }
    if category == "fruit" {
        let sellable = can_sell(item);
        let sell_status = text(item.get("sellStatus"), "");
        let status = if sellable {
            crate::i18n::text(language, "可出售", "Sellable")
        } else if sell_status == "conditional" {
            crate::i18n::text(language, "条件出售", "Conditional sale")
        } else {
            crate::i18n::text(language, "不可出售", "Not sellable")
        };
        let color = if sellable { cx.theme().success } else { cx.theme().muted_foreground };
        let condition = item
            .get("sellCondition")
            .and_then(serde_json::Value::as_str)
            .filter(|condition| !condition.is_empty())
            .map(str::to_string);
        let status_label = match condition {
            Some(condition) => div()
                .id(format!("bag-sell-condition-{}", number(item.get("id"))))
                .text_color(color)
                .child(status)
                .tooltip(move |window, cx| Tooltip::new(condition.clone()).build(window, cx))
                .into_any_element(),
            None => div().text_color(color).child(status).into_any_element(),
        };
        details = details.child(status_label);
    }
    details
}

fn bag_category_badge(label: &str, cx: &App) -> Div {
    div()
        .rounded(cx.theme().radius)
        .bg(cx.theme().background)
        .px_2()
        .py_1()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(label.to_string())
}
