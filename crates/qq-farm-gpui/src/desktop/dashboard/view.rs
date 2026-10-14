use super::super::*;

impl FarmDesktop {
    pub(super) fn render_dashboard_view(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        div()
            .v_flex()
            .size_full()
            .min_h_0()
            .gap_3()
            .when(self.selected_account.is_none(), |view| {
                view.child(
                    div()
                        .id("dashboard-no-account")
                        .py_6()
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "请选择账号", "Select account"))
                        .test_support(),
                )
            })
            .when(self.selected_account.is_some(), |view| {
                view.child(
                    h_flex()
                        .flex_wrap()
                        .items_stretch()
                        .gap_3()
                        .child(self.render_account_card(cx))
                        .child(self.render_assets_card(cx))
                        .child(self.render_inventory_card(cx)),
                )
            })
            // Vue keeps the runtime-log panel and the schedule/operations
            // column outside the account grid, so they stay available even
            // without a selected account (see the 2026-10-09 sync entry).
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .gap_3()
                    .child(self.render_logs_panel(cx))
                    .child(
                        div()
                            .v_flex()
                            .w(px(280.))
                            .min_w(px(230.))
                            .min_h_0()
                            .gap_3()
                            .child(self.render_schedule_card(cx))
                            .child(self.render_operations_card(cx)),
                    ),
            )
    }
}
