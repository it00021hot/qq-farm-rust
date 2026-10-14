use super::super::*;
use super::shared::{
    format_game_config_price, game_config_number, game_config_rarity_label, game_config_text,
};

pub(super) fn cell_value(row: &DisplayRow, column: &str, language: &str) -> Option<String> {
    Some(match column {
        "fruitId" => game_config_text(&row.record, &["id"], "-"),
        "plantName" => game_config_text(&row.record, &["plantName"], "-"),
        "rarity" => game_config_rarity_label(
            game_config_number(&row.record, &["rarity"]).unwrap_or_default(),
            language,
        ),
        "price" => format_game_config_price(
            game_config_number(&row.record, &["price"]).unwrap_or_default(),
            game_config_number(&row.record, &["priceId"]).unwrap_or_default(),
            language,
        ),
        _ => return None,
    })
}
