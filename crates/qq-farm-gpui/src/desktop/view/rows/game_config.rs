use super::super::super::*;

impl FarmDesktop {
    pub(super) fn render_game_config_business_row(
        &self,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let actions = self.render_row_actions(row, index, cx);
        let preview = render_asset_preview(row, cx);
        let title = row.title.clone();
        let detail = row.detail.clone();
        let state = row.state.clone();

        match self.page {
            Page::GameConfig => {
                let card = div()
                    .id(format!("game-config-record-{index}"))
                    .v_flex()
                    .gap_3()
                    .w(px(280.))
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().group_box)
                    .p_4()
                    .when_some(preview, |card, image| card.child(image))
                    .child(div().font_semibold().child(title))
                    .when(!detail.is_empty(), |card| {
                        card.child(
                            div().text_sm().text_color(cx.theme().muted_foreground).child(detail),
                        )
                    })
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(state),
                            )
                            .child(actions),
                    );
                #[cfg(test)]
                let card = card.test_support();
                card.into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}
