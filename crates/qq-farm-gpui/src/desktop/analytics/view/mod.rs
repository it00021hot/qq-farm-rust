mod blacklist;
mod strategy;

use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn render_analytics_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.selected_account.is_none() {
            let label =
                crate::i18n::text(&self.preferences.language, "请选择账号", "Select account");
            let empty = div()
                .id("analytics-no-account")
                .aria_label(label.to_string())
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child(label)
                .test_support()
                .into_any_element();
            return empty;
        }

        div()
            .v_flex()
            .gap_4()
            .p_6()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .when(self.page_section == "strategy", |panel| {
                panel.child(self.render_analytics_strategy(cx))
            })
            .when(self.page_section == "plantBlacklist", |panel| {
                panel.child(self.render_analytics_blacklist(cx))
            })
            .into_any_element()
    }
}
