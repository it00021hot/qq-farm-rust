use super::super::super::*;
use gpui_kit::component::combobox::Combobox;
use serde_json::Value;
use std::collections::HashSet;

fn numeric_id(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
            .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
    })
}

fn blacklist_item_name(row: Option<&Value>, seed_id: i64, language: &str) -> String {
    row.and_then(|row| row.get("name").and_then(Value::as_str))
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if language == "en-US" {
                format!("Seed ID: {seed_id}")
            } else {
                format!("种子ID:{seed_id}")
            }
        })
}

impl FarmDesktop {
    pub(super) fn render_analytics_blacklist(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = &self.preferences.language;
        let selected_ids = self
            .page_raw
            .get("plantBlacklist")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| numeric_id(Some(value)))
            .collect::<Vec<_>>();
        let selected = selected_ids.iter().copied().collect::<HashSet<_>>();
        let saving = self.analytics.blacklist_save_pending;
        let controls_disabled = self.loading || saving;
        let rankings = self
            .page_raw
            .get("rankings")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();

        let mut selected_list = div().v_flex().gap_2();
        for seed_id in &selected_ids {
            let row = rankings.iter().find(|row| numeric_id(row.get("seedId")) == Some(*seed_id));
            let name = blacklist_item_name(row, *seed_id, language);
            let image =
                row.map(|row| self.render_analytics_plant_image(row, cx)).unwrap_or_else(|| {
                    div()
                        .size(px(36.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xl()
                        .child("🌱")
                        .into_any_element()
                });
            let remove_id = *seed_id;
            selected_list = selected_list.child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .px_3()
                    .py_2()
                    .child(
                        h_flex()
                            .min_w_0()
                            .flex_1()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(40.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(cx.theme().radius)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().background)
                                    .child(image),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .min_w_0()
                                    .child(div().truncate().font_medium().child(name))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("ID: {seed_id}")),
                                    ),
                            ),
                    )
                    .child(
                        Button::new(format!("analytics-blacklist-remove-{seed_id}"))
                            .label(crate::i18n::text(language, "移出黑名单", "Remove"))
                            .ghost()
                            .danger()
                            .loading(saving)
                            .disabled(controls_disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.change_plant_blacklist(remove_id, false, window, cx)
                            })),
                    ),
            );
        }

        div()
            .id("analytics-blacklist")
            .v_flex()
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(div().font_semibold().child(crate::i18n::text(language, "偷菜黑名单", "Steal blacklist")))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(crate::i18n::text(
                                        language,
                                        "加入黑名单的作物在自动偷菜时会被跳过，但不会影响自己种植",
                                        "Blacklisted crops are skipped when auto-stealing; planting is unaffected",
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("blacklist-add-all")
                                    .label(crate::i18n::text(language, "一键全部加入黑名单", "Add all to blacklist"))
                                    .ghost()
                                    .loading(saving)
                                    .disabled(controls_disabled || rankings.is_empty())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.set_all_plant_blacklist(true, window, cx)
                                    })),
                            )
                            .when(!selected.is_empty(), |actions| {
                                actions.child(
                                    Button::new("blacklist-clear-all")
                                    .label(crate::i18n::text(language, "清空黑名单", "Clear blacklist"))
                                    .ghost()
                                    .danger()
                                    .loading(saving)
                                    .disabled(controls_disabled)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_all_plant_blacklist(false, window, cx)
                                        })),
                                )
                            }),
                    ),
            )
            .when_some(self.analytics.blacklist_feedback.as_ref(), |panel, message| {
                panel.child(
                    div()
                        .id("analytics-blacklist-feedback")
                        .text_sm()
                        .text_color(if self.analytics.blacklist_feedback_is_error {
                            cx.theme().danger
                        } else {
                            cx.theme().success
                        })
                        .child(message.clone())
                        .test_support(),
                )
            })
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                        crate::i18n::text(language, "搜索并添加作物", "Search and add crops"),
                    ))
                    .child(
                        div().w_full().h(px(40.)).child(
                            Combobox::new(&self.analytics.blacklist_selector)
                                .placeholder(crate::i18n::text(
                                    language,
                                    "搜索并选择作物（可多选）",
                                    "Search and select crops",
                                ))
                                .search_placeholder(crate::i18n::text(
                                    language,
                                    "输入作物名称或编号",
                                    "Enter a crop name or ID",
                                ))
                                .menu_max_h(rems(20.))
                                .cleanable(true)
                                .disabled(controls_disabled),
                        ),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .when(selected.is_empty(), |section| {
                        section.child(
                            div()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().background)
                                .px_3()
                                .py_6()
                                .text_center()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(crate::i18n::text(language, "暂无黑名单作物", "No blacklisted crops")),
                        )
                    })
                    .when(!selected.is_empty(), |section| section.child(selected_list)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::blacklist_item_name;

    #[test]
    fn empty_or_missing_name_uses_the_vue_seed_id_fallback() {
        let empty_name = serde_json::json!({ "name": "" });
        assert_eq!(blacklist_item_name(Some(&empty_name), 42, "zh-CN"), "种子ID:42");
        assert_eq!(blacklist_item_name(None, 42, "en-US"), "Seed ID: 42");
    }

    #[test]
    fn known_name_is_preserved() {
        let row = serde_json::json!({ "name": "Wheat" });
        assert_eq!(blacklist_item_name(Some(&row), 42, "en-US"), "Wheat");
    }
}
