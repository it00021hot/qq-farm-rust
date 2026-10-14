use super::super::super::*;

impl FarmDesktop {
    pub(super) fn render_settings_business_row(
        &self,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let actions = self.render_row_actions(row, index, cx);
        let title = row.title.clone();
        let detail = row.detail.clone();

        match self.page {
            Page::Settings => div()
                .h_flex()
                .items_center()
                .justify_between()
                .gap_4()
                .border_b_1()
                .border_color(cx.theme().border)
                .py_3()
                .child(
                    div().v_flex().min_w_0().gap_1().child(div().font_medium().child(title)).when(
                        !detail.is_empty(),
                        |line| {
                            line.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(detail),
                            )
                        },
                    ),
                )
                .child(actions)
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}
