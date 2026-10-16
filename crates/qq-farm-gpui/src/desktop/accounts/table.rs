mod cells;
mod header;
mod pinned;
mod rows;

use super::super::*;
use super::AccountColumnPin;
use gpui_kit::base::v_flex;

/// Column cell sizing for one lane. Pinned lanes keep the column's design
/// width; the center lane grows every column proportionally so the table
/// fills the panel instead of leaving a dead strip before the pinned lane.
/// 表头与字段文字统一居中（对齐 Tauri 表格）。
pub(super) fn account_cell_frame(stretch: bool, width: f32) -> Div {
    let cell = div().min_w_0().text_center();
    if stretch {
        cell.flex_basis(px(width)).flex_grow(width).flex_shrink(1.).min_w(px(width))
    } else {
        cell.w(px(width)).flex_none()
    }
}

impl FarmDesktop {
    pub(super) fn render_account_table(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        page_records: Vec<serde_json::Value>,
        page_ids: Vec<String>,
        all_selected: bool,
        total: usize,
        start_index: usize,
        end_index: usize,
        page: usize,
        page_count: usize,
    ) -> impl IntoElement {
        let left_columns: Vec<_> = self
            .accounts
            .columns
            .iter()
            .copied()
            .filter(|column| column.visible && column.pin == AccountColumnPin::Left)
            .collect();
        let center_columns: Vec<_> = self
            .accounts
            .columns
            .iter()
            .copied()
            .filter(|column| column.visible && column.pin == AccountColumnPin::Unfixed)
            .collect();
        let right_columns: Vec<_> = self
            .accounts
            .columns
            .iter()
            .copied()
            .filter(|column| column.visible && column.pin == AccountColumnPin::Right)
            .collect();
        let center_width = center_columns.iter().map(|column| column.id.width()).sum::<f32>() + 60.;
        let column_scroll = window
            .use_keyed_state("account-table-column-scroll", cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let row_scroll = window
            .use_keyed_state("account-table-row-scroll", cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let center_body = {
            let body = div()
                .id("account-table-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .restrict_scroll_to_axis()
                .track_scroll(&row_scroll)
                .children(page_records.iter().enumerate().map(|(index, record)| {
                    self.render_account_row(&center_columns, index, true, true, record, cx)
                }))
                .when(total == 0 && !self.loading, |body| {
                    let empty_label = if self.accounts_filtering() {
                        crate::i18n::text(
                            &self.preferences.language,
                            "没有符合条件的账号",
                            "No accounts match these filters",
                        )
                    } else {
                        crate::i18n::text(&self.preferences.language, "无数据", "No Data")
                    };
                    body.child({
                        let empty = div()
                            .id("accounts-empty")
                            .aria_label(empty_label.to_string())
                            .v_flex()
                            .items_center()
                            .gap_2()
                            .py_16()
                            .text_color(cx.theme().muted_foreground)
                            .child(empty_label);
                        #[cfg(test)]
                        let empty = empty.test_support();
                        empty
                    })
                });
            #[cfg(test)]
            let body = body.test_support();
            body
        };
        let center = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .min_h_0()
            .horizontal_scrollbar(&column_scroll)
            .when(right_columns.is_empty(), |center| center.vertical_scrollbar(&row_scroll))
            .child(
                div()
                    .id("account-table-columns-scroll")
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
                            .child(self.render_account_header(
                                &center_columns,
                                Some((page_ids, all_selected)),
                                true,
                                cx,
                            ))
                            .child(center_body),
                    ),
            );
        let table_columns = h_flex()
            .id("account-table-columns")
            .items_start()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .when(!left_columns.is_empty(), |table| {
                table.child(self.render_account_pinned_lane(
                    &left_columns,
                    &page_records,
                    "left",
                    &row_scroll,
                    false,
                    cx,
                ))
            })
            .child(center)
            .when(!right_columns.is_empty(), |table| {
                table.child(self.render_account_pinned_lane(
                    &right_columns,
                    &page_records,
                    "right",
                    &row_scroll,
                    true,
                    cx,
                ))
            });
        #[cfg(test)]
        let table_columns = table_columns.test_support();

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
                    .child(table_columns),
            )
            .child(self.render_table_pagination(
                TablePager {
                    target: TablePagerTarget::Accounts,
                    page,
                    page_count,
                    page_size: self.accounts.page_size,
                    page_size_options: &[10, 15, 20, 25, 30],
                    total,
                    start_index,
                    end_index,
                    selected: self.accounts.selected.len(),
                    page_size_disabled: self.loading,
                },
                cx,
            ))
    }

    fn accounts_filtering(&self) -> bool {
        !self.accounts.keyword_applied.trim().is_empty()
            || self.accounts.platform_filter.is_some()
            || self.accounts.running_filter.is_some()
            || self.accounts.auth_filter.is_some()
    }
}
