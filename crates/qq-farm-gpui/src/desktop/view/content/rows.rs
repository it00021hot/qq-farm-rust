use std::rc::Rc;

use super::*;
use crate::desktop::view::grid::{chunk_indices, estimated_grid_rows, responsive_grid_columns};

impl FarmDesktop {
    pub(super) fn render_row_contents(
        &self,
        visible_row_indices: Vec<usize>,
        rows_use_grid: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.page == Page::GameConfig {
            return self.render_game_config_table(visible_row_indices, window, cx);
        }
        let virtualize_rows = !rows_use_grid && visible_row_indices.len() > 80;
        let virtualize_grid = rows_use_grid && visible_row_indices.len() > 80;
        let row_contents: AnyElement = if virtualize_grid {
            let card_width = if self.page == Page::GameConfig { 280. } else { 270. };
            let columns = responsive_grid_columns(
                window,
                self.sidebar_collapsed,
                self.preferences.sidebar_width,
                card_width,
                12.,
                48.,
            );
            let grid_rows = chunk_indices(&visible_row_indices, columns);
            let entity = cx.entity();
            let virtual_rows = grid_rows.clone();
            let sizes = Rc::new(estimated_grid_rows(grid_rows.len(), 360.));
            let list = gpui_kit::component::v_virtual_list(
                entity,
                "business-grid-virtual-list",
                sizes,
                move |this, range, _, cx| {
                    range
                        .map(|virtual_index| {
                            let indices = &virtual_rows[virtual_index];
                            let row = div()
                                .id(("business-grid-row", virtual_index))
                                .w_full()
                                .pb_3()
                                .child(h_flex().items_stretch().gap_3().children(
                                    indices.iter().map(|row_index| {
                                        this.render_business_row(
                                            &this.rows[*row_index],
                                            *row_index,
                                            cx,
                                        )
                                    }),
                                ));
                            #[cfg(test)]
                            let row = row.test_support();
                            row
                        })
                        .collect::<Vec<_>>()
                },
            )
            .with_sizing_behavior(gpui_kit::ListSizingBehavior::Infer)
            .flex_1()
            .min_h_0();
            let viewport = div().id("business-grid-viewport").flex_1().min_h_0().child(list);
            #[cfg(test)]
            let viewport = viewport.test_support();
            viewport.into_any_element()
        } else if virtualize_rows {
            let entity = cx.entity();
            let virtual_indices = visible_row_indices.clone();
            let sizes =
                Rc::new(virtual_indices.iter().map(|_| Size::new(px(1.), px(92.))).collect());
            gpui_kit::component::v_virtual_list(
                entity,
                "business-rows-virtual-list",
                sizes,
                move |this, range, _, cx| {
                    range
                        .map(|visible_index| {
                            let row_index = virtual_indices[visible_index];
                            div().pb_3().child(this.render_business_row(
                                &this.rows[row_index],
                                row_index,
                                cx,
                            ))
                        })
                        .collect::<Vec<_>>()
                },
            )
            .with_sizing_behavior(gpui_kit::ListSizingBehavior::Infer)
            .flex_1()
            .min_h_0()
            .into_any_element()
        } else {
            div()
                .v_flex()
                .gap_3()
                .when(rows_use_grid, |rows| rows.flex_wrap())
                .children(
                    visible_row_indices
                        .iter()
                        .map(|index| self.render_business_row(&self.rows[*index], *index, cx)),
                )
                .into_any_element()
        };
        row_contents
    }
}
