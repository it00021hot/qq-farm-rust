use self::shared::{column_width, configured_columns, game_config_row_id};
use super::super::*;
use super::columns::GameConfigColumnPin;
use gpui_kit::base::v_flex;

mod cells;
mod fruits;
mod header;
mod items;
mod rows;
mod seeds;
mod selection;
mod shared;

impl FarmDesktop {
    pub(in crate::desktop) fn render_game_config_table(
        &self,
        visible_row_indices: Vec<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let settings = self
            .game_config
            .columns
            .get(&self.page_section)
            .cloned()
            .unwrap_or_else(|| super::columns::default_game_config_columns(&self.page_section));
        let left_columns = configured_columns(&settings, GameConfigColumnPin::Left);
        let center_columns = configured_columns(&settings, GameConfigColumnPin::Unfixed);
        let right_columns = configured_columns(&settings, GameConfigColumnPin::Right);
        let no_visible_rows = visible_row_indices.is_empty();
        let page_ids = visible_row_indices
            .iter()
            .filter_map(|index| game_config_row_id(&self.rows[*index]))
            .collect::<Vec<_>>();
        let all_selected = !page_ids.is_empty()
            && page_ids.iter().all(|id| {
                self.game_config.selected_ids.contains(&(self.page_section.clone(), *id))
            });
        let left_width = column_width(&left_columns);
        let center_width = column_width(&center_columns) + 42.;
        let right_width = column_width(&right_columns);
        let column_scroll = window
            .use_keyed_state("game-config-table-column-scroll", cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let row_scroll = window
            .use_keyed_state("game-config-table-row-scroll", cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let left = (left_width > 0.).then(|| {
            self.render_game_config_pinned_lane(
                &left_columns,
                &visible_row_indices,
                left_width,
                "left",
                &row_scroll,
                false,
                cx,
            )
        });
        let right = (right_width > 0.).then(|| {
            self.render_game_config_pinned_lane(
                &right_columns,
                &visible_row_indices,
                right_width,
                "right",
                &row_scroll,
                true,
                cx,
            )
        });
        let center_header = self.render_game_config_table_header(
            &center_columns,
            center_width,
            page_ids,
            all_selected,
            true,
            true,
            "center",
            cx,
        );
        let center_rows: AnyElement = if no_visible_rows {
            let empty_label = if self.game_config_filtering() {
                crate::i18n::text(
                    &self.preferences.language,
                    "没有符合条件的配置",
                    "No matching configurations",
                )
            } else {
                crate::i18n::text(&self.preferences.language, "无数据", "No Data")
            };
            let empty = div()
                .id("game-config-empty")
                .aria_label(empty_label.to_string())
                .px_4()
                .py_8()
                .text_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(empty_label);
            #[cfg(test)]
            let empty = empty.test_support();
            empty.into_any_element()
        } else {
            div()
                .children(visible_row_indices.iter().map(|index| {
                    self.render_game_config_table_row(
                        *index,
                        &center_columns,
                        center_width,
                        true,
                        true,
                        cx,
                    )
                }))
                .into_any_element()
        };
        let center_body = div()
            .id("game-config-table-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .track_scroll(&row_scroll)
            .child(center_rows);
        let center = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .min_h_0()
            .horizontal_scrollbar(&column_scroll)
            .when(right_width <= 0., |center| center.vertical_scrollbar(&row_scroll))
            .child(
                div()
                    .id("game-config-table-columns-scroll")
                    .size_full()
                    .min_h_0()
                    .overflow_x_scroll()
                    .restrict_scroll_to_axis()
                    .track_scroll(&column_scroll)
                    .child(
                        v_flex()
                            .w_full()
                            .min_w(px(center_width))
                            .h_full()
                            .min_h_0()
                            .child(center_header)
                            .child(center_body),
                    ),
            );

        let total = self.game_config_filtered_total();
        let page_size = self.game_config.page_size.max(1);
        let page_count = total.div_ceil(page_size).max(1);
        let page = self.game_config.page.min(page_count - 1);
        let start_index = page * page_size + 1;
        let end_index = (page * page_size + page_size).min(total);
        let selected = self
            .game_config
            .selected_ids
            .iter()
            .filter(|(section, _)| section == &self.page_section)
            .count();
        let table = h_flex()
            .id("game-config-table")
            .flex_1()
            .min_h_0()
            .items_start()
            .min_w_0()
            .when_some(left, |table, left| table.child(left))
            .child(center)
            .when_some(right, |table, right| table.child(right));

        div()
            .relative()
            .v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(
                div()
                    .relative()
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(table),
            )
            .child(self.render_table_pagination(
                TablePager {
                    target: TablePagerTarget::GameConfig,
                    page,
                    page_count,
                    page_size,
                    page_size_options: &[10, 20, 50, 100],
                    total,
                    start_index,
                    end_index,
                    selected,
                    page_size_disabled: false,
                },
                cx,
            ))
            .into_any_element()
    }

    /// One pinned column lane: a fixed header row plus a body that scrolls
    /// vertically in lockstep with the other lanes through the shared handle.
    #[allow(clippy::too_many_arguments)]
    fn render_game_config_pinned_lane(
        &self,
        columns: &[(&'static str, f32)],
        visible_row_indices: &[usize],
        width: f32,
        lane: &'static str,
        row_scroll: &ScrollHandle,
        show_scrollbar: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let header = self.render_game_config_table_header(
            columns,
            width,
            Vec::new(),
            false,
            false,
            false,
            lane,
            cx,
        );
        let rows = div()
            .id(format!("game-config-pinned-{lane}-rows"))
            .size_full()
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .track_scroll(row_scroll)
            .children(visible_row_indices.iter().map(|index| {
                self.render_game_config_table_row(*index, columns, width, false, false, cx)
            }));
        let body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .when(show_scrollbar, |body| body.vertical_scrollbar(row_scroll))
            .child(rows);
        div()
            .v_flex()
            .w(px(width))
            .flex_none()
            .h_full()
            .min_h_0()
            .child(header)
            .child(body)
            .into_any_element()
    }

    fn game_config_filtering(&self) -> bool {
        !self.game_config.applied_keyword.trim().is_empty()
            || self.game_config.applied_seasons.is_some()
            || self.game_config.applied_rarity.is_some()
            || self.game_config.applied_item_type.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::shared::{
        format_game_config_grow_time, format_game_config_price, game_config_rarity_label,
    };

    #[test]
    fn game_config_table_formats_fields_like_vue_catalog() {
        assert_eq!(format_game_config_grow_time(3599, "zh-CN"), "59分");
        assert_eq!(format_game_config_grow_time(3660, "zh-CN"), "1时1分");
        assert_eq!(format_game_config_grow_time(59, "en-US"), "59s");
        assert_eq!(format_game_config_price(12, 1005, "zh-CN"), "12 金豆");
        assert_eq!(game_config_rarity_label(4, "en-US"), "Epic");
    }
}
