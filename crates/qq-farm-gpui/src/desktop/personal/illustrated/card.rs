use super::super::super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::TestSupportExt;

pub(super) fn stars(category: i64) -> String {
    "★".repeat(category.saturating_sub(1).clamp(0, 3) as usize)
}

impl FarmDesktop {
    pub(super) fn render_illustrated_item(
        &self,
        item: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = item.get("seedId").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let name = item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(crate::i18n::text(&self.preferences.language, "种子", "Seed"));
        let unlocked = item.get("unlocked").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let status = crate::i18n::text(
            &self.preferences.language,
            if unlocked { "已解锁" } else { "未点亮" },
            if unlocked { "Unlocked" } else { "Not Lit" },
        );
        let progress = item.get("progress").and_then(serde_json::Value::as_i64).unwrap_or_default();
        let stars = stars(
            item.get("rewardCategory").and_then(serde_json::Value::as_i64).unwrap_or_default(),
        );
        let mut description = format!("{name} · {status}");
        if let Some(reward) = item.get("reward").filter(|reward| !reward.is_null()) {
            description.push_str(&format!(
                " · {} x{}",
                reward
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(crate::i18n::text(&self.preferences.language, "奖励", "Reward"),),
                reward.get("count").and_then(serde_json::Value::as_i64).unwrap_or_default()
            ));
        }
        let preview = render_record_asset_preview_sized(item, 40., cx);
        // 图鉴种子购买（bot 0f09702）：可购买项挂「购买」按钮；不可购买在 tooltip 说明原因
        let crop_book = self.farm.illustrated.book == "crop";
        let buyable = crop_book && super::purchase::can_buy(item);
        if crop_book && !buyable {
            if let Some(reason) =
                super::purchase::seed_availability_text(item, &self.preferences.language)
            {
                description.push_str(&format!(" · {reason}"));
            }
        }
        let buy_label = crate::i18n::text(&self.preferences.language, "购买", "Buy");
        div()
            .id(format!("illustrated-item-{}-{id}", self.farm.illustrated.book))
            .v_flex()
            .w(rems(6.))
            .h(rems(7.))
            .items_center()
            .justify_center()
            .gap_1()
            .rounded(cx.theme().radius)
            .bg(cx.theme().group_box)
            .p_2()
            .child(
                h_flex().w_full().justify_end().child(
                    Icon::new(if unlocked { IconName::Check } else { IconName::Lock })
                        .size_3()
                        .text_color(if unlocked {
                            cx.theme().success
                        } else {
                            cx.theme().muted_foreground
                        }),
                ),
            )
            .child(
                div().h(rems(2.5)).w(rems(2.5)).when(!unlocked, |image| image.opacity(0.35)).child(
                    preview.unwrap_or_else(|| {
                        Icon::new(IconName::Sprout)
                            .size_8()
                            .text_color(cx.theme().muted_foreground)
                            .into_any_element()
                    }),
                ),
            )
            .child(div().text_xs().text_color(cx.theme().warning).child(stars))
            .child(
                div()
                    .text_xs()
                    .text_color(if unlocked {
                        cx.theme().foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(format!(
                        "{progress}{}",
                        crate::i18n::text(&self.preferences.language, "点", " pts")
                    )),
            )
            .tooltip(move |window, cx| Tooltip::new(description.clone()).build(window, cx))
            .when(buyable, |card| {
                card.child({
                    let item = item.clone();
                    Button::new(format!("illustrated-buy-{id}"))
                        .label(buy_label)
                        .compact()
                        .primary()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_seed_purchase_confirmation(item.clone(), window, cx);
                        }))
                })
            })
            .test_support()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn collection_reward_tiers_use_one_two_or_three_stars() {
        assert_eq!(super::stars(1), "");
        assert_eq!(super::stars(2), "★");
        assert_eq!(super::stars(3), "★★");
        assert_eq!(super::stars(4), "★★★");
        assert_eq!(super::stars(10), "★★★");
    }
}
