use super::super::super::*;

fn text(value: Option<&serde_json::Value>, fallback: &str) -> String {
    value
        .map(value_to_action_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn status_label(goods: &serde_json::Value, language: &str) -> (&'static str, bool) {
    let english = language == "en-US";
    let status =
        goods.get("purchaseStatus").and_then(serde_json::Value::as_str).unwrap_or_default();
    let (label, disabled) = match status {
        "owned" => (if english { "Owned" } else { "已拥有" }, true),
        "sold_out" if goods.get("isFree").and_then(serde_json::Value::as_bool) == Some(true) => {
            (if english { "Reward claimed" } else { "奖励已领取" }, true)
        }
        "sold_out" => (if english { "Sold out" } else { "已售罄" }, true),
        "ad_required" => (if english { "Claim in game" } else { "游戏内广告领取" }, true),
        "share_required" => {
            (if english { "Share in game to claim" } else { "游戏内分享后领取" }, true)
        }
        "svip_required" => (if english { "SVIP required" } else { "需要 SVIP" }, true),
        "unavailable" => (if english { "Unavailable" } else { "暂不可购买" }, true),
        _ => (if english { "Buy" } else { "购买" }, false),
    };
    let purchasable = goods.get("purchasable").and_then(serde_json::Value::as_bool);
    (label, disabled || purchasable == Some(false))
}

fn price_text(goods: &serde_json::Value, language: &str) -> String {
    if goods.get("isFree").and_then(serde_json::Value::as_bool) == Some(true) {
        return crate::i18n::text(language, "免费", "Free").into();
    }
    let price = goods.get("price").unwrap_or(&serde_json::Value::Null);
    let count = text(price.get("count"), "0");
    let name = price
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{}", text(price.get("id"), "?")));
    format!("{count} {name}")
}

pub(super) fn status_chip(label: String, color: Hsla, cx: &App) -> Div {
    div()
        .rounded(cx.theme().radius)
        .bg(color.opacity(0.14))
        .px_2()
        .py_1()
        .text_xs()
        .text_color(color)
        .child(label)
}

pub(super) fn render(
    goods: &serde_json::Value,
    index: usize,
    language: &str,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let id = text(goods.get("id"), "?");
    let name =
        text(goods.get("name"), &format!("{} #{id}", crate::i18n::text(language, "商品", "Item")));
    let reward = goods.pointer("/rewards/0").cloned().unwrap_or_else(|| serde_json::json!({}));
    let reward_image = render_asset_preview_sized(
        &DisplayRow {
            id: format!("mall-reward-{id}"),
            section: "mall".into(),
            title: name.clone(),
            detail: String::new(),
            state: String::new(),
            record: reward,
        },
        64.,
        cx,
    )
    .unwrap_or_else(|| {
        div()
            .size(px(64.))
            .flex()
            .items_center()
            .justify_center()
            .text_2xl()
            .child("🛒")
            .into_any_element()
    });
    let original_price = goods
        .get("originalPrice")
        .filter(|value| {
            !value.is_null()
                && goods.get("isFree").and_then(serde_json::Value::as_bool) != Some(true)
        })
        .map(value_to_action_string);
    let discount = text(goods.get("discountText"), "");
    let limit = goods.get("limit").filter(|limit| !limit.is_null()).map(|limit| {
        format!(
            "{} {}/{}",
            crate::i18n::text(language, "限购", "Limit"),
            text(limit.get("bought"), "0"),
            text(limit.get("max"), "0")
        )
    });
    let status =
        goods.get("purchaseStatus").and_then(serde_json::Value::as_str).unwrap_or_default();
    let unavailable_reason = goods
        .get("unavailableReason")
        .and_then(serde_json::Value::as_str)
        .filter(|reason| !reason.is_empty() && status != "available")
        .map(str::to_string);
    let (label, disabled) = status_label(goods, language);
    let goods_for_purchase = goods.clone();

    let mut price_row = h_flex().items_center().gap_2().text_xs();
    if let Some(original_price) = original_price {
        let original_label =
            format!("{} {original_price}", crate::i18n::text(language, "原价", "Original"));
        let original_price = div()
            .id(format!("mall-original-price-{id}-{index}"))
            .aria_label(original_label)
            .text_color(cx.theme().muted_foreground)
            .line_through()
            .child(original_price);
        #[cfg(test)]
        let original_price = original_price.test_support();
        price_row = price_row.child(original_price);
    }
    price_row = price_row.child(
        div()
            .id(format!("mall-current-price-{id}-{index}"))
            .text_color(cx.theme().success)
            .child(price_text(goods, language)),
    );
    let mut price_details = div()
        .v_flex()
        .gap_1()
        .min_h(px(40.))
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(price_row);
    if !discount.is_empty() {
        price_details = price_details.child(div().text_color(cx.theme().danger).child(discount));
    }
    if let Some(limit) = limit {
        price_details = price_details.child(limit);
    }
    if let Some(reason) = unavailable_reason {
        price_details = price_details.child(div().text_color(cx.theme().danger).child(reason));
    }

    let name_label = div()
        .id(format!("mall-name-{id}-{index}"))
        .aria_label(name.clone())
        .min_w_0()
        .min_h(px(40.))
        .text_sm()
        .font_medium()
        .line_clamp(2)
        .child(name);
    #[cfg(test)]
    let name_label = name_label.test_support();

    div()
        .v_flex()
        .gap_2()
        .w(px(220.))
        .min_h(px(220.))
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .p_3()
        .child(div().h(px(72.)).flex().items_center().justify_center().child(reward_image))
        .child(name_label)
        .child(price_details)
        .child(
            Button::new(format!("mall-buy-{id}-{index}"))
                .label(label)
                .w_full()
                .when(!disabled, |button| button.primary())
                .when(disabled, |button| button.disabled(true))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if goods_for_purchase.get("purchasable").and_then(serde_json::Value::as_bool)
                        != Some(true)
                    {
                        return;
                    }
                    this.open_purchase_confirmation(goods_for_purchase.clone(), window, cx);
                })),
        )
}
