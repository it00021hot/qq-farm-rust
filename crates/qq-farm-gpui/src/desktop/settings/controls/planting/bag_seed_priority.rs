use super::super::super::super::*;
use gpui_kit::component::select::Select;

#[derive(Clone)]
struct BagSeedDrag(i64);

impl Render for BagSeedDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(8.))
    }
}

impl FarmDesktop {
    pub(super) fn append_bag_seed_priority_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        controls = self.append_setting_switch(
            controls,
            "bagSeedMultiLandReservationEnabled",
            "多格种子预留空地",
            self.loading,
            cx,
        );
        controls = controls.child(
            div()
                .v_flex()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .child(settings_text(&self.preferences.language, "第二优先策略")),
                )
                .child(
                    Select::new(&self.settings.bag_fallback)
                        .id("bag-seed-fallback-select")
                        .w(rems(24.))
                        .accessibility_label(settings_text(
                            &self.preferences.language,
                            "第二优先策略",
                        ))
                        .disabled(self.loading),
                ),
        );
        let language = &self.preferences.language;
        let mut panel = div()
            .v_flex()
            .w_full()
            .gap_2()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_3()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .child(settings_text(language, "背包种子优先顺序")),
                            )
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                                settings_text(language, "优先种植顺序（拖动或使用 ↑ ↓ 调整）"),
                            )),
                    )
                    .child(
                        Button::new("bag-seed-priority-reset")
                            .label(settings_text(language, "重置顺序"))
                            .ghost()
                            .disabled(self.loading)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.reset_bag_seed_priority(cx)),
                            ),
                    ),
            );

        let order_hint = crate::i18n::text(
            language,
            "先按下方顺序消耗背包中的 1x1 / 2x2 种子；背包种子不足时，再按“第二优先策略”补种。切换第二优先策略或重置时会据此重新排序。",
            "Consume 1x1 / 2x2 bag seeds in the order below first; when bag seeds run out, fall back to the secondary strategy. Changing the fallback strategy or resetting will re-sort accordingly.",
        )
        .to_string();
        panel = panel.child(
            div()
                .id("bag-seed-order-hint")
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .aria_label(order_hint.clone())
                .child(order_hint)
                .test_support(),
        );

        if let Some(error) = self.page_raw.get("bagSeedsError").and_then(serde_json::Value::as_str)
        {
            panel = panel.child(
                div()
                    .id("bag-seeds-error")
                    .test_support()
                    .w_full()
                    .py_4()
                    .text_sm()
                    .text_color(cx.theme().warning)
                    .child(error.to_string()),
            );
            return controls.child(panel);
        }

        let Some(seeds) = self.page_raw.get("bagSeeds").and_then(serde_json::Value::as_array)
        else {
            panel = panel.child(
                div()
                    .id("bag-seeds-loading")
                    .test_support()
                    .w_full()
                    .py_4()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(settings_text(language, "正在读取背包种子…")),
            );
            return controls.child(panel);
        };
        let mut seeds = seeds.clone();
        let priority = self
            .settings
            .draft
            .get("bagSeedPriority")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_i64)
            .collect::<Vec<_>>();
        seeds.sort_by_key(|seed| {
            let id = seed.get("seedId").and_then(serde_json::Value::as_i64).unwrap_or_default();
            priority.iter().position(|candidate| *candidate == id).unwrap_or(usize::MAX)
        });
        if seeds.is_empty() {
            panel = panel.child(
                div()
                    .id("bag-seeds-empty")
                    .test_support()
                    .w_full()
                    .py_4()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(settings_text(language, "当前没有可排序的背包种子。")),
            );
            return controls.child(panel);
        }

        let seed_count = seeds.len();
        let cards = seeds.iter().enumerate().map(|(index, seed)| {
            let id = seed.get("seedId").and_then(serde_json::Value::as_i64).unwrap_or_default();
            let name = seed
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| settings_text(&self.preferences.language, "种子"));
            let (size, info) = seed_priority_details(seed, &self.preferences.language);

            div()
                .id(format!("bag-seed-priority-{id}"))
                .test_support()
                .on_drag(BagSeedDrag(id), |payload, _, _, cx| cx.new(|_| payload.clone()))
                .on_drop(cx.listener(move |this, drag: &BagSeedDrag, _, cx| {
                    this.drop_bag_seed_priority(drag.0, id, cx)
                }))
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .p_2()
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(28.))
                                .h(px(28.))
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .justify_center()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().muted)
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().muted_foreground)
                                .child((index + 1).to_string()),
                        )
                        .child(
                            div()
                                .v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            div()
                                                .min_w_0()
                                                .truncate()
                                                .text_sm()
                                                .font_medium()
                                                .child(name),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("{size}x{size}")),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(info),
                                ),
                        )
                        .child(
                            div()
                                .v_flex()
                                .flex_shrink_0()
                                .gap_1()
                                .child(
                                    Button::new(format!("bag-seed-up-{id}"))
                                        .label("↑")
                                        .ghost()
                                        .disabled(index == 0 || self.loading)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.set_bag_seed_priority(id, -1, cx)
                                        })),
                                )
                                .child(
                                    Button::new(format!("bag-seed-down-{id}"))
                                        .label("↓")
                                        .ghost()
                                        .disabled(index + 1 == seed_count || self.loading)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.set_bag_seed_priority(id, 1, cx)
                                        })),
                                ),
                        ),
                )
        });
        panel = panel.child(div().grid().grid_cols(3).w_full().gap_2().children(cards));
        controls.child(panel)
    }
}

fn numeric_seed_field(seed: &serde_json::Value, key: &str) -> String {
    seed.get(key)
        .and_then(|value| {
            value
                .as_i64()
                .map(|number| number.to_string())
                .or_else(|| value.as_str().map(str::to_owned))
        })
        .unwrap_or_else(|| "0".to_owned())
}

fn seed_priority_details(seed: &serde_json::Value, language: &str) -> (String, String) {
    let id = numeric_seed_field(seed, "seedId");
    let count = numeric_seed_field(seed, "count");
    let level = numeric_seed_field(seed, "requiredLevel");
    let size = numeric_seed_field(seed, "plantSize");
    let info = if language == "en-US" {
        format!("In stock {count} · Lv{level} · ID {id}")
    } else {
        format!("库存 {count} · Lv{level} · ID {id}")
    };
    (size, info)
}

#[cfg(test)]
mod tests {
    use super::seed_priority_details;

    #[test]
    fn seed_priority_details_include_size_stock_level_and_id() {
        let seed = serde_json::json!({
            "seedId": 10,
            "count": 3,
            "requiredLevel": 4,
            "plantSize": 2
        });
        assert_eq!(
            seed_priority_details(&seed, "zh-CN"),
            ("2".to_owned(), "库存 3 · Lv4 · ID 10".to_owned())
        );
        assert_eq!(
            seed_priority_details(&seed, "en-US"),
            ("2".to_owned(), "In stock 3 · Lv4 · ID 10".to_owned())
        );
    }
}
