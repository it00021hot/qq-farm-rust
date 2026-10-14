mod state;
pub(in crate::desktop) use state::BagState;

mod actions;
mod card;
mod clock;
mod controls;
mod feedback;
mod helpers;

use std::rc::Rc;

use super::super::*;
use crate::desktop::view::grid::{chunk_indices, estimated_grid_rows, responsive_grid_columns};
pub(crate) use helpers::bag_category_for_type;
use helpers::filtered_items_with_indices;

impl FarmDesktop {
    pub(in crate::desktop) fn render_personal_bag(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let language = &self.preferences.language;
        let query = self.search.read(cx).value().trim().to_lowercase();
        let items = filtered_items_with_indices(&self.page_raw, &self.farm.bag.category, &query);
        let item_indices = items.iter().map(|(index, _)| *index).collect::<Vec<_>>();
        let virtualize = item_indices.len() > 80;
        div()
            .v_flex()
            .gap_3()
            .when_some(self.farm.bag.notice.as_ref(), |panel, (message, success)| {
                panel.child(
                    div()
                        .text_sm()
                        .text_color(if *success { cx.theme().success } else { cx.theme().danger })
                        .child(message.clone()),
                )
            })
            .when(self.farm.bag.refreshing, |panel| {
                panel.child(
                    div().text_xs().text_color(cx.theme().muted_foreground).child(
                        crate::i18n::text(language, "正在刷新仓库…", "Refreshing warehouse…"),
                    ),
                )
            })
            .when_some(self.farm.bag.error.as_ref(), |panel, error| {
                panel.child(div().text_sm().text_color(cx.theme().danger).child(
                    if language == "en-US" {
                        format!("Warehouse refresh failed: {error}")
                    } else {
                        format!("仓库刷新失败：{error}")
                    },
                ))
            })
            .when(items.is_empty(), |panel| {
                let empty = if query.is_empty() {
                    crate::i18n::text(language, "无数据", "No Data")
                } else {
                    crate::i18n::text(language, "没有匹配的物品", "No matching items")
                };
                let empty_state = div()
                    .id("bag-empty")
                    .aria_label(empty.to_string())
                    .py_8()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(empty.to_string());
                #[cfg(test)]
                let empty_state = empty_state.test_support();
                panel.child(empty_state)
            })
            .when(!items.is_empty(), |panel| {
                if virtualize {
                    let columns = responsive_grid_columns(
                        window,
                        self.sidebar_collapsed,
                        self.preferences.sidebar_width,
                        170.,
                        12.,
                        48.,
                    );
                    let grid_rows = chunk_indices(&item_indices, columns);
                    let virtual_rows = grid_rows.clone();
                    let entity = cx.entity();
                    let sizes = Rc::new(estimated_grid_rows(grid_rows.len(), 270.));
                    let list = gpui_kit::component::v_virtual_list(
                        entity,
                        "personal-bag-virtual-list",
                        sizes,
                        move |this, range, _, cx| {
                            range
                                .map(|virtual_index| {
                                    let visible_indices = &virtual_rows[virtual_index];
                                    let row = div()
                                        .id(("personal-bag-grid-row", virtual_index))
                                        .w_full()
                                        .pb_3()
                                        .child(h_flex().items_stretch().gap_3().children(
                                            visible_indices.iter().enumerate().filter_map(
                                                |(column, raw_index)| {
                                                    this.page_raw
                                                        .pointer("/bag/items")
                                                        .and_then(serde_json::Value::as_array)
                                                        .and_then(|items| items.get(*raw_index))
                                                        .map(|item| {
                                                            this.render_personal_bag_card(
                                                                item,
                                                                virtual_index * columns + column,
                                                                cx,
                                                            )
                                                            .into_any_element()
                                                        })
                                                },
                                            ),
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
                    let viewport =
                        div().id("personal-bag-grid-viewport").flex_1().min_h_0().child(list);
                    #[cfg(test)]
                    let viewport = viewport.test_support();
                    panel.flex_1().min_h_0().child(viewport)
                } else {
                    panel.child(div().flex().flex_wrap().gap_3().children(
                        items.into_iter().enumerate().map(|(index, (_, item))| {
                            self.render_personal_bag_card(item, index, cx)
                        }),
                    ))
                }
            })
    }
}
