use super::super::*;
use super::shared::{game_config_cell, game_config_frame};

impl FarmDesktop {
    pub(super) fn render_game_config_cell(
        &self,
        row: &DisplayRow,
        column: &str,
        width: f32,
        index: usize,
        stretch: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        if let Some(fallback) = match column {
            "seed" => Some("🌱"),
            "fruit" => Some("🍎"),
            "item" => Some("🎒"),
            _ => None,
        } {
            let preview = render_record_asset_preview_sized(&row.record, 32., cx)
                .unwrap_or_else(|| div().text_lg().child(fallback).into_any_element());
            return game_config_frame(width, stretch)
                .flex()
                .min_w_0()
                .items_center()
                .justify_center()
                .gap_2()
                .px_2()
                .child(preview)
                .child(div().min_w_0().truncate().font_medium().child(row.title.clone()));
        }
        if column == "operate" {
            return game_config_frame(width, stretch)
                .flex()
                .items_center()
                .child(self.render_row_actions(row, index, cx))
                .justify_center()
                .gap_1();
        }

        let language = &self.preferences.language;
        let value = match row.section.as_str() {
            "seeds" => super::seeds::cell_value(row, column, language),
            "fruits" => super::fruits::cell_value(row, column, language),
            "items" => super::items::cell_value(&self.page_raw, row, column, language),
            _ => None,
        }
        .unwrap_or_else(|| "-".into());
        game_config_cell(&value, width, stretch)
            .text_sm()
            .text_color(cx.theme().foreground)
            .truncate()
    }
}

#[cfg(test)]
mod tests {
    use super::super::items::item_type_label;
    use serde_json::json;

    #[test]
    fn item_type_uses_the_catalog_label_and_falls_back_to_its_numeric_value() {
        let catalog = json!({"itemTypes": [{"value": 7, "label": "化肥"}]});
        assert_eq!(item_type_label(&catalog, 7), "化肥");
        assert_eq!(item_type_label(&catalog, 99), "99");
    }
}
