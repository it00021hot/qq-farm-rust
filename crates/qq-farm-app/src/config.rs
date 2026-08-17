//! 游戏配置门面（种子 / 果实 / 道具）。

use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

/// 种子列表。
#[must_use]
pub fn list_seeds() -> Value {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<Value> = gc
        .get_all_seeds()
        .into_iter()
        .map(|s| {
            let item = gc.get_item_by_id(s.seed_id);
            let sells = item
                .as_ref()
                .and_then(|i| i.sells.as_ref())
                .and_then(|v| v.as_str())
                .map(|s| gc.parse_sells(s))
                .unwrap_or_default();
            let mut val = serde_json::to_value(&s).unwrap_or(json!({}));
            if let Some(obj) = val.as_object_mut() {
                obj.insert(
                    "priceId".into(),
                    json!(sells.first().map(|p| p.0).unwrap_or(0)),
                );
                if !obj.contains_key("image") {
                    obj.insert(
                        "image".into(),
                        json!(qq_farm_core::config::game_config::mapped_item_image(s.seed_id)),
                    );
                }
            }
            val
        })
        .collect();
    json!(data)
}

/// 果实列表（item_type == 6）。
#[must_use]
pub fn list_fruits() -> Value {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<Value> = gc
        .get_all_items()
        .into_iter()
        .filter(|i| i.item_type == 6)
        .map(|fruit| {
            let mut val = serde_json::to_value(&fruit).unwrap_or(json!({}));
            if let Some(obj) = val.as_object_mut() {
                if !obj.contains_key("image") {
                    obj.insert(
                        "image".into(),
                        json!(qq_farm_core::config::game_config::mapped_item_image(fruit.id)),
                    );
                }
            }
            val
        })
        .collect();
    json!(data)
}

/// 道具列表。
#[must_use]
pub fn list_items() -> Value {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<Value> = gc
        .get_all_items()
        .into_iter()
        .map(|item| {
            let mut val = serde_json::to_value(&item).unwrap_or(json!({}));
            if let Some(obj) = val.as_object_mut() {
                if !obj.contains_key("image") {
                    obj.insert(
                        "image".into(),
                        json!(qq_farm_core::config::game_config::mapped_item_image(item.id)),
                    );
                }
            }
            val
        })
        .collect();
    json!(data)
}

/// 植物列表。
#[must_use]
pub fn list_plants() -> Value {
    let gc = qq_farm_core::config::game_config::global();
    json!(gc.get_all_plants())
}

/// 道具类型。
#[must_use]
pub fn item_types() -> Value {
    json!(qq_farm_core::config::game_config::load_item_types_config())
}

/// 保存配置图片（base64）→ 返回文件名。
pub fn save_image(name: &str, base64: &str) -> AppResult<String> {
    qq_farm_core::config::game_config::GameConfig::save_config_image_base64(name, base64)
        .map_err(|e| AppError::Internal(e.to_string()))
}
