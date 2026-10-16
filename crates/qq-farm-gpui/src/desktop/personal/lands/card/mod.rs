use super::super::super::*;
use super::clock::format_land_countdown;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::StatefulInteractiveElement;
use gpui_kit::TestSupportExt;

mod crop;
mod effects;
mod labels;

pub(in crate::desktop) use crop::render_countdown;
use crop::render_crop;
pub(in crate::desktop) use effects::care_flags;
use effects::{
    interaction_names, interaction_tooltip, mutant_names, render_interaction, render_mutation,
};
use labels::{json_number, land_badge};
pub(in crate::desktop) use labels::{
    land_badge_with_color, land_status_color, land_status_label, soil_label,
};

impl FarmDesktop {
    pub(super) fn render_personal_land_card(
        &self,
        land: &serde_json::Value,
        placement: crate::desktop::land_grid::LandPlacement,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = &self.preferences.language;
        let id = json_number(land.get("id"));
        let status = land.get("status").and_then(serde_json::Value::as_str).unwrap_or_default();
        let occupied =
            land.get("occupiedByMaster").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let remaining = self.personal_land_remaining_seconds(land);
        let total = json_number(land.get("totalGrowTime")).max(0);
        let plant_name =
            land.get("plantName").and_then(serde_json::Value::as_str).unwrap_or_default();
        let plant = if plant_name.is_empty() { "-" } else { plant_name };
        let plant_display = plant.to_string();
        let plant_tooltip = plant_name.to_string();
        let status_color = land_status_color(status, occupied, cx);
        let status_label = land_status_label(status, occupied, language);
        let level = json_number(land.get("level"));
        let size = json_number(land.get("plantSize"));
        let phase = land.get("phaseName").and_then(serde_json::Value::as_str).unwrap_or("-");
        let progress = if total > 0 {
            ((total - remaining).max(0) as f32 / total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let mut card = div()
            .id(format!("farm-land-card-{id}"))
            .v_flex()
            .col_start(placement.col as i16)
            .col_end((placement.col + placement.col_span) as i16)
            .row_start(placement.row as i16)
            .row_end((placement.row + placement.row_span) as i16)
            .min_w_0()
            .min_h(px(152.))
            .gap_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_3()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(crate::desktop::land_grid::land_id_label(land)),
                            )
                            // 土地详情入口（bot e5f3552 LandCard「土地详情」按钮）
                            .child({
                                let detail_land = land.clone();
                                Button::new(format!("farm-land-detail-{id}"))
                                    .label(crate::i18n::text(language, "详情", "Details"))
                                    .compact()
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_land_detail(detail_land.clone(), window, cx);
                                    }))
                            }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .when(size > 1, |row| {
                                row.child(land_badge(
                                    crate::i18n::text(
                                        language,
                                        &format!("合种 {size}x{size}"),
                                        &format!("Joint {size}x{size}"),
                                    )
                                    .to_string(),
                                    cx,
                                ))
                            })
                            .child(land_badge_with_color(status_label, status_color, cx)),
                    ),
            )
            .child(render_crop(land, cx))
            .child(
                div()
                    .id(format!("farm-land-plant-{id}"))
                    .min_w_0()
                    .truncate()
                    .text_center()
                    .font_medium()
                    .child(plant_display)
                    .tooltip(move |window, cx| {
                        Tooltip::new(plant_tooltip.clone()).build(window, cx)
                    })
                    .test_support(),
            )
            .child(render_countdown(remaining, total, progress, phase, cx));

        let mutant_names = mutant_names(land);
        let purple_percent =
            json_number(land.get("purpleCrystalResonanceExpBonus")).saturating_add(50) / 100;
        if !mutant_names.is_empty() || purple_percent > 0 {
            card = card.child(render_mutation(land, &mutant_names, purple_percent, language, cx));
        }

        let interactions = interaction_names(land);
        let interaction_tooltip = interaction_tooltip(land);
        let needs_cleanup = land
            .get("needInteractionCleanup")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !interactions.is_empty() || needs_cleanup {
            card = card.child(render_interaction(
                &interactions,
                interaction_tooltip,
                needs_cleanup,
                language,
                cx,
            ));
        }

        let total_season = json_number(land.get("totalSeason"));
        let current_season = json_number(land.get("currentSeason")).max(1);
        let care = care_flags(land, language, cx);
        if soil_label(level, language).is_some() || total_season > 1 || !care.is_empty() {
            card = card.child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .when_some(soil_label(level, language), |row, soil| {
                        row.child(land_badge(soil.to_string(), cx))
                    })
                    .when(total_season > 1, |row| {
                        row.child(land_badge(
                            crate::i18n::text(
                                language,
                                &format!("第{current_season}/{total_season}季"),
                                &format!("Season {current_season}/{total_season}"),
                            )
                            .to_string(),
                            cx,
                        ))
                    })
                    .children(care.into_iter().enumerate().map(|(index, (icon, label, color))| {
                        land_badge_with_color(icon.to_string(), color, cx)
                            .id(("farm-land-care", index))
                            .aria_label(label.to_string())
                    })),
            );
        }

        card.test_support()
    }
}

#[cfg(test)]
mod tests {
    use super::{land_status_label, mutant_names};
    use crate::desktop::personal::lands::clock::format_land_countdown;
    use serde_json::json;

    #[test]
    fn land_labels_preserve_vue_status_mutation_and_countdown_semantics() {
        assert_eq!(land_status_label("growing", false, "zh-CN"), "生长中");
        assert_eq!(land_status_label("empty", true, "zh-CN"), "合种占用");
        assert_eq!(land_status_label("growing", false, "en-US"), "Growing");
        assert_eq!(format_land_countdown(0), "00:00");
        assert_eq!(mutant_names(&json!({"mutantConfigIds": [20, 21]})), ["20", "21"]);
    }
}
