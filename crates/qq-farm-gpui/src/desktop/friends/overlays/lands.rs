use super::super::super::*;
use crate::desktop::land_grid;
use crate::desktop::personal::lands::{
    care_flags, land_badge_with_color, land_status_color, land_status_label, render_countdown,
    soil_label,
};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::StatefulInteractiveElement;
#[cfg(test)]
use gpui_kit::TestSupportExt;

fn land_title(land: &serde_json::Value) -> &str {
    land.get("plantName")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .unwrap_or("-")
}

fn land_phase(land: &serde_json::Value) -> &str {
    land.get("phaseName")
        .and_then(serde_json::Value::as_str)
        .filter(|phase| !phase.is_empty())
        .unwrap_or("-")
}

impl FarmDesktop {
    pub(super) fn render_friend_lands(
        &self,
        gid: i64,
        all_lands: &[serde_json::Value],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = &self.preferences.language;
        let english = language == "en-US";
        let lands = land_grid::visible_lands(all_lands);
        if lands.is_empty() {
            let label = crate::i18n::text(language, "无土地数据", "No land data");
            let empty = div()
                .id("friend-lands-empty")
                .aria_label(label.to_string())
                .py_4()
                .text_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label);
            #[cfg(test)]
            let empty = empty.test_support();
            return empty.into_any_element();
        }

        div()
            .child(div().grid().grid_cols(4).grid_rows_min_content(6).gap_2().children(
                lands.into_iter().map(|land| {
                    let id = land
                        .get("id")
                        .and_then(serde_json::Value::as_i64)
                        .or_else(|| land.get("id")?.as_str()?.parse::<i64>().ok())
                        .unwrap_or_default();
                    let placement = land_grid::land_placement(land);
                    let title = land_title(land);
                    let status =
                        land.get("status").and_then(serde_json::Value::as_str).unwrap_or_default();
                    let occupied = land
                        .get("occupiedByMaster")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false);
                    let title_tooltip = land
                        .get("plantName")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let phase = land_phase(land);
                    let mature = land
                        .get("matureInSec")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or_default();
                    let remaining = mature
                        .saturating_add(self.friends.detail_loaded_at)
                        .saturating_sub(self.friends.countdown_now_sec)
                        .max(0);
                    let total_grow_time = land
                        .get("totalGrowTime")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or_default();
                    let progress = if total_grow_time > 0 {
                        ((total_grow_time - remaining).clamp(0, total_grow_time) as f32
                            / total_grow_time as f32)
                            .clamp(0., 1.)
                    } else {
                        0.
                    };
                    let current_season =
                        land.get("currentSeason").and_then(serde_json::Value::as_i64).unwrap_or(1);
                    let total_season = land
                        .get("totalSeason")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or_default();
                    let level =
                        land.get("level").and_then(serde_json::Value::as_i64).unwrap_or_default();
                    let preview = render_asset_preview(
                        &DisplayRow {
                            id: format!("friend-land-{gid}-{id}"),
                            section: "lands".into(),
                            title: title.into(),
                            detail: String::new(),
                            state: String::new(),
                            record: land.clone(),
                        },
                        cx,
                    );
                    let care = care_flags(land, language, cx);
                    let soil = soil_label(level, language);
                    let mut tile = div()
                        .id(format!("friend-land-{gid}-{id}"))
                        .v_flex()
                        .col_start(placement.col as i16)
                        .col_end((placement.col + placement.col_span) as i16)
                        .row_start(placement.row as i16)
                        .row_end((placement.row + placement.row_span) as i16)
                        .min_w_0()
                        .min_h_0()
                        .min_h(px(140.))
                        .gap_1()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().group_box)
                        .p_2()
                        .child(
                            h_flex()
                                .justify_between()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(land_grid::land_id_label(land))
                                .child(land_badge_with_color(
                                    land_status_label(status, occupied, language),
                                    land_status_color(status, occupied, cx),
                                    cx,
                                )),
                        );
                    if let Some(preview) = preview {
                        tile = tile.child(div().flex().justify_center().child(preview));
                    } else {
                        tile = tile.child(div().flex().justify_center().py_1().child("🌱"));
                    }
                    let tile = tile
                        .child(
                            div()
                                .id(format!("friend-land-name-{gid}-{id}"))
                                .truncate()
                                .text_center()
                                .text_sm()
                                .font_medium()
                                .child(title.to_string())
                                .tooltip(move |window, cx| {
                                    Tooltip::new(title_tooltip.clone()).build(window, cx)
                                }),
                        )
                        .child(render_countdown(remaining, total_grow_time, progress, phase, cx))
                        .when(soil.is_some() || total_season > 1, |tile| {
                            tile.child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_1()
                                    .children(soil.into_iter().map(|label| {
                                        div()
                                            .rounded(cx.theme().radius)
                                            .bg(cx.theme().background)
                                            .px_1()
                                            .text_xs()
                                            .child(label)
                                    }))
                                    .when(total_season > 1, |row| {
                                        row.child(
                                            div()
                                                .rounded(cx.theme().radius)
                                                .bg(cx.theme().background)
                                                .px_1()
                                                .text_xs()
                                                .child(if english {
                                                    format!(
                                                        "Season {current_season}/{total_season}"
                                                    )
                                                } else {
                                                    format!("第{current_season}/{total_season}季")
                                                }),
                                        )
                                    }),
                            )
                        })
                        .when(!care.is_empty(), |tile| {
                            tile.child(h_flex().flex_wrap().gap_1().children(
                                care.into_iter().enumerate().map(
                                    |(index, (icon, label, color))| {
                                        land_badge_with_color(icon.to_string(), color, cx)
                                            .id(("friend-land-care", index))
                                            .aria_label(label.to_string())
                                    },
                                ),
                            ))
                        });
                    #[cfg(test)]
                    let tile = tile.test_support();
                    tile.into_any_element()
                }),
            ))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{land_phase, land_title};
    use crate::desktop::land_grid::visible_lands;
    use crate::desktop::personal::lands::{land_status_label, soil_label};

    #[test]
    fn friend_land_labels_follow_vue_localized_statuses() {
        assert_eq!(land_status_label("growing", false, "en-US"), "Growing");
        assert_eq!(land_status_label("future", false, "zh-CN"), "future");
        assert_eq!(land_status_label("empty", true, "zh-CN"), "合种占用");
        assert_eq!(land_title(&serde_json::json!({})), "-");
        assert_eq!(land_phase(&serde_json::json!({})), "-");
        assert_eq!(soil_label(4, "en-US"), Some("Golden soil"));
    }

    #[test]
    fn friend_land_display_hides_merged_slave_cells_like_vue() {
        let lands = vec![
            serde_json::json!({ "id": 1, "occupiedLandIds": [1, 2], "plantSize": 2 }),
            serde_json::json!({ "id": 2, "occupiedByMaster": true }),
            serde_json::json!({ "id": 3, "plantSize": 1 }),
        ];
        let visible = visible_lands(&lands);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0]["id"], 1);
        assert_eq!(visible[1]["id"], 3);
    }
}
