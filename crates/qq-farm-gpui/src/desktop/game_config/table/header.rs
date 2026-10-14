use super::super::*;
use super::shared::game_config_cell;

impl FarmDesktop {
    pub(super) fn render_game_config_table_header(
        &self,
        columns: &[(&str, f32)],
        table_width: f32,
        page_ids: Vec<i64>,
        all_selected: bool,
        includes_selection: bool,
        stretch: bool,
        lane: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = &self.preferences.language;
        let section = self.page_section.clone();
        let labels = columns
            .iter()
            .map(|(key, width)| {
                let label = super::super::columns::game_config_column_title(key, language);
                (!label.is_empty()).then(|| {
                    let cell = game_config_cell(label, *width, stretch)
                        .id(format!("game-config-header-{key}"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground);
                    #[cfg(test)]
                    let cell = cell.test_support();
                    cell.into_any_element()
                })
            })
            .collect::<Vec<_>>();
        let mut header = h_flex()
            .id(match lane {
                "center" => "game-config-table-header",
                "left" => "game-config-table-left-header",
                _ => "game-config-table-right-header",
            })
            .h(px(46.))
            .flex_none()
            .items_center()
            .bg(cx.theme().group_box)
            .border_b_1()
            .border_color(cx.theme().border)
            .when(includes_selection, |header| {
                header.child(
                    div().w(px(42.)).flex_none().flex().justify_center().items_center().child(
                        Checkbox::new("game-config-select-page")
                            .checked(all_selected)
                            .accessibility_label(crate::i18n::text(
                                language,
                                "选择当前页配置",
                                "Select configurations on this page",
                            ))
                            .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                                for id in &page_ids {
                                    let key = (section.clone(), *id);
                                    if *checked {
                                        this.game_config.selected_ids.insert(key);
                                    } else {
                                        this.game_config.selected_ids.remove(&key);
                                    }
                                }
                                cx.notify();
                            })),
                    ),
                )
            });
        header = if stretch { header.w_full() } else { header.w(px(table_width)) };
        for label in labels.into_iter().flatten() {
            header = header.child(label);
        }
        #[cfg(test)]
        let header = header.test_support();
        header.into_any_element()
    }
}
