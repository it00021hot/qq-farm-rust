use super::super::*;
use gpui_kit::component::input::NumberInput;
use gpui_kit::component::WindowExt;

impl FarmDesktop {
    pub(in crate::desktop) fn open_purchase_confirmation(
        &mut self,
        goods: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.commerce.purchase_pending {
            return;
        }
        self.commerce.purchase_count.update(cx, |state, cx| state.set_value("1", window, cx));
        let mystery_goods = self.page == Page::MysteryShop
            || (self.page == Page::Mall && self.commerce.mall_slot_type == -1);
        let language = self.preferences.language.clone();
        let english = language == "en-US";
        let name = goods
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(crate::i18n::text(&language, "所选商品", "Selected item"))
            .to_string();
        let title = crate::i18n::text(&language, "确认购买", "Confirm purchase");
        let mystery_summary = if english {
            format!(
                "Spend {} {} to buy {} ×{}?",
                goods
                    .pointer("/price/count")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or_default(),
                goods
                    .pointer("/price/name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("currency"),
                goods.pointer("/reward/name").and_then(serde_json::Value::as_str).unwrap_or(&name),
                goods.pointer("/reward/count").and_then(serde_json::Value::as_i64).unwrap_or(1),
            )
        } else {
            format!(
                "确认花费 {} {} 购买 {} ×{}？",
                goods
                    .pointer("/price/count")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or_default(),
                goods.pointer("/price/name").and_then(serde_json::Value::as_str).unwrap_or("货币"),
                goods.pointer("/reward/name").and_then(serde_json::Value::as_str).unwrap_or(&name),
                goods.pointer("/reward/count").and_then(serde_json::Value::as_i64).unwrap_or(1),
            )
        };
        self.commerce.pending_purchase = Some(goods);
        self.commerce.purchase_dialog_open = true;
        let mall_goods = self.commerce.pending_purchase.clone().unwrap_or_default();
        let purchase_count = self.commerce.purchase_count.clone();
        let muted_foreground = cx.theme().muted_foreground;
        let desktop = cx.entity();
        let state_entity = desktop.clone();
        let close_entity = desktop.clone();
        let confirm_entity = desktop;
        window.open_dialog(cx, move |dialog, _, app| {
            let purchase_pending = state_entity.read(app).commerce.purchase_pending;
            let confirm_entity = confirm_entity.clone();
            let close_entity = close_entity.clone();
            let mall_price = if !mystery_goods {
                Some(mall_price_summary(&mall_goods, &language, app))
            } else {
                None
            };
            let content = if mystery_goods {
                let summary = div()
                    .id("purchase-mystery-summary")
                    .aria_label(mystery_summary.clone())
                    .text_sm()
                    .child(mystery_summary.clone());
                #[cfg(test)]
                let summary = summary.test_support();
                summary.into_any_element()
            } else {
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(crate::i18n::text(&language, "数量", "Quantity"))
                    .child(NumberInput::new(&purchase_count).w(px(148.)))
                    .into_any_element()
            };
            dialog
                .title(title)
                .w(px(420.))
                .child(
                    div()
                        .v_flex()
                        .gap_4()
                        // The mystery copy names the reward inside its summary
                        // sentence, so it keeps the heading out (as in Vue).
                        .when(!mystery_goods, |body| {
                            body.child(div().font_medium().child(name.clone()))
                        })
                        .when_some(mall_price, |body, summary| body.child(summary))
                        .child(content)
                        .when(mystery_goods, |body| {
                            body.child(div().text_xs().text_color(muted_foreground).child(
                                crate::i18n::text(
                                    &language,
                                    "本次限购一件。",
                                    "This purchase is limited to one item.",
                                ),
                            ))
                        })
                        .when(!mystery_goods, |body| {
                            body.child(div().text_xs().text_color(muted_foreground).child(
                                crate::i18n::text(
                                    &language,
                                    "服务端会再次检查余额和限购。",
                                    "The server will check your balance and purchase limit again.",
                                ),
                            ))
                        }),
                )
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("purchase-cancel")
                                .label(crate::i18n::text(&language, "取消", "Cancel"))
                                .ghost()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("purchase-confirm")
                                .label(crate::i18n::text(&language, "购买", "Buy"))
                                .primary()
                                .loading(purchase_pending)
                                .disabled(purchase_pending)
                                .on_click(move |_, _window, cx| {
                                    confirm_entity.update(cx, |this, cx| this.confirm_purchase(cx));
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    close_entity.update(cx, |this, cx| {
                        this.commerce.pending_purchase = None;
                        this.commerce.purchase_dialog_open = false;
                        cx.notify();
                    });
                })
        });
    }
}

fn mall_price_summary(goods: &serde_json::Value, language: &str, cx: &App) -> Div {
    let original =
        goods.get("originalPrice").filter(|value| !value.is_null()).map(value_to_action_string);
    let is_free = goods.get("isFree").and_then(serde_json::Value::as_bool) == Some(true);
    let current = if is_free {
        crate::i18n::text(language, "免费", "Free").to_string()
    } else {
        let price = goods.get("price").unwrap_or(&serde_json::Value::Null);
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
    };
    let mut summary = h_flex().flex_wrap().items_center().gap_2();
    if let Some(original) = original {
        if !is_free {
            let original_label =
                format!("{} {original}", crate::i18n::text(language, "原价", "Original"));
            let original_price = div()
                .id("purchase-original-price")
                .aria_label(original_label)
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .line_through()
                .child(original);
            #[cfg(test)]
            let original_price = original_price.test_support();
            summary = summary.child(original_price);
        }
    }
    summary = summary.child(div().text_sm().text_color(cx.theme().success).child(current));
    if let Some(discount) = goods
        .get("discountText")
        .and_then(serde_json::Value::as_str)
        .filter(|discount| !discount.is_empty())
    {
        summary = summary
            .child(div().text_xs().text_color(cx.theme().danger).child(discount.to_string()));
    }
    summary
}
