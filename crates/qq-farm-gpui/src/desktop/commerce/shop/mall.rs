use super::super::super::*;
use super::card;
use gpui_kit::TestSupportExt;

impl FarmDesktop {
    pub(in crate::desktop) fn render_mall_shop_panel(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        if self.selected_account.is_none() {
            let label = crate::i18n::text(language, "请先选择农场账号", "Select a farm account");
            let empty = div()
                .id("mall-no-account")
                .aria_label(label.to_string())
                .v_flex()
                .items_center()
                .gap_3()
                .py_12()
                .text_color(cx.theme().muted_foreground)
                .child(label)
                .test_support();
            return div().child(empty);
        }

        let catalog = self.page_raw.get("catalog").unwrap_or(&serde_json::Value::Null);
        let visible_goods =
            catalog.get("goods").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();

        let mut header = h_flex().items_center().flex_wrap().gap_2();
        if let Some(membership) = catalog.get("membership") {
            let is_svip =
                membership.get("isSvip").and_then(serde_json::Value::as_bool) == Some(true);
            let member_label = if is_svip {
                let days = membership
                    .get("remainingDays")
                    .map(value_to_action_string)
                    .unwrap_or_else(|| "0".into());
                if language == "en-US" {
                    format!("SVIP · {days} days left")
                } else {
                    format!("SVIP · 剩余 {days} 天")
                }
            } else {
                crate::i18n::text(language, "非 SVIP 会员", "Non-SVIP member").into()
            };
            header = header.child(card::status_chip(
                member_label,
                if is_svip { cx.theme().warning } else { cx.theme().muted_foreground },
                cx,
            ));
        }
        if let Some(diamond) = self.page_raw.get("diamond") {
            header = header.child(card::status_chip(
                format!(
                    "{} {}",
                    crate::i18n::text(language, "钻石", "Diamonds"),
                    value_to_action_string(diamond)
                ),
                cx.theme().warning,
                cx,
            ));
        }
        let total = visible_goods.len();
        let countdown =
            catalog.get("refreshCountdown").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let count_label = if countdown > 0 {
            if language == "en-US" {
                format!("{total} items · refresh in {countdown}s")
            } else {
                format!("{total} 件商品 · {countdown}s 后刷新")
            }
        } else {
            if language == "en-US" {
                format!("{total} items")
            } else {
                format!("{total} 件商品")
            }
        };
        header = header.child(
            div().flex_1().text_xs().text_color(cx.theme().muted_foreground).child(count_label),
        );
        header = header.child(
            Button::new("refresh-mall")
                .label(crate::i18n::text(language, "刷新", "Refresh"))
                .ghost()
                .disabled(self.loading)
                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
        );

        let mut panel = div()
            .v_flex()
            .gap_4()
            .child(div().text_lg().font_medium().child(crate::i18n::text(
                language,
                "游戏商城",
                "Game Shop",
            )))
            .child(header);
        if visible_goods.is_empty() {
            let empty_label = crate::i18n::text(language, "暂无商品", "No goods available");
            let empty = div()
                .id("mall-empty")
                .aria_label(empty_label.to_string())
                .py_10()
                .text_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(empty_label)
                .test_support();
            return panel.child(empty);
        }

        let cards = visible_goods
            .iter()
            .enumerate()
            .map(|(index, good)| card::render(good, index, language, cx));
        panel = panel.child(h_flex().flex_wrap().gap_3().children(cards));
        panel
    }
}
