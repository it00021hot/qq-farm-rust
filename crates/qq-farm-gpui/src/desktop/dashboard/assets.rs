use super::super::*;
use super::format::{
    bag_count, bucket_time, format_clock, format_number, int_at, metric, metric_colored, value_text,
};

impl FarmDesktop {
    pub(super) fn render_assets_card(&self, cx: &mut Context<Self>) -> Div {
        let status = self.page_raw.get("status").unwrap_or(&serde_json::Value::Null);
        let bag = self.page_raw.get("bag").unwrap_or(&serde_json::Value::Null);
        let language = self.preferences.language.as_str();
        let online = int_at(status, &["runStatus"]) == 1;
        let uptime = int_at(status, &["uptime"]).max(0);
        let coupon = if online { bag_count(bag, 1002) } else { 0 };
        let gold_bean = if online { bag_count(bag, 1005) } else { 0 };
        let diamond = if online {
            self.page_raw.get("diamond").map(value_text).unwrap_or_else(|| "-".into())
        } else {
            "-".into()
        };
        let assets_card = div()
            .v_flex()
            .flex_1()
            .min_w(px(230.))
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                div().text_sm().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                    language,
                    "农场资产",
                    "Farm assets",
                )),
            )
            .child(
                // 2×2 网格（对齐 Tauri：金币/点券、钻石/金豆豆 各占一行两列）
                div().grid().grid_cols(2).gap_3().children([
                    metric_colored(
                        crate::i18n::text(language, "金币", "Gold"),
                        format_number(int_at(status, &["gold"])),
                        int_at(status, &["sessionGoldGained"]).to_string(),
                        cx.theme().warning,
                        cx,
                    ),
                    metric_colored(
                        crate::i18n::text(language, "点券", "Coupons"),
                        coupon.to_string(),
                        self.dashboard
                            .coupon_baseline
                            .map_or(0, |baseline| coupon - baseline)
                            .to_string(),
                        cx.theme().success,
                        cx,
                    ),
                    metric_colored(
                        crate::i18n::text(language, "钻石", "Diamonds"),
                        diamond,
                        String::new(),
                        cx.theme().info,
                        cx,
                    ),
                    metric_colored(
                        crate::i18n::text(language, "金豆豆", "Gold Beans"),
                        gold_bean.to_string(),
                        String::new(),
                        cx.theme().warning,
                        cx,
                    ),
                ]),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(8.)).rounded_full().bg(if online {
                                cx.theme().success
                            } else {
                                cx.theme().danger
                            }))
                            .child(div().text_xs().font_medium().child(crate::i18n::text(
                                language,
                                if online { "在线" } else { "离线" },
                                if online { "Online" } else { "Offline" },
                            ))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format_clock(uptime, language)),
                    ),
            )
            .when_some(
                status.get("lastError").and_then(serde_json::Value::as_str),
                |card, error| {
                    card.child(
                        div().text_xs().text_color(cx.theme().danger).child(error.to_string()),
                    )
                },
            );

        assets_card
    }

    pub(super) fn render_inventory_card(&self, cx: &mut Context<Self>) -> Div {
        let bag = self.page_raw.get("bag").unwrap_or(&serde_json::Value::Null);
        let language = self.preferences.language.as_str();
        let inventory_card = div()
            .v_flex()
            .flex_1()
            .min_w(px(230.))
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                div().text_sm().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                    language,
                    "化肥容器",
                    "Fertilizer",
                )),
            )
            .child(h_flex().flex_wrap().gap_4().children([
                metric(
                    crate::i18n::text(language, "普通", "Normal"),
                    bucket_time(bag, 1011),
                    String::new(),
                    cx,
                ),
                metric(
                    crate::i18n::text(language, "有机", "Organic"),
                    bucket_time(bag, 1012),
                    String::new(),
                    cx,
                ),
            ]))
            .child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::i18n::text(language, "收藏点", "Collection")),
            )
            .child(h_flex().flex_wrap().gap_4().children([
                metric(
                    crate::i18n::text(language, "普通", "Normal"),
                    bag_count(bag, 3001).to_string(),
                    String::new(),
                    cx,
                ),
                metric(
                    crate::i18n::text(language, "典藏", "Rare"),
                    bag_count(bag, 3002).to_string(),
                    String::new(),
                    cx,
                ),
            ]));

        inventory_card
    }
}
