use super::super::super::super::*;
use gpui_kit::component::input::NumberInput;
use serde_json::Value;

use super::super::common::{is_true, items_line, picture, section_card, text};

fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn remaining_suffix(good: &Value, english: bool) -> String {
    good.get("remaining")
        .filter(|value| js_truthy(value))
        .map(|remaining| {
            if english {
                format!(" · {} remaining", text(Some(remaining), "0"))
            } else {
                format!(" · 剩 {}", text(Some(remaining), "0"))
            }
        })
        .unwrap_or_default()
}

pub(super) fn render(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    let language = &this.preferences.language;
    let english = language == "en-US";
    let goods = snapshot.get("shop").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut panel = section_card(this, "拾物小铺", "shop", cx);
    if !this.activities.pet_diary_expanded_sections.contains("shop") {
        return panel;
    }
    if goods.is_empty() {
        panel = panel.child(
            div().text_sm().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                language,
                "小铺目录为空",
                "The shop catalog is empty.",
            )),
        );
    } else {
        for (index, good) in goods.iter().enumerate() {
            let id = text(good.get("id"), "");
            let name =
                text(good.get("name"), crate::i18n::text(language, "未命名商品", "Unnamed item"));
            let exchangeable = is_true(good.get("exchangeable"));
            let remaining = remaining_suffix(good, english);
            panel = panel.child(
                h_flex()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_3()
                    .child(div().w(px(36.)).text_xs().child(format!("#{id}")))
                    .when_some(
                        good.get("image").and_then(Value::as_str).filter(|image| !image.is_empty()),
                        |row, image| row.child(picture(image, 22., cx)),
                    )
                    .child(div().min_w(px(120.)).font_medium().child(name))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::i18n::text(language, "消耗", "Cost")),
                    )
                    .child(items_line(good.get("costs"), language, cx))
                    .child(if english {
                        format!(
                            "Purchased {}/{}{}",
                            text(good.get("purchased"), "0"),
                            {
                                let limit = text(good.get("limit"), "0");
                                if limit == "0" || limit.is_empty() {
                                    "Unlimited".to_string()
                                } else {
                                    limit
                                }
                            },
                            remaining
                        )
                    } else {
                        format!(
                            "限购 {}/{}{}",
                            text(good.get("purchased"), "0"),
                            {
                                let limit = text(good.get("limit"), "0");
                                if limit == "0" || limit.is_empty() {
                                    "不限".to_string()
                                } else {
                                    limit
                                }
                            },
                            remaining
                        )
                    })
                    .child(
                        Button::new(("pet-diary-exchange-good", index))
                            .label(crate::i18n::text(language, "兑换", "Exchange"))
                            .ghost()
                            .loading(this.pet_diary_action_is_pending("exchange"))
                            .disabled(this.loading || !exchangeable)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_pet_diary_action(
                                    "exchange",
                                    serde_json::json!({"goodsId": id, "count": 1}),
                                    cx,
                                )
                            })),
                    ),
            );
        }
    }

    let goods_id = this.activities.pet_diary_goods_id.read(cx).value().trim().to_string();
    let exchange_count = this
        .activities
        .pet_diary_exchange_count
        .read(cx)
        .value()
        .trim()
        .parse::<i64>()
        .unwrap_or(1)
        .max(1);
    panel.child(
        h_flex()
            .items_center()
            .flex_wrap()
            .gap_2()
            .child(
                Input::new(&this.activities.pet_diary_goods_id)
                    .id("pet-diary-goods-id")
                    .w(px(125.)),
            )
            .child(
                NumberInput::new(&this.activities.pet_diary_exchange_count)
                    .w(px(112.))
                    .disabled(this.loading),
            )
            .child(
                Button::new("pet-diary-exchange-by-id")
                    .label(crate::i18n::text(language, "按编号兑换", "Exchange by ID"))
                    .ghost()
                    .loading(this.pet_diary_action_is_pending("exchange"))
                    .disabled(this.loading)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if goods_id.is_empty() {
                            this.error = Some(
                                crate::i18n::text(
                                    &this.preferences.language,
                                    "请输入商品编号",
                                    "Enter an item ID.",
                                )
                                .into(),
                            );
                            cx.notify();
                            return;
                        }
                        this.run_pet_diary_action(
                            "exchange",
                            serde_json::json!({"goodsId": goods_id, "count": exchange_count}),
                            cx,
                        )
                    })),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::remaining_suffix;
    use serde_json::json;

    #[test]
    fn shop_remaining_suffix_follows_vue_truthiness() {
        assert_eq!(remaining_suffix(&json!({}), false), "");
        assert_eq!(remaining_suffix(&json!({"remaining": null}), false), "");
        assert_eq!(remaining_suffix(&json!({"remaining": ""}), false), "");
        assert_eq!(remaining_suffix(&json!({"remaining": 0}), false), "");
        assert_eq!(remaining_suffix(&json!({"remaining": "0"}), false), " · 剩 0");
        assert_eq!(remaining_suffix(&json!({"remaining": 3}), true), " · 3 remaining");
    }
}
