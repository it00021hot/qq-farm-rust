use super::super::*;

impl FarmDesktop {
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = &self.preferences.language;
        div()
            .id("farm-sidebar")
            .test_support()
            .v_flex()
            .w_full()
            .h_full()
            .gap_4()
            .p_3()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .when(!self.sidebar_collapsed, |sidebar| {
                sidebar.child(div().px_2().font_semibold().truncate().child("QQ Farm"))
            })
            .child(
                div()
                    .id("sidebar-navigation")
                    .v_flex()
                    .gap_1()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .children(Page::ALL.into_iter().map(|page| {
                        Button::new(page.id())
                            .accessibility_label(page.title_for(language))
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_2()
                                    .items_center()
                                    .when(self.sidebar_collapsed, |row| row.justify_center())
                                    .child(Icon::new(page.icon()))
                                    .when(!self.sidebar_collapsed, |row| {
                                        row.child(page.title_for(language))
                                    }),
                            )
                            .ghost()
                            .w_full()
                            .when(!self.sidebar_collapsed, |button| button.justify_start())
                            .when(self.page == page, |button| {
                                button
                                    .bg(cx.theme().sidebar_accent)
                                    .text_color(cx.theme().sidebar_accent_foreground)
                            })
                            .disabled(self.quitting)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_page(page, cx);
                                this.sync_search_placeholder(window, cx);
                            }))
                    })),
            )
            .when(!self.sidebar_collapsed, |sidebar| {
                sidebar.child(
                    div()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .pt_3()
                        .child(self.render_sidebar_account_switcher(cx)),
                )
            })
    }
}
