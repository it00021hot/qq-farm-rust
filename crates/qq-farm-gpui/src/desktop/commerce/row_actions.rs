use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn append_commerce_row_actions(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page == Page::Mall {
            let status =
                row.record.get("purchaseStatus").and_then(serde_json::Value::as_str).unwrap_or("");
            let allowed =
                row.record.get("purchasable").and_then(serde_json::Value::as_bool).unwrap_or(
                    status.is_empty() || status == "available" || status == "purchasable",
                ) && (self.commerce.mall_slot_type != -1
                    || (row
                        .record
                        .get("active")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true)
                        && row
                            .record
                            .get("stock")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(1)
                            > 0
                        && row
                            .record
                            .pointer("/price/balance")
                            .and_then(serde_json::Value::as_i64)
                            .zip(
                                row.record
                                    .pointer("/price/count")
                                    .and_then(serde_json::Value::as_i64),
                            )
                            .is_none_or(|(balance, price)| balance >= price)));
            let english = self.preferences.language == "en-US";
            let label = match (status, english) {
                ("owned", true) => "Owned",
                ("owned", false) => "已拥有",
                ("sold_out", true) => "Sold out",
                ("sold_out", false) => "已售罄",
                ("ad_required", true) => "Claim in game",
                ("ad_required", false) => "游戏内广告领取",
                ("share_required", true) => "Share in game to claim",
                ("share_required", false) => "游戏内分享后领取",
                ("svip_required", true) => "SVIP required",
                ("svip_required", false) => "需要 SVIP",
                ("unavailable", true) => "Unavailable",
                ("unavailable", false) => "暂不可购买",
                (_, true) => "Buy…",
                (_, false) => "购买…",
            };
            if allowed {
                let goods = row.record.clone();
                actions =
                    actions.child(Button::new(("buy", index)).label(label).primary().on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.open_purchase_confirmation(goods.clone(), window, cx);
                        }),
                    ));
            } else if !status.is_empty() {
                actions = actions.child(
                    Button::new(("buy-disabled", index)).label(label).ghost().disabled(true),
                );
            }
        }
        actions
    }
}
