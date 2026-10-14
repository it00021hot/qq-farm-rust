use super::super::*;
use super::shared::{
    format_game_config_price, game_config_number, game_config_rarity_label, game_config_text,
};

pub(super) fn cell_value(
    page_raw: &serde_json::Value,
    row: &DisplayRow,
    column: &str,
    language: &str,
) -> Option<String> {
    Some(match column {
        "itemId" => game_config_text(&row.record, &["id"], "-"),
        "type" => item_type_label(
            page_raw,
            game_config_number(&row.record, &["type"]).unwrap_or_default(),
        ),
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

pub(super) fn item_type_label(page_raw: &serde_json::Value, item_type: i64) -> String {
    page_raw
        .get("itemTypes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find(|entry| entry.get("value").and_then(serde_json::Value::as_i64) == Some(item_type))
        .and_then(|entry| entry.get("label"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| item_type.to_string())
}
