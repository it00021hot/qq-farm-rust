use super::super::super::*;
use gpui_kit::component::select::Select;
use gpui_kit::TestSupportExt;

impl FarmDesktop {
    pub(in crate::desktop) fn render_personal_interaction_items(
        &self,
        cx: &mut Context<Self>,
    ) -> Div {
        let language = &self.preferences.language;
        let items = self
            .page_raw
            .pointer("/interactionItems/items")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let message = self
            .page_raw
            .pointer("/interactionItems/message")
            .and_then(serde_json::Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .unwrap_or(crate::i18n::text(
                language,
                "背包中暂无可用道具",
                "No usable interaction items",
            ));
        div()
            .v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_semibold().child(crate::i18n::text(
                        language,
                        "特殊互动道具",
                        "Interaction Items",
                    )))
                    .child(
                        Button::new("personal-interaction-refresh")
                            .label(crate::i18n::text(language, "刷新", "Refresh"))
                            .ghost()
                            .loading(self.farm.interaction.refreshing)
                            .disabled(
                                self.loading
                                    || self.farm.interaction.using
                                    || self.farm.interaction.refreshing,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_personal_interaction_items(cx)
                            })),
                    ),
            )
            .when_some(self.farm.interaction.notice.as_ref(), |panel, (message, success)| {
                panel.child(
                    div()
                        .text_sm()
                        .text_color(if *success { cx.theme().success } else { cx.theme().danger })
                        .child(message.clone()),
                )
            })
            .when(items.is_empty(), |panel| {
                panel.child(
                    div()
                        .py_8()
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(message.to_string()),
                )
            })
            .when(!items.is_empty(), |panel| {
                panel
                    .child(
                        div()
                            .v_flex()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .px_3()
                            .children(items.iter().enumerate().map(|(index, item)| {
                                let id = item
                                    .get("itemId")
                                    .and_then(serde_json::Value::as_i64)
                                    .unwrap_or_default();
                                let name =
                                    item.get("name").and_then(serde_json::Value::as_str).unwrap_or(
                                        crate::i18n::text(language, "互动道具", "Interaction item"),
                                    );
                                let count = item
                                    .get("count")
                                    .and_then(serde_json::Value::as_i64)
                                    .unwrap_or_default();
                                let sellable = item
                                    .get("saleConditionSatisfiedCount")
                                    .and_then(serde_json::Value::as_i64)
                                    .unwrap_or_default();
                                let description = item
                                    .get("description")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("");
                                h_flex()
                                    .id(format!("personal-interaction-item-{id}"))
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .py_3()
                                    .when(index + 1 < items.len(), |row| {
                                        row.border_b_1().border_color(cx.theme().border)
                                    })
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_3()
                                            .child(div().font_medium().child(name.to_string()))
                                            .child(count_tag(format!("x{count}"), cx))
                                            .when(sellable > 0, |row| {
                                                row.child(sellable_tag(
                                                    format!("personal-interaction-sellable-{id}"),
                                                    crate::i18n::text(
                                                        language,
                                                        &format!("可出售: {sellable}"),
                                                        &format!("Sellable: {sellable}"),
                                                    )
                                                    .to_string(),
                                                    cx,
                                                ))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .max_w(rems(22.5))
                                            .truncate()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(description.to_string()),
                                    )
                                    .test_support()
                            })),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .flex_wrap()
                            .gap_3()
                            .child(
                                div().w(rems(14.)).flex_none().child(
                                    Select::new(&self.farm.interaction.picker)
                                        .id("personal-interaction-picker")
                                        .w_full()
                                        .accessibility_label(crate::i18n::text(
                                            language,
                                            "选择自用互动道具",
                                            "Select an item to use on your farm",
                                        ))
                                        .disabled(self.farm.interaction.using),
                                ),
                            )
                            .child(
                                Input::new(&self.farm.interaction.land_ids)
                                    .id("interaction-land-ids")
                                    .w(rems(18.)),
                            )
                            .child(
                                Button::new("personal-interaction-use")
                                    .label(crate::i18n::text(language, "使用", "Use"))
                                    .primary()
                                    .loading(self.farm.interaction.using)
                                    .disabled(
                                        self.loading
                                            || self.farm.interaction.using
                                            || self
                                                .farm
                                                .interaction
                                                .picker
                                                .read(cx)
                                                .selected_value()
                                                .is_none(),
                                    )
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.run_interaction_item(cx)),
                                    ),
                            ),
                    )
            })
    }
}

fn count_tag(label: String, cx: &App) -> AnyElement {
    div()
        .rounded_full()
        .bg(cx.theme().muted)
        .px_2()
        .py_1()
        .text_xs()
        .child(label)
        .into_any_element()
}

fn sellable_tag(id: String, label: String, cx: &App) -> AnyElement {
    let tag = div()
        .id(id)
        .aria_label(label.clone())
        .rounded_full()
        .bg(cx.theme().warning.opacity(0.16))
        .px_2()
        .py_1()
        .text_xs()
        .text_color(cx.theme().warning)
        .child(label);
    #[cfg(test)]
    let tag = tag.test_support();
    tag.into_any_element()
}
