use super::super::super::*;
use gpui_kit::component::select::Select;

mod bag_seed_priority;

impl FarmDesktop {
    pub(super) fn append_planting_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.selected_account.is_none() {
            return controls.child(
                div()
                    .id("settings-select-account-strategy")
                    .test_support()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_4()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::i18n::text(
                        &self.preferences.language,
                        "请选择账号",
                        "Select account",
                    )),
            );
        }

        let selected = self
            .settings
            .draft
            .get("plantingStrategy")
            .or_else(|| self.settings.draft.get("strategy"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("max_exp");
        controls = controls.child(
            div()
                .v_flex()
                .gap_1()
                .child({
                    let label = div()
                        .id("planting-strategy-label")
                        .text_sm()
                        .child(settings_text(&self.preferences.language, "种植策略"));
                    #[cfg(test)]
                    let label = label.test_support();
                    label
                })
                .child(
                    Select::new(&self.settings.planting_strategy)
                        .id("planting-strategy-select")
                        .w(rems(24.))
                        .accessibility_label(settings_text(&self.preferences.language, "种植策略"))
                        .disabled(self.loading),
                ),
        );
        let fallback = self
            .settings
            .draft
            .get("bagSeedFallbackStrategy")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("preferred");
        if selected == "preferred" || (selected == "bag_priority" && fallback == "preferred") {
            controls = controls.child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .child(settings_text(&self.preferences.language, "指定种子")),
                    )
                    .child(
                        Select::new(&self.settings.preferred_seed)
                            .id("preferred-seed-select")
                            .w(rems(24.))
                            .accessibility_label(settings_text(
                                &self.preferences.language,
                                "指定种子",
                            ))
                            .disabled(self.loading),
                    ),
            );
        } else {
            let label = if selected == "bag_priority" {
                crate::i18n::text(
                    &self.preferences.language,
                    "第二优先策略预览",
                    "Fallback Strategy Preview",
                )
            } else {
                crate::i18n::text(
                    &self.preferences.language,
                    "策略选种预览",
                    "Strategy Seed Preview",
                )
            };
            let preview = self
                .settings
                .strategy_preview
                .as_ref()
                .map(|preview| preview.label(&self.preferences.language))
                .unwrap_or_else(|| {
                    crate::i18n::text(&self.preferences.language, "加载中...", "Loading...")
                        .to_string()
                });
            controls = controls.child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        div()
                            .id("strategy-preview-label")
                            .test_support()
                            .text_sm()
                            .aria_label(label)
                            .child(label),
                    )
                    .child(
                        div()
                            .id("strategy-preview-value")
                            .test_support()
                            .w_full()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().group_box)
                            .px_3()
                            .py_2()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .aria_label(preview.clone())
                            .child(preview),
                    ),
            );
        }
        if selected == "bag_priority" {
            controls = self.append_bag_seed_priority_controls(controls, cx);
        }

        controls = self.append_strategy_timing_controls(controls, cx);

        controls
    }
}
