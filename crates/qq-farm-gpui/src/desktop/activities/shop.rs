use super::super::*;
use serde_json::Value;

mod categories;

use categories::{goods_category, is_truthy, js_string, shop_categories};

fn text(value: Option<&Value>, fallback: &str) -> String {
    value.map(js_string).filter(|value| !value.is_empty()).unwrap_or_else(|| fallback.to_string())
}

fn goods_name(good: &Value, language: &str) -> String {
    good.get("name")
        .filter(|value| is_truthy(value))
        .or_else(|| good.pointer("/item/name").filter(|value| is_truthy(value)))
        .or_else(|| good.get("id"))
        .map(js_string)
        .unwrap_or_else(|| crate::i18n::text(language, "未命名商品", "Unnamed item").into())
}

pub(in crate::desktop::activities) fn balance(shop: &Value) -> String {
    if shop.get("balanceKnown").and_then(Value::as_bool) == Some(false) {
        return "--".into();
    }

    let first_currency_balance = shop
        .get("currencies")
        .and_then(Value::as_array)
        .and_then(|currencies| currencies.first())
        .and_then(|currency| currency.get("balance"));
    let currency = shop.get("currency");
    let value = shop
        .get("balance")
        .filter(|value| !value.is_null())
        .or_else(|| first_currency_balance.filter(|value| !value.is_null()))
        .or_else(|| currency.and_then(|currency| currency.get("balance")))
        .filter(|value| !value.is_null())
        .or_else(|| currency.and_then(|currency| currency.get("count")));

    match value {
        None => "--".into(),
        Some(Value::String(value)) if value.is_empty() => "--".into(),
        Some(value) => js_string(value),
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_activity_shop_panel(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        let shop = self.page_raw.get("shop").unwrap_or(&Value::Null);
        let goods = shop.get("goods").and_then(Value::as_array).cloned().unwrap_or_default();
        let categories = shop_categories(shop);
        let category = &self.activities.star_sand_category;
        let query = self.search.read(cx).value().trim().to_lowercase();
        let visible_goods = goods
            .iter()
            .enumerate()
            .filter(|(_, good)| {
                let good_category = goods_category(good);
                let category_matches = category == "__all__" || good_category == *category;
                let searchable = [
                    text(good.get("name"), ""),
                    text(good.pointer("/item/name"), ""),
                    text(good.get("id"), ""),
                    good_category,
                ]
                .join(" ")
                .to_lowercase();
                category_matches && (query.is_empty() || searchable.contains(&query))
            })
            .map(|(index, good)| (index, good.clone()))
            .collect::<Vec<_>>();
        let exchange_enabled = activity_action_enabled(&self.page_raw, "exchange") != Some(false);
        let title = shop
            .get("title")
            .filter(|value| is_truthy(value))
            .or_else(|| shop.get("name").filter(|value| is_truthy(value)))
            .map(js_string)
            .unwrap_or_else(|| {
                crate::i18n::text(language, "星砂商店", "Star Sand Shop").to_string()
            });

        let mut category_buttons = vec![Button::new("star-sand-category-all")
            .label(crate::i18n::text(language, "全部", "All"))
            .when(category == "__all__", |button| button.primary())
            .when(category != "__all__", |button| button.ghost())
            .on_click(cx.listener(|this, _, _, cx| {
                this.activities.star_sand_category = "__all__".into();
                cx.notify();
            }))
            .into_any_element()];
        category_buttons.extend(categories.iter().enumerate().map(|(index, category_entry)| {
            let category_id = category_entry.id.clone();
            let category_name = category_entry.name.clone();
            Button::new(("star-sand-category", index))
                .label(category_name)
                .when(category == &category_id, |button| button.primary())
                .when(category != &category_id, |button| button.ghost())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.activities.star_sand_category = category_id.clone();
                    cx.notify();
                }))
                .into_any_element()
        }));

        let panel = div()
            .v_flex()
            .gap_4()
            .child(div().v_flex().gap_1().child(div().font_medium().text_lg().child(title)).child(
                div().text_sm().text_color(cx.theme().muted_foreground).child(text(
                    shop.get("description"),
                    crate::i18n::text(
                        language,
                        "使用星砂兑换心仪奖励",
                        "Exchange rewards with star sand",
                    ),
                )),
            ))
            .child(h_flex().flex_wrap().gap_2().children(category_buttons));

        if visible_goods.is_empty() {
            return panel.child(
                div().py_12().text_sm().text_color(cx.theme().muted_foreground).child(
                    crate::i18n::text(
                        language,
                        "当前分类没有可显示的商品",
                        "No items are available in this category.",
                    ),
                ),
            );
        }

        let mut cards = Vec::with_capacity(visible_goods.len());
        for (index, good) in &visible_goods {
            let id = text(good.get("id"), "");
            let name = goods_name(good, language);
            let owned = good.get("owned").and_then(Value::as_bool) == Some(true);
            let sold_out = good.get("soldOut").and_then(Value::as_bool) == Some(true);
            let exchangeable = good.get("exchangeable").and_then(Value::as_bool) == Some(true);
            let unavailable = owned || sold_out || !exchangeable || !exchange_enabled;
            let item_image = good
                .pointer("/item/image")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(|url| render_asset_image(url, 56., cx));
            let cost_image = good
                .pointer("/cost/image")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(|url| render_asset_image(url, 16., cx));
            let cost_count = text(good.pointer("/cost/count"), "--");
            let name_for_action = name.clone();
            let fallback_goods = good.clone();
            let button_label = if self.commerce.purchase_pending {
                crate::i18n::text(language, "正在处理兑换", "Exchanging")
            } else if owned || sold_out {
                crate::i18n::text(language, "已兑换", "Redeemed")
            } else if !exchangeable || !exchange_enabled {
                if !exchangeable {
                    crate::i18n::text(language, "不可兑换", "Not exchangeable")
                } else {
                    crate::i18n::text(language, "暂不可兑换", "Exchange unavailable")
                }
            } else {
                crate::i18n::text(language, "兑换", "Exchange")
            };

            let mut card = div()
                .v_flex()
                .gap_2()
                .w(px(196.))
                .min_w_0()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .p_3()
                .when(unavailable, |card| card.opacity(0.65))
                .child(h_flex().justify_end().h(px(18.)).when(owned || sold_out, |row| {
                    row.child(
                        div()
                            .rounded_full()
                            .bg(cx.theme().success.opacity(0.16))
                            .px_2()
                            .text_xs()
                            .text_color(cx.theme().success)
                            .child(crate::i18n::text(language, "已兑换", "Redeemed")),
                    )
                }));
            let preview = div()
                .h(px(76.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(cx.theme().radius)
                .bg(cx.theme().group_box);
            card = card.child(if let Some(image) = item_image {
                preview.child(image)
            } else {
                preview.text_2xl().child("🎁")
            });
            card = card
                .child(div().truncate().font_medium().text_sm().child(name.clone()))
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .text_color(cx.theme().warning)
                        .when_some(cost_image, |row, image| row.child(image))
                        .child(cost_count),
                )
                .child(
                    Button::new(("star-sand-exchange", *index))
                        .label(button_label)
                        .primary()
                        .disabled(self.loading || self.commerce.purchase_pending || unavailable)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_star_sand_confirmation(
                                id.clone(),
                                name_for_action.clone(),
                                fallback_goods.clone(),
                                window,
                                cx,
                            );
                        })),
                );
            cards.push(card.into_any_element());
        }

        panel.child(h_flex().flex_wrap().gap_3().children(cards))
    }
}

#[cfg(test)]
mod tests {
    use super::{balance, goods_name};
    use serde_json::json;

    #[test]
    fn balance_uses_vue_nullish_fallback_order() {
        assert_eq!(balance(&json!({ "balance": 0, "currencies": [{"balance": 8}] })), "0");
        assert_eq!(balance(&json!({ "balance": null, "currencies": [{"balance": 8}] })), "8");
        assert_eq!(
            balance(&json!({ "currencies": [], "currency": {"balance": 5, "count": 4} })),
            "5"
        );
        assert_eq!(balance(&json!({ "currency": {"balance": null, "count": 4} })), "4");
        assert_eq!(balance(&json!({ "balanceKnown": false, "balance": 8 })), "--");
        assert_eq!(balance(&json!({ "balance": "" })), "--");
    }

    #[test]
    fn goods_name_uses_vue_truthy_fallback_order() {
        assert_eq!(
            goods_name(&json!({ "name": "", "item": {"name": "Item label"}, "id": 7 }), "zh-CN"),
            "Item label"
        );
        assert_eq!(
            goods_name(&json!({ "name": null, "item": {"name": ""}, "id": 7 }), "zh-CN"),
            "7"
        );
    }
}
