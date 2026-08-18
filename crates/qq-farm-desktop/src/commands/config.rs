//! 游戏配置列表与 overlay 增删改。

use serde_json::{json, Value};
use tauri::State;

use qq_farm_app::config;
use qq_farm_app::AppError;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn maybe_save_image(payload: &Value) -> Option<String> {
    let name = payload.get("imageName").and_then(|v| v.as_str())?;
    let b64 =
        payload.get("imageBase64").or_else(|| payload.get("image")).and_then(|v| v.as_str())?;
    if name.is_empty() || b64.is_empty() {
        return None;
    }
    config::save_image(name, b64).ok()
}

/// 种子列表。
#[tauri::command]
pub fn config_list_seeds(state: State<'_, DesktopState>) -> IpcResult<Value> {
    let _ = &state.acl;
    Ok(config::list_seeds())
}

/// 果实列表。
#[tauri::command]
pub fn config_list_fruits(state: State<'_, DesktopState>) -> IpcResult<Value> {
    let _ = &state.acl;
    Ok(config::list_fruits())
}

/// 道具列表。
#[tauri::command]
pub fn config_list_items(state: State<'_, DesktopState>) -> IpcResult<Value> {
    let _ = &state.acl;
    Ok(config::list_items())
}

/// 植物列表。
#[tauri::command]
pub fn config_list_plants(state: State<'_, DesktopState>) -> IpcResult<Value> {
    let _ = &state.acl;
    Ok(config::list_plants())
}

/// 道具类型。
#[tauri::command]
pub fn config_list_item_types(state: State<'_, DesktopState>) -> IpcResult<Value> {
    let _ = &state.acl;
    Ok(config::item_types())
}

/// 新增配置项（seed→plant overlay；fruit/item→item overlay）。
#[tauri::command]
pub fn config_add(
    state: State<'_, DesktopState>,
    kind: String,
    payload: Value,
) -> IpcResult<Value> {
    let _ = &state.acl;
    mutate_config(&kind, None, payload)
}

/// 修改配置项。
#[tauri::command]
pub fn config_modify(
    state: State<'_, DesktopState>,
    kind: String,
    id: String,
    payload: Value,
) -> IpcResult<Value> {
    let _ = &state.acl;
    mutate_config(&kind, Some(id), payload)
}

/// 删除配置项（仅 overlay）。
#[tauri::command]
pub fn config_delete(state: State<'_, DesktopState>, kind: String, id: String) -> IpcResult<Value> {
    let _ = &state.acl;
    let id_num: i64 =
        id.parse().map_err(|_| IpcError::from(AppError::BadRequest("invalid id".into())))?;
    let gc = qq_farm_core::config::game_config::global();
    let removed = match kind.as_str() {
        "seed" | "plant" => gc
            .delete_plant_overlay(id_num)
            .map_err(|e| IpcError::from(AppError::Internal(e.to_string())))?,
        "fruit" | "item" => gc
            .delete_item_overlay(id_num)
            .map_err(|e| IpcError::from(AppError::Internal(e.to_string())))?,
        _ => return Err(IpcError::from(AppError::BadRequest(format!("unknown kind: {kind}")))),
    };
    Ok(json!({ "ok": true, "removed": removed }))
}

fn mutate_config(kind: &str, id: Option<String>, mut payload: Value) -> IpcResult<Value> {
    // UI 多为 camelCase；Plant/Item 反序列化为 snake_case。
    if let Some(obj) = payload.as_object_mut() {
        const RENAMES: &[(&str, &str)] = &[
            ("seedId", "seed_id"),
            ("landLevelNeed", "land_level_need"),
            ("growPhases", "grow_phases"),
            ("expRoot", "exp_root"),
            ("expAlter", "exp_alter"),
            ("fruitRoot", "fruit_root"),
            ("fruitAlter", "fruit_alter"),
            ("harvestAnimation", "harvest_animation"),
            ("matureEffect", "mature_effect"),
            ("specialFruit", "special_fruit"),
            ("itemType", "type"),
            ("interactionType", "interaction_type"),
            ("assetName", "asset_name"),
            ("iconRes", "icon_res"),
            ("maxCount", "max_count"),
            ("canUse", "can_use"),
            ("targetId", "target_id"),
        ];
        for (from, to) in RENAMES {
            if let Some(v) = obj.remove(*from) {
                obj.entry((*to).to_string()).or_insert(v);
            }
        }
    }
    if let Some(id) = id {
        if let Ok(n) = id.parse::<i64>() {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("id".into(), json!(n));
                if kind == "seed" && !obj.contains_key("seed_id") {
                    obj.insert("seed_id".into(), json!(n));
                }
            }
        }
    }
    let img = maybe_save_image(&payload);
    let gc = qq_farm_core::config::game_config::global();
    match kind {
        "seed" | "plant" => {
            let mut plant: qq_farm_core::config::game_config::Plant =
                serde_json::from_value(payload)
                    .map_err(|e| IpcError::from(AppError::BadRequest(e.to_string())))?;
            if plant.id <= 0 {
                return Err(IpcError::from(AppError::BadRequest("plant id required".into())));
            }
            if let Some(url) = img {
                plant.harvest_animation = Some(url);
            }
            gc.upsert_plant(plant)
                .map_err(|e| IpcError::from(AppError::Internal(e.to_string())))?;
        }
        "fruit" | "item" => {
            let mut item: qq_farm_core::config::game_config::Item = serde_json::from_value(payload)
                .map_err(|e| IpcError::from(AppError::BadRequest(e.to_string())))?;
            if item.id <= 0 {
                return Err(IpcError::from(AppError::BadRequest("item id required".into())));
            }
            if let Some(url) = img {
                item.asset_name = Some(url);
            }
            gc.upsert_item(item).map_err(|e| IpcError::from(AppError::Internal(e.to_string())))?;
        }
        _ => return Err(IpcError::from(AppError::BadRequest(format!("unknown kind: {kind}")))),
    }
    Ok(json!({ "ok": true }))
}
