//! 游戏配置门面（种子 / 果实 / 道具）。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

/// 游戏配置中可以由用户编辑的条目类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameConfigKind {
    Plant,
    Item,
}

/// 一条需要写入 overlay 的游戏配置。
#[derive(Debug, Clone)]
pub enum GameConfigEntry {
    Plant(qq_farm_core::config::game_config::Plant),
    Item(qq_farm_core::config::game_config::Item),
}

/// 游戏配置条目的图片上传内容。
#[derive(Debug, Clone)]
pub struct GameConfigImage {
    pub name: String,
    pub base64: String,
}

/// Vue/Tauri 与 GPUI 共用的种子编辑表单字段。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedConfigWrite {
    pub seed_id: Option<i64>,
    pub name: Option<String>,
    pub grow_phases: Option<String>,
    pub land_level_need: Option<i64>,
    pub seasons: Option<i64>,
    pub fruit_count: Option<i64>,
    pub price: Option<i64>,
    pub price_id: Option<i64>,
    pub exp: Option<i64>,
    pub size: Option<i64>,
}

/// Vue/Tauri 与 GPUI 共用的果实编辑表单字段。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FruitConfigWrite {
    pub id: Option<i64>,
    pub plant_id: Option<i64>,
    pub name: Option<String>,
    pub price: Option<i64>,
    pub price_id: Option<i64>,
    pub desc: Option<String>,
    pub effect_desc: Option<String>,
    pub rarity: Option<i64>,
    pub max_count: Option<i64>,
    pub level: Option<i64>,
    pub fruit_count: Option<i64>,
    pub asset_name: Option<String>,
}

/// Vue/Tauri 与 GPUI 共用的道具编辑表单字段。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemConfigWrite {
    pub id: Option<i64>,
    #[serde(rename = "type")]
    pub item_type: Option<i64>,
    pub name: Option<String>,
    pub price: Option<i64>,
    pub price_id: Option<i64>,
    pub interaction_type: Option<String>,
    pub can_use: Option<i64>,
    pub desc: Option<String>,
    pub effect_desc: Option<String>,
    pub rarity: Option<i64>,
    pub max_count: Option<i64>,
    pub level: Option<i64>,
    pub asset_name: Option<String>,
}

/// 写入一条游戏配置及其可选图片，并返回最终写入的条目。
pub fn upsert_game_config(
    mut entry: GameConfigEntry,
    image: Option<&GameConfigImage>,
) -> AppResult<GameConfigEntry> {
    let image_path = image.map(|image| save_image(&image.name, &image.base64)).transpose()?;
    match &mut entry {
        GameConfigEntry::Plant(plant) => {
            if plant.id <= 0 {
                return Err(AppError::BadRequest("plant id required".into()));
            }
            if let Some(path) = image_path {
                plant.harvest_animation = Some(path);
            }
            qq_farm_core::config::game_config::global()
                .upsert_plant(plant.clone())
                .map_err(|error| AppError::Internal(error.to_string()))?;
        }
        GameConfigEntry::Item(item) => {
            if item.id <= 0 {
                return Err(AppError::BadRequest("item id required".into()));
            }
            if let Some(path) = image_path {
                item.asset_name = Some(path);
            }
            qq_farm_core::config::game_config::global()
                .upsert_item(item.clone())
                .map_err(|error| AppError::Internal(error.to_string()))?;
        }
    }
    Ok(entry)
}

/// 保存种子表单。一个种子由 Plant 配置和 type=5 的 Item 售价配置组成。
pub fn upsert_seed_config(
    write: SeedConfigWrite,
    image: Option<&GameConfigImage>,
) -> AppResult<()> {
    let seed_id = write
        .seed_id
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::BadRequest("seed id must be greater than zero".into()))?;
    if write.name.as_deref().is_some_and(|name| name.trim().is_empty()) {
        return Err(AppError::BadRequest("seed name cannot be empty".into()));
    }

    let gc = qq_farm_core::config::game_config::global();
    let mut plant =
        gc.get_plant_by_seed_id(seed_id).or_else(|| gc.get_plant_by_id(seed_id)).unwrap_or_else(
            || qq_farm_core::config::game_config::Plant { id: seed_id, ..Default::default() },
        );
    plant.seed_id = Some(seed_id);
    if let Some(name) = write.name.as_ref() {
        plant.name = name.clone();
    }
    if let Some(value) = write.grow_phases {
        plant.grow_phases = Some(value);
    }
    if let Some(value) = write.land_level_need {
        plant.land_level_need = Some(value);
    }
    if let Some(value) = write.seasons {
        plant.seasons = Some(value);
    }
    if let Some(value) = write.exp {
        plant.exp = Some(value);
    }
    if let Some(value) = write.size {
        plant.size = Some(value);
    }
    if let Some(value) = write.fruit_count {
        if let Some(fruit) = plant.fruit.as_mut() {
            fruit.count = value;
        }
    }
    upsert_game_config(GameConfigEntry::Plant(plant), image)?;

    let mut seed_item = gc.get_item_by_id(seed_id).unwrap_or_else(|| {
        qq_farm_core::config::game_config::Item { id: seed_id, item_type: 5, ..Default::default() }
    });
    seed_item.item_type = 5;
    if let Some(name) = write.name {
        seed_item.name = name;
    }
    apply_sell_price(&mut seed_item, write.price, write.price_id);
    upsert_game_config(GameConfigEntry::Item(seed_item), None)?;
    Ok(())
}

/// 保存果实表单，并同步所选植物上的果实编号与产量。
pub fn upsert_fruit_config(
    write: FruitConfigWrite,
    image: Option<&GameConfigImage>,
) -> AppResult<()> {
    if write.name.as_deref().is_some_and(|name| name.trim().is_empty()) {
        return Err(AppError::BadRequest("fruit name cannot be empty".into()));
    }
    let gc = qq_farm_core::config::game_config::global();
    let mut plant = write
        .plant_id
        .and_then(|plant_id| gc.get_plant_by_id(plant_id))
        .or_else(|| write.id.and_then(|fruit_id| gc.get_plant_by_fruit_id(fruit_id)))
        .ok_or_else(|| AppError::BadRequest("a valid plant is required".into()))?;
    let id = write
        .id
        .or_else(|| plant.fruit.as_ref().map(|fruit| fruit.id))
        .filter(|id| *id > 0)
        .ok_or_else(|| {
        AppError::BadRequest("fruit id is missing from the selected plant".into())
    })?;
    let fruit_count = write.fruit_count.or_else(|| plant.fruit.as_ref().map(|fruit| fruit.count));
    if let Some(count) = fruit_count {
        plant.fruit = Some(qq_farm_core::config::game_config::PlantFruit { id, count });
    } else if plant.fruit.is_none() {
        plant.fruit = Some(qq_farm_core::config::game_config::PlantFruit { id, count: 0 });
    }

    let mut item = gc.get_item_by_id(id).unwrap_or_else(|| {
        qq_farm_core::config::game_config::Item { id, item_type: 6, ..Default::default() }
    });
    item.item_type = 6;
    if let Some(name) = write.name {
        item.name = name;
    }
    if let Some(value) = write.desc {
        item.desc = Some(value);
    }
    if let Some(value) = write.effect_desc {
        item.effect_desc = Some(value);
    }
    if let Some(value) = write.rarity {
        item.rarity = Some(value);
    }
    if let Some(value) = write.max_count {
        item.max_count = Some(value);
    }
    if let Some(value) = write.level {
        item.level = Some(value);
    }
    if let Some(value) = write.asset_name {
        item.asset_name = Some(value);
    }
    apply_sell_price(&mut item, write.price, write.price_id);

    upsert_game_config(GameConfigEntry::Item(item), image)?;
    upsert_game_config(GameConfigEntry::Plant(plant), None)?;
    Ok(())
}

/// 保存普通道具表单。
pub fn upsert_item_config(
    write: ItemConfigWrite,
    image: Option<&GameConfigImage>,
) -> AppResult<()> {
    let id = write
        .id
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::BadRequest("item id must be greater than zero".into()))?;
    if write.name.as_deref().is_some_and(|name| name.trim().is_empty()) {
        return Err(AppError::BadRequest("item name cannot be empty".into()));
    }
    let gc = qq_farm_core::config::game_config::global();
    let mut item =
        gc.get_item_by_id(id).unwrap_or_else(|| qq_farm_core::config::game_config::Item {
            id,
            item_type: write.item_type.unwrap_or(7),
            ..Default::default()
        });
    if let Some(value) = write.item_type {
        item.item_type = value;
    }
    if let Some(value) = write.name {
        item.name = value;
    }
    if let Some(value) = write.interaction_type {
        item.interaction_type = Some(value);
    }
    if let Some(value) = write.can_use {
        item.can_use = Some(value);
    }
    if let Some(value) = write.desc {
        item.desc = Some(value);
    }
    if let Some(value) = write.effect_desc {
        item.effect_desc = Some(value);
    }
    if let Some(value) = write.rarity {
        item.rarity = Some(value);
    }
    if let Some(value) = write.max_count {
        item.max_count = Some(value);
    }
    if let Some(value) = write.level {
        item.level = Some(value);
    }
    if let Some(value) = write.asset_name {
        item.asset_name = Some(value);
    }
    apply_sell_price(&mut item, write.price, write.price_id);
    upsert_game_config(GameConfigEntry::Item(item), image)?;
    Ok(())
}

fn apply_sell_price(
    item: &mut qq_farm_core::config::game_config::Item,
    price: Option<i64>,
    price_id: Option<i64>,
) {
    let Some(price) = price else { return };
    if price <= 0 {
        item.sells = None;
    } else {
        item.sells = Some(json!(format!("{}:{price}", price_id.unwrap_or(0))));
    }
}

/// 删除一条自定义配置 overlay，返回该条目是否存在。
pub fn delete_game_config(kind: GameConfigKind, id: i64) -> AppResult<bool> {
    let gc = qq_farm_core::config::game_config::global();
    match kind {
        GameConfigKind::Plant => {
            gc.delete_plant_overlay(id).map_err(|error| AppError::Internal(error.to_string()))
        }
        GameConfigKind::Item => {
            gc.delete_item_overlay(id).map_err(|error| AppError::Internal(error.to_string()))
        }
    }
}

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
                obj.insert("priceId".into(), json!(sells.first().map_or(0, |p| p.0)));
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

fn first_sell_pair(
    gc: &qq_farm_core::config::game_config::GameConfig,
    item: &qq_farm_core::config::game_config::Item,
    fallback_cond: bool,
) -> (i64, i64) {
    let mut sells = gc.parse_sells_value(item.sells.as_ref());
    if sells.is_empty() && fallback_cond {
        sells = gc.parse_sells_value(item.cond_sells.as_ref());
    }
    sells.first().copied().unwrap_or((0, 0))
}

/// `果实列表（item_type` == 6）。
#[must_use]
pub fn list_fruits() -> Value {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<Value> = gc
        .get_all_items()
        .into_iter()
        .filter(|i| i.item_type == 6)
        .map(|fruit| {
            let plant = gc.get_plant_by_fruit_id(fruit.id);
            let (price_id, price) = first_sell_pair(gc, &fruit, true);
            json!({
                "id": fruit.id,
                "name": fruit.name,
                "type": fruit.item_type,
                "price": price,
                "priceId": price_id,
                "level": fruit.level.unwrap_or(0),
                "assetName": fruit.asset_name.clone().unwrap_or_default(),
                "desc": fruit.desc.clone().unwrap_or_default(),
                "effectDesc": fruit.effect_desc.clone().unwrap_or_default(),
                "rarity": fruit.rarity.unwrap_or(0),
                "maxCount": fruit.max_count.unwrap_or(9999),
                "fruitCount": plant.as_ref().and_then(|p| p.fruit.as_ref()).map_or(200, |f| f.count),
                "plantId": plant.as_ref().map(|p| p.id),
                "seedId": plant.as_ref().and_then(|p| p.seed_id),
                "plantName": plant.as_ref().map(|p| p.name.clone()),
                "image": qq_farm_core::config::game_config::mapped_item_image(fruit.id),
            })
        })
        .collect();
    json!(data)
}

/// 道具列表（排除种子 type=5、果实 type=6）。
#[must_use]
pub fn list_items() -> Value {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<Value> = gc
        .get_all_items()
        .into_iter()
        .filter(|i| i.item_type != 5 && i.item_type != 6)
        .map(|item| {
            let (price_id, price) = first_sell_pair(gc, &item, false);
            json!({
                "id": item.id,
                "type": item.item_type,
                "name": item.name,
                "interactionType": item.interaction_type.clone().unwrap_or_default(),
                "priceId": price_id,
                "price": price,
                "level": item.level.unwrap_or(0),
                "assetName": item.asset_name.clone().unwrap_or_default(),
                "iconRes": item.icon_res.clone().unwrap_or_default(),
                "maxCount": item.max_count.unwrap_or(9999),
                "canUse": item.can_use.unwrap_or(0),
                "desc": item.desc.clone().unwrap_or_default(),
                "effectDesc": item.effect_desc.clone().unwrap_or_default(),
                "rarity": item.rarity.unwrap_or(0),
                "image": qq_farm_core::config::game_config::mapped_item_image(item.id),
            })
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

/// 道具类型（对齐 Go `logic.ItemTypes` 面板选项）。
#[must_use]
pub fn item_types() -> Value {
    json!([
        { "value": 1, "label": "特殊道具" },
        { "value": 2, "label": "货币" },
        { "value": 3, "label": "经验" },
        { "value": 4, "label": "农场工具" },
        { "value": 7, "label": "化肥" },
        { "value": 8, "label": "宠物" },
        { "value": 9, "label": "宠物食品" },
        { "value": 10, "label": "头像框" },
        { "value": 11, "label": "礼品盒" },
        { "value": 12, "label": "收藏点" },
        { "value": 13, "label": "活跃点" },
        { "value": 14, "label": "解锁卡" },
        { "value": 15, "label": "高级货币" },
        { "value": 16, "label": "自选礼包" },
        { "value": 17, "label": "变异果实" },
        { "value": 18, "label": "皮肤/装饰" },
        { "value": 23, "label": "虫虫道具" },
    ])
}

/// 保存配置图片（base64）→ 返回文件名。
pub fn save_image(name: &str, base64: &str) -> AppResult<String> {
    qq_farm_core::config::game_config::GameConfig::save_config_image_base64(name, base64)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Read an image previously uploaded through the local game configuration API.
pub fn read_image(name: &str) -> Option<Vec<u8>> {
    qq_farm_core::config::game_config::GameConfig::read_config_image(name)
}
