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
        // 顶部两列并排（对齐 Tauri md:grid-cols-2 + NForm label-placement=left
        // label-width=140：标签在固定宽列内左对齐——右对齐会在标签前留出一大块空白）。
        let form_label =
            |id: &'static str, label: String| self.settings_form_label(id.to_string(), label, cx);
        let strategy_row = h_flex()
            .items_center()
            .gap_3()
            .child(form_label(
                "planting-strategy-label",
                settings_text(&self.preferences.language, "种植策略"),
            ))
            .child(
                Select::new(&self.settings.planting_strategy)
                    .id("planting-strategy-select")
                    .w_full()
                    .accessibility_label(settings_text(&self.preferences.language, "种植策略"))
                    .disabled(self.loading),
            );
        let fallback = self
            .settings
            .draft
            .get("bagSeedFallbackStrategy")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("preferred");
        let second = if selected == "preferred"
            || (selected == "bag_priority" && fallback == "preferred")
        {
            h_flex()
                .items_center()
                .gap_3()
                .child(form_label(
                    "preferred-seed-label",
                    settings_text(&self.preferences.language, "指定种子"),
                ))
                .child(
                    Select::new(&self.settings.preferred_seed)
                        .id("preferred-seed-select")
                        .w_full()
                        .accessibility_label(settings_text(&self.preferences.language, "指定种子"))
                        .disabled(self.loading),
                )
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
            h_flex()
                .items_center()
                .gap_3()
                .child(form_label("strategy-preview-label", label.to_string()))
                .child(
                    div()
                        .id("strategy-preview-value")
                        .test_support()
                        .flex_1()
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
                )
        };
        controls =
            controls.child(div().grid().grid_cols(2).gap_4().child(strategy_row).child(second));
        if selected == "bag_priority" {
            controls = self.append_bag_seed_priority_controls(controls, cx);
        }

        controls = self.append_strategy_timing_controls(controls, cx);

        controls
    }
}
