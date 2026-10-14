mod state;
pub(in crate::desktop) use state::LandsState;

mod card;
mod clock;
mod summary;

pub(in crate::desktop) use card::{
    care_flags, land_badge_with_color, land_status_color, land_status_label, render_countdown,
    soil_label,
};

use super::super::*;
use crate::desktop::land_grid;

impl FarmDesktop {
    pub(in crate::desktop) fn render_personal_lands(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        let land_payload = self.page_raw.get("lands");
        let lands = land_payload
            .and_then(|payload| payload.get("lands"))
            .and_then(serde_json::Value::as_array);
        let keyword = self.search.read(cx).value().trim().to_lowercase();

        div()
            .v_flex()
            .gap_4()
            .when(self.farm.lands.refreshing, |panel| {
                panel.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "正在刷新地块…", "Refreshing lands…")),
                )
            })
            .when_some(self.farm.lands.error.as_ref(), |panel, error| {
                panel.child(div().text_sm().text_color(cx.theme().danger).child(
                    if language == "en-US" {
                        format!("Land refresh failed: {error}")
                    } else {
                        format!("地块刷新失败：{error}")
                    },
                ))
            })
            .when_some(land_payload, |panel, payload| {
                panel
                    .when_some(payload.get("career"), |panel, career| {
                        panel.child(summary::render_career(career, language, cx))
                    })
                    .when_some(payload.get("summary"), |panel, summary| {
                        panel.child(summary::render_land_summary(summary, language, cx))
                    })
            })
            .when_some(lands, |panel, lands| {
                let visible = land_grid::visible_lands(lands)
                    .into_iter()
                    .filter(|land| land_grid::matches_search(land, &keyword))
                    .collect::<Vec<_>>();
                if visible.is_empty() {
                    panel.child(
                        div().py_8().text_center().text_color(cx.theme().muted_foreground).child(
                            if keyword.is_empty() {
                                crate::i18n::text(language, "无数据", "No Data").to_string()
                            } else {
                                crate::i18n::text(language, "没有匹配的地块", "No matching lands")
                                    .to_string()
                            },
                        ),
                    )
                } else {
                    let mut land_grid = div().grid().grid_cols(4).grid_rows_min_content(6).gap_3();
                    for land in visible {
                        let placement = land_grid::land_placement(land);
                        land_grid =
                            land_grid.child(self.render_personal_land_card(land, placement, cx));
                    }
                    panel.child(land_grid)
                }
            })
            .when(lands.is_none(), |panel| {
                panel.child(
                    div().py_8().text_center().text_color(cx.theme().muted_foreground).child(
                        if self.selected_account.is_some() {
                            crate::i18n::text(
                                language,
                                "启动账号后即可查看农场",
                                "Start an account to view the farm",
                            )
                            .to_string()
                        } else {
                            crate::i18n::text(language, "请选择账号", "Select account").to_string()
                        },
                    ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::desktop::land_grid::{land_id_label, land_placement, visible_lands};
    use serde_json::json;

    #[test]
    fn merged_lands_hide_slave_cells_and_keep_the_four_column_footprint() {
        let lands = vec![
            json!({"id": 1, "plantSize": 2, "occupiedLandIds": [1, 2, 5, 6]}),
            json!({"id": 2, "occupiedByMaster": true}),
            json!({"id": 3}),
            json!({"id": 5, "occupiedByMaster": true}),
            json!({"id": 6, "occupiedByMaster": true}),
        ];

        let visible = visible_lands(&lands);
        assert_eq!(
            visible.iter().map(|land| land["id"].as_i64().unwrap()).collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(land_id_label(&lands[0]), "#1-6");
        assert_eq!(
            land_placement(&lands[0]),
            super::super::super::land_grid::LandPlacement {
                col: 1,
                row: 1,
                col_span: 2,
                row_span: 2,
            }
        );
    }

    #[test]
    fn non_numeric_or_missing_land_ids_fall_back_to_the_first_slot() {
        assert_eq!(land_placement(&json!({"id": "bad"})).col, 1);
        assert_eq!(land_id_label(&json!({})), "#0");
    }
}
