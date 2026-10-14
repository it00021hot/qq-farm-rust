use super::super::super::*;

impl FarmDesktop {
    pub(super) fn render_dashboard_business_row(
        &self,
        row: &DisplayRow,
        _index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = row.title.clone();
        let detail = row.detail.clone();
        let state = row.state.clone();

        match self.page {
            Page::Dashboard if row.id == "account-status" => div()
                .v_flex()
                .gap_3()
                .rounded(cx.theme().radius_lg)
                .bg(cx.theme().group_box)
                .p_5()
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .v_flex()
                                .gap_1()
                                .child(div().text_lg().font_semibold().child(title))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(detail),
                                ),
                        )
                        .child(
                            div()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().background)
                                .px_3()
                                .py_2()
                                .child(state),
                        ),
                )
                .into_any_element(),
            Page::Dashboard => div()
                .h_flex()
                .items_start()
                .gap_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .py_3()
                .child(div().mt_1().size(px(8.)).rounded_full().bg(if state.is_empty() {
                    cx.theme().muted
                } else {
                    cx.theme().warning
                }))
                .child(div().v_flex().min_w_0().gap_1().child(div().child(title)).when(
                    !detail.is_empty(),
                    |line| {
                        line.child(
                            div().text_sm().text_color(cx.theme().muted_foreground).child(detail),
                        )
                    },
                ))
                .when(!state.is_empty(), |line| {
                    line.child(div().text_sm().text_color(cx.theme().warning).child(state))
                })
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}
