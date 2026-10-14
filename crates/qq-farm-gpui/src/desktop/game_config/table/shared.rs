use super::super::columns::game_config_column_width;
use super::super::columns::{GameConfigColumnPin, GameConfigColumnSetting};
use super::super::*;

pub(super) fn configured_columns(
    settings: &[GameConfigColumnSetting],
    pin: GameConfigColumnPin,
) -> Vec<(&'static str, f32)> {
    settings
        .iter()
        .filter(|column| column.checked && column.pin == pin)
        .filter_map(|column| Some((column.key, game_config_column_width(column.key)?)))
        .collect()
}

pub(super) fn column_width(columns: &[(&str, f32)]) -> f32 {
    columns.iter().map(|(_, width)| *width).sum()
}

/// Column cell sizing for one lane. Pinned lanes keep the column's design
/// width; the center lane grows every column proportionally so the table
/// fills the panel instead of leaving a dead strip before the pinned lane.
pub(super) fn game_config_frame(width: f32, stretch: bool) -> Div {
    if stretch {
        div().flex_basis(px(width)).flex_grow(width).flex_shrink(1.).min_w(px(width))
    } else {
        div().w(px(width)).flex_none()
    }
}

pub(super) fn game_config_cell(label: &str, width: f32, stretch: bool) -> Div {
    game_config_frame(width, stretch)
        .h_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .text_center()
        .child(label.to_string())
}

pub(super) fn game_config_row_id(row: &DisplayRow) -> Option<i64> {
    if row.section == "seeds" {
        row.record
            .get("seedId")
            .or_else(|| row.record.get("id"))
            .and_then(serde_json::Value::as_i64)
    } else {
        row.record.get("id").and_then(serde_json::Value::as_i64)
    }
}

pub(super) fn game_config_text(
    record: &serde_json::Value,
    keys: &[&str],
    fallback: &str,
) -> String {
    keys.iter()
        .find_map(|key| record.get(*key))
        .filter(|value| !value.is_null())
        .map(value_to_action_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn game_config_number(record: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        record
            .get(*key)
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.trim().parse::<i64>().ok()))
    })
}

pub(super) fn game_config_rarity_label(rarity: i64, language: &str) -> String {
    let (chinese, english) = match rarity {
        0 => ("普通", "Common"),
        1 => ("优秀", "Uncommon"),
        2 => ("精良", "Fine"),
        3 => ("稀有", "Rare"),
        4 => ("史诗", "Epic"),
        5 => ("传说", "Legendary"),
        _ => return rarity.to_string(),
    };
    crate::i18n::text(language, chinese, english).into()
}

pub(super) fn format_game_config_price(price: i64, price_id: i64, language: &str) -> String {
    let currency = match price_id {
        qq_farm_core::constants::GOLD_BEAN_ITEM_ID => {
            crate::i18n::text(language, "金豆", "Gold beans")
        }
        1004 => crate::i18n::text(language, "钻石", "Diamonds"),
        _ => crate::i18n::text(language, "金币", "Coins"),
    };
    format!("{price} {currency}")
}

pub(super) fn format_game_config_grow_time(seconds: i64, language: &str) -> String {
    if seconds <= 0 {
        return "-".into();
    }
    if language == "en-US" {
        if seconds < 60 {
            return format!("{seconds}s");
        }
        if seconds < 3600 {
            return format!("{}m", seconds / 60);
        }
        let hours = seconds / 3600;
        let minutes = seconds % 3600 / 60;
        return if minutes > 0 { format!("{hours}h {minutes}m") } else { format!("{hours}h") };
    }
    if seconds < 60 {
        format!("{seconds}秒")
    } else if seconds < 3600 {
        format!("{}分", seconds / 60)
    } else {
        let hours = seconds / 3600;
        let minutes = seconds % 3600 / 60;
        if minutes > 0 {
            format!("{hours}时{minutes}分")
        } else {
            format!("{hours}时")
        }
    }
}
