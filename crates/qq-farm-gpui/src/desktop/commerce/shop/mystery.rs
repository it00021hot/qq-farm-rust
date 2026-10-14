use super::super::super::*;
use super::card;

fn mystery_price_text(npc: &serde_json::Value) -> String {
    let price = npc.get("price").unwrap_or(&serde_json::Value::Null);
    let count = price.get("count").map(value_to_action_string).unwrap_or_else(|| "0".into());
    let currency = price
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "#{}",
                price.get("id").map(value_to_action_string).unwrap_or_else(|| "?".into())
            )
        });
    format!("{count} {currency}")
}

fn mystery_time_text(value: Option<&serde_json::Value>) -> String {
    let millis = value.and_then(serde_json::Value::as_i64).filter(|millis| *millis > 0);
    millis
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|time| time.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".into())
}

fn mystery_purchase_enabled(shop: &serde_json::Value, npc: &serde_json::Value) -> bool {
    if shop.get("active").and_then(serde_json::Value::as_bool) != Some(true)
        || npc.get("stock").and_then(serde_json::Value::as_i64).unwrap_or_default() <= 0
    {
        return false;
    }
    let price = npc.get("price").unwrap_or(&serde_json::Value::Null);
    if price.get("balance").is_none_or(serde_json::Value::is_null) {
        return true;
    }
    let balance = price
        .get("balance")
        .and_then(serde_json::Value::as_i64)
        .or_else(|| price.get("balance")?.as_str()?.trim().parse().ok());
    let count = price.get("count").and_then(serde_json::Value::as_i64);
    balance.zip(count).is_some_and(|(balance, count)| balance >= count)
}

impl FarmDesktop {
    pub(in crate::desktop) fn render_mystery_shop_panel(&self, cx: &mut Context<Self>) -> Div {
        let shop = &self.page_raw;
        let language = &self.preferences.language;
        let refresh = Button::new("mystery-shop-refresh")
            .label(crate::i18n::text(language, "刷新", "Refresh"))
            .ghost()
            .disabled(self.loading)
            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)));
        // Vue's mystery panel has no heading inside the mall tab (the "神秘商人"
        // tab itself labels it); keep the heading on the standalone page only.
        let header = if self.page == Page::MysteryShop {
            h_flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().text_lg().font_medium().child(crate::i18n::text(
                    language,
                    "神秘商人",
                    "Mystery Merchant",
                )))
                .child(refresh)
        } else {
            h_flex().justify_end().child(refresh)
        };

        let Some(npc) = shop
            .get("npc")
            .filter(|npc| !npc.is_null())
            .filter(|_| shop.get("active").and_then(serde_json::Value::as_bool) == Some(true))
        else {
            let empty_label = if self.selected_account.is_none() {
                crate::i18n::text(language, "请先选择农场账号", "Select a farm account")
            } else {
                crate::i18n::text(language, "当前没有活跃的神秘商人", "No active mystery merchant")
            };
            let empty = div()
                .id("mystery-shop-empty")
                .aria_label(empty_label.to_string())
                .py_12()
                .text_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(empty_label)
                .test_support();
            return div().v_flex().gap_4().child(header).child(empty);
        };

        let reward = npc.get("reward").unwrap_or(&serde_json::Value::Null);
        let reward_name = reward
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(crate::i18n::text(language, "神秘商品", "Mystery item"))
            .to_string();
        let reward_count =
            reward.get("count").map(value_to_action_string).unwrap_or_else(|| "1".into());
        let reward_image =
            render_record_asset_preview_sized(reward, 72., cx).unwrap_or_else(|| {
                div()
                    .size(px(72.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_3xl()
                    .child("🛒")
                    .into_any_element()
            });
        let stock = npc.get("stock").map(value_to_action_string).unwrap_or_else(|| "0".into());
        let price = npc.get("price").unwrap_or(&serde_json::Value::Null);
        let balance = price.get("balance").filter(|value| !value.is_null());
        let discount =
            npc.get("discountPercent").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let original_price =
            npc.get("originalPrice").map(value_to_action_string).unwrap_or_else(|| "0".into());
        let enabled = mystery_purchase_enabled(shop, npc) && !self.loading;
        let goods = npc.clone();
        let mut card = div()
            .v_flex()
            .gap_3()
            .w_full()
            .max_w(px(480.))
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .size(px(80.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .child(reward_image),
                    )
                    .child(
                        div().v_flex().gap_1().child(div().font_medium().child(reward_name)).child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("×{reward_count}")),
                        ),
                    ),
            )
            .child(h_flex().child(card::status_chip(
                format!("{} {stock}", crate::i18n::text(language, "库存", "Stock")),
                cx.theme().primary,
                cx,
            )))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(crate::i18n::text(language, "现价：", "Price: "))
                    .child(
                        div()
                            .font_medium()
                            .text_color(cx.theme().success)
                            .child(mystery_price_text(npc)),
                    )
                    .when_some(balance, |row, balance| {
                        row.child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                            format!(
                                "{} {}",
                                crate::i18n::text(language, "余额", "Balance"),
                                value_to_action_string(balance)
                            ),
                        ))
                    }),
            );
        if discount > 0 {
            let discount_label = if language == "en-US" {
                format!("{discount}% off · Original ")
            } else {
                format!("{discount}% 折扣 · 原价 ")
            };
            let original_label =
                format!("{} {original_price}", crate::i18n::text(language, "原价", "Original"));
            let original_price = div()
                .id("mystery-original-price")
                .aria_label(original_label)
                .line_through()
                .child(original_price);
            #[cfg(test)]
            let original_price = original_price.test_support();
            card = card.child(
                h_flex()
                    .items_center()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(discount_label)
                    .child(original_price),
            );
        }
        card = card
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!(
                "{}{}",
                crate::i18n::text(language, "活动开始：", "Starts: "),
                mystery_time_text(shop.get("activeTime"))
            )))
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!(
                "{}{}",
                crate::i18n::text(language, "活动结束：", "Ends: "),
                mystery_time_text(shop.get("expireTime"))
            )))
            .child(
                Button::new("mystery-shop-buy")
                    .label(crate::i18n::text(language, "购买", "Buy"))
                    .primary()
                    .disabled(!enabled)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_purchase_confirmation(goods.clone(), window, cx);
                    })),
            );

        div().v_flex().gap_4().child(header).child(card)
    }
}

#[cfg(test)]
mod tests {
    use super::{mystery_price_text, mystery_purchase_enabled, mystery_time_text};

    #[test]
    fn mystery_purchase_gate_matches_stock_and_known_balance_rules() {
        let shop = serde_json::json!({"active": true});
        let affordable = serde_json::json!({
            "stock": 1,
            "price": {"count": 8, "name": "金币", "balance": 8}
        });
        assert!(mystery_purchase_enabled(&shop, &affordable));
        let insufficient = serde_json::json!({
            "stock": 1,
            "price": {"count": 9, "name": "金币", "balance": 8}
        });
        assert!(!mystery_purchase_enabled(&shop, &insufficient));
        let unknown_balance =
            serde_json::json!({"stock": 1, "price": {"count": 9, "balance": null}});
        assert!(mystery_purchase_enabled(&shop, &unknown_balance));
        let sold_out = serde_json::json!({"stock": 0, "price": {"count": 1, "balance": 9}});
        assert!(!mystery_purchase_enabled(&shop, &sold_out));
        assert!(!mystery_purchase_enabled(&serde_json::json!({"active": false}), &affordable));
    }

    #[test]
    fn mystery_shop_displays_currency_fallback_and_vue_timestamp_shape() {
        let npc = serde_json::json!({"price": {"id": 88, "count": 4}});
        assert_eq!(mystery_price_text(&npc), "4 #88");
        assert_eq!(mystery_time_text(None), "-");
        assert_eq!(mystery_time_text(Some(&serde_json::json!(0))), "-");
        assert!(
            mystery_time_text(Some(&serde_json::json!(1_790_000_000_000_i64))).starts_with("2026-")
        );
    }
}
