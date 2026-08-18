//! Game config 路由 — 种子/果实/物品配置 CRUD 与图片。
//!
//! 1:1 对应原 `controllers/admin/farm-routes.ts` 中的 config 段。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::context::{ok, ok_data, AdminContext, ApiError, ApiResult};

/// 构造 game-config 路由
pub fn router() -> Router<Arc<AdminContext>> {
    Router::new()
        .route("/api/config/seeds", get(get_config_seeds))
        .route("/api/config/fruits", get(get_config_fruits))
        .route("/api/config/items", get(get_config_items))
        .route("/api/config/item-types", get(get_config_item_types))
        .route("/api/config/plants", get(get_config_plants))
        .route("/api/seed", post(post_seed))
        .route("/api/config/fruit", post(post_fruit))
        .route("/api/config/seed/{id}", put(put_seed).delete(delete_config_seed))
        .route("/api/config/fruit/{id}", put(put_fruit).delete(delete_config_fruit))
        .route("/api/config/item/{id}", put(put_item).delete(delete_config_item))
        .route("/api/config/images/{name}", get(get_config_image))
}

#[derive(Debug, Default, Deserialize)]
struct ConfigImageBody {
    #[serde(default)]
    image_base64: Option<String>,
    #[serde(default)]
    image_name: Option<String>,
}

fn maybe_save_image(body: &ConfigImageBody) -> Option<String> {
    let b64 = body.image_base64.as_deref()?.trim();
    if b64.is_empty() {
        return None;
    }
    let name = body
        .image_name
        .clone()
        .unwrap_or_else(|| format!("img-{}.png", chrono::Utc::now().timestamp_millis()));
    qq_farm_core::config::game_config::GameConfig::save_config_image_base64(&name, b64).ok()
}

async fn get_config_seeds(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<serde_json::Value> = gc
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
                obj.insert("priceId".into(), json!(sells.first().map(|p| p.0).unwrap_or(0)));
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
    ok_data(data)
}

async fn get_config_fruits(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<serde_json::Value> = gc
        .get_all_items()
        .into_iter()
        .filter(|i| i.item_type == 6)
        .map(|fruit| {
            let plant = gc.get_plant_by_fruit_id(fruit.id);
            let sells = fruit
                .sells
                .as_ref()
                .and_then(|v| v.as_str())
                .map(|s| gc.parse_sells(s))
                .unwrap_or_default();
            json!({
                "id": fruit.id,
                "name": fruit.name,
                "type": fruit.item_type,
                "price": sells.first().map(|p| p.1).unwrap_or(0),
                "priceId": sells.first().map(|p| p.0).unwrap_or(0),
                "level": fruit.level.unwrap_or(0),
                "assetName": fruit.asset_name.clone().unwrap_or_default(),
                "desc": fruit.desc.clone().unwrap_or_default(),
                "rarity": fruit.rarity.unwrap_or(0),
                "maxCount": fruit.max_count.unwrap_or(9999),
                "plantId": plant.as_ref().map(|p| p.id),
                "seedId": plant.as_ref().and_then(|p| p.seed_id),
                "plantName": plant.as_ref().map(|p| p.name.clone()),
                "image": qq_farm_core::config::game_config::mapped_item_image(fruit.id),
            })
        })
        .collect();
    ok_data(data)
}

async fn get_config_items(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<serde_json::Value> = gc
        .get_all_items()
        .into_iter()
        .filter(|i| i.item_type != 5 && i.item_type != 6)
        .map(|item| {
            let sells = item
                .sells
                .as_ref()
                .and_then(|v| v.as_str())
                .map(|s| gc.parse_sells(s))
                .unwrap_or_default();
            json!({
                "id": item.id,
                "type": item.item_type,
                "name": item.name,
                "interactionType": item.interaction_type.clone().unwrap_or_default(),
                "priceId": sells.first().map(|p| p.0).unwrap_or(0),
                "price": sells.first().map(|p| p.1).unwrap_or(0),
                "level": item.level.unwrap_or(0),
                "assetName": item.asset_name.clone().unwrap_or_default(),
                "iconRes": item.icon_res.clone().unwrap_or_default(),
                "maxCount": item.max_count.unwrap_or(9999),
                "canUse": item.can_use.unwrap_or(0),
                "desc": item.desc.clone().unwrap_or_default(),
                "rarity": item.rarity.unwrap_or(0),
                "image": qq_farm_core::config::game_config::mapped_item_image(item.id),
            })
        })
        .collect();
    ok_data(data)
}

async fn get_config_item_types(
    State(_ctx): State<Arc<AdminContext>>,
) -> ApiResult<serde_json::Value> {
    let cfg = qq_farm_core::config::game_config::load_item_types_config();
    ok(json!({ "ok": true, "itemTypes": cfg }))
}

async fn get_config_plants(State(_ctx): State<Arc<AdminContext>>) -> ApiResult<serde_json::Value> {
    let gc = qq_farm_core::config::game_config::global();
    let data: Vec<serde_json::Value> = gc
        .get_all_plants()
        .into_iter()
        .map(|p| {
            let seed_id = p.seed_id.unwrap_or(0);
            let land_level = if seed_id > 0 {
                gc.get_item_by_id(seed_id)
                    .and_then(|i| i.level)
                    .unwrap_or_else(|| p.land_level_need.unwrap_or(0))
            } else {
                p.land_level_need.unwrap_or(0)
            };
            json!({
                "plantId": p.id,
                "id": p.id,
                "name": p.name,
                "plantName": p.name,
                "seedId": p.seed_id,
                "fruitId": p.fruit.as_ref().map(|f| f.id),
                "fruitCount": p.fruit.as_ref().map(|f| f.count).unwrap_or(0),
                "landLevelNeed": land_level,
                "seasons": p.seasons.unwrap_or(1),
                "growPhases": p.grow_phases.clone().unwrap_or_default(),
                "exp": p.exp.unwrap_or(0),
                "price": if seed_id > 0 { gc.get_seed_price(seed_id) } else { 0 },
                "image": if seed_id > 0 {
                    qq_farm_core::config::game_config::mapped_item_image(seed_id)
                } else {
                    String::new()
                },
            })
        })
        .collect();
    ok_data(data)
}

async fn post_seed(
    State(_ctx): State<Arc<AdminContext>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    let mut plant: qq_farm_core::config::game_config::Plant =
        serde_json::from_value(body.clone()).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    if plant.id <= 0 {
        return Err(ApiError::BadRequest("plant id required".into()));
    }
    let img_body: ConfigImageBody = serde_json::from_value(body).unwrap_or_default();
    if let Some(url) = maybe_save_image(&img_body) {
        plant.harvest_animation = Some(url);
    }
    qq_farm_core::config::game_config::global()
        .upsert_plant(plant)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true }))
}

async fn put_seed(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
    Json(mut body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    body["id"] = json!(id);
    if body.get("seed_id").is_none() {
        body["seed_id"] = json!(id);
    }
    post_seed(State(_ctx), Json(body)).await
}

async fn delete_config_seed(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let removed = qq_farm_core::config::game_config::global()
        .delete_plant_overlay(id)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true, "removed": removed }))
}

async fn post_fruit(
    State(ctx): State<Arc<AdminContext>>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    put_fruit(State(ctx), Path(id), Json(body)).await
}

async fn put_fruit(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
    Json(mut body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    body["id"] = json!(id);
    let mut item: qq_farm_core::config::game_config::Item =
        serde_json::from_value(body.clone()).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let img_body: ConfigImageBody = serde_json::from_value(body).unwrap_or_default();
    if let Some(url) = maybe_save_image(&img_body) {
        item.asset_name = Some(url);
    }
    qq_farm_core::config::game_config::global()
        .upsert_item(item)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true }))
}

async fn delete_config_fruit(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    delete_config_item(State(_ctx), Path(id)).await
}

async fn put_item(
    State(ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    put_fruit(State(ctx), Path(id), Json(body)).await
}

async fn delete_config_item(
    State(_ctx): State<Arc<AdminContext>>,
    Path(id): Path<i64>,
) -> ApiResult<serde_json::Value> {
    let removed = qq_farm_core::config::game_config::global()
        .delete_item_overlay(id)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    ok(json!({ "ok": true, "removed": removed }))
}

async fn get_config_image(Path(name): Path<String>) -> impl IntoResponse {
    match qq_farm_core::config::game_config::GameConfig::read_config_image(&name) {
        Some(bytes) => {
            let mime = if name.ends_with(".jpg") || name.ends_with(".jpeg") {
                "image/jpeg"
            } else if name.ends_with(".gif") {
                "image/gif"
            } else if name.ends_with(".webp") {
                "image/webp"
            } else {
                "image/png"
            };
            ([(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
