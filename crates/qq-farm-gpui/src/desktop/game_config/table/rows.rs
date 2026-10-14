use super::super::*;
use super::shared::game_config_row_id;

impl FarmDesktop {
    pub(super) fn render_game_config_table_row(
        &self,
        index: usize,
        columns: &[(&str, f32)],
        table_width: f32,
        includes_selection: bool,
        stretch: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = &self.rows[index];
        let language = &self.preferences.language;
        let section = row.section.clone();
        let id = game_config_row_id(row);
        let selected =
            id.is_some_and(|id| self.game_config.selected_ids.contains(&(section.clone(), id)));
        let accessibility_label = if language == "en-US" {
            format!("Select {}", row.title)
        } else {
            format!("选择 {}", row.title)
        };
        let mut selection = Checkbox::new(format!("game-config-select-{index}"))
            .checked(selected)
            .disabled(id.is_none())
            .accessibility_label(accessibility_label);
        if let Some(id) = id {
            selection = selection.on_change(cx.listener(move |this, checked: &bool, _, cx| {
                this.set_game_config_selection(&section, id, *checked, cx);
            }));
        }
        let mut cells = h_flex()
            .h(px(62.))
            .flex_none()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .when(selected, |row| row.bg(cx.theme().group_box))
            .when(includes_selection, |row| {
                row.child(
                    div()
                        .w(px(42.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(selection),
                )
            });
        cells = if stretch { cells.w_full() } else { cells.w(px(table_width)) };
        for (column, width) in columns {
            let cell = self
                .render_game_config_cell(row, column, *width, index, stretch, cx)
                .id(format!("game-config-cell-{column}-{index}"));
            #[cfg(test)]
            let cell = cell.test_support();
            cells = cells.child(cell);
        }
        #[cfg(test)]
        let cells = if includes_selection {
            cells.id(format!("game-config-record-{index}")).test_support().into_any_element()
        } else {
            cells.into_any_element()
        };
        #[cfg(not(test))]
        let cells = cells.into_any_element();
        cells
    }
}
