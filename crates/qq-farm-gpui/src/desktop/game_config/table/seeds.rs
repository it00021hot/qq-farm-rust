use super::super::*;
use super::shared::{format_game_config_grow_time, game_config_number, game_config_text};

pub(super) fn cell_value(row: &DisplayRow, column: &str, language: &str) -> Option<String> {
    Some(match column {
        "seedId" => game_config_text(&row.record, &["seedId"], "-"),
        "requiredLevel" => format!("Lv.{}", game_config_text(&row.record, &["requiredLevel"], "0")),
        "seasons" => {
            if game_config_number(&row.record, &["seasons"]).unwrap_or_default() == 2 {
                crate::i18n::text(language, "双季", "Two seasons").into()
            } else {
                crate::i18n::text(language, "单季", "Single season").into()
            }
        }
        "growTime" => format_game_config_grow_time(
            game_config_number(&row.record, &["growTime"]).unwrap_or_default(),
            language,
        ),
        "harvestCount" => game_config_text(&row.record, &["harvestCount"], "-"),
        "exp" => game_config_text(&row.record, &["exp"], "-"),
        _ => return None,
    })
}
