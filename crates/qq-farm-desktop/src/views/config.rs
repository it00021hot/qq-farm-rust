//! 游戏配置：种子/果实/道具列表。

use gpui::*;

use crate::app_state::AppState;
use crate::ui::*;
use crate::views::{card, empty_hint, ji64, jstr, section_title};

pub fn render(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let tab = state.read(cx).config_tab;
    let data = state.read(cx).config_seeds.clone();
    let rows = data.as_array().cloned().unwrap_or_default();

    v_flex()
        .gap_3()
        .child(section_title("游戏配置"))
        .child(
            h_flex().gap_2().children(
                [("种子", 0), ("果实", 1), ("道具", 2)]
                    .into_iter()
                    .map(|(label, i)| {
                        let state = state.clone();
                        Button::new(SharedString::from(format!("ctab-{i}")))
                            .small()
                            .selected(tab == i)
                            .label(label)
                            .on_click(move |_, _, cx| {
                                state.update(cx, |s, cx| {
                                    s.config_tab = i;
                                    s.config_seeds = match i {
                                        1 => qq_farm_app::config::list_fruits(),
                                        2 => qq_farm_app::config::list_items(),
                                        _ => qq_farm_app::config::list_seeds(),
                                    };
                                    cx.notify();
                                });
                            })
                    }),
            ),
        )
        .child(if rows.is_empty() {
            empty_hint("暂无配置项。", cx).into_any_element()
        } else {
            v_flex()
                .id("config-list")
                .gap_1()
                .max_h(px(520.))
                .overflow_y_scroll()
                .children(rows.into_iter().take(200).enumerate().map(|(i, row)| {
                    let name = jstr(&row, "/name");
                    let id = ji64(&row, "/seed_id")
                        .max(ji64(&row, "/seedId"))
                        .max(ji64(&row, "/id"));
                    let level = ji64(&row, "/land_level_need").max(ji64(&row, "/level"));
                    card(cx)
                        .id(SharedString::from(format!("cfg-{i}-{id}")))
                        .child(
                            h_flex()
                                .items_center()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(if name.is_empty() {
                                                    format!("#{id}")
                                                } else {
                                                    name
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("ID {id} · 等级 {level}")),
                                        ),
                                )
                                .when(tab == 0 && id > 0, |el| {
                                    let state = state.clone();
                                    el.child(
                                        Button::new(SharedString::from(format!("addbl-{id}")))
                                            .small()
                                            .ghost()
                                            .label("加入黑名单")
                                            .on_click(move |_, _, cx| {
                                                state.update(cx, |s, cx| {
                                                    let _ = qq_farm_app::farm::add_plant_blacklist(
                                                        &s.account_id,
                                                        id,
                                                    );
                                                    s.last_message =
                                                        Some(format!("已加入黑名单 {id}"));
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                }),
                        )
                }))
                .into_any_element()
        })
}
