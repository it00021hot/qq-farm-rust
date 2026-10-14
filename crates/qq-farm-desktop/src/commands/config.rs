//! 游戏配置列表与 overlay 增删改。

use serde_json::{json, Value};
use tauri::State;

use qq_farm_app::config::{self, GameConfigEntry, GameConfigImage, GameConfigKind};
use qq_farm_app::AppError;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn maybe_save_image(payload: &Value) -> Option<GameConfigImage> {
    let name = payload.get("imageName").and_then(|v| v.as_str())?;
    let b64 =
        payload.get("imageBase64").or_else(|| payload.get("image")).and_then(|v| v.as_str())?;
    if name.is_empty() || b64.is_empty() {
        return None;
    }
    Some(GameConfigImage { name: name.to_string(), base64: b64.to_string() })
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
    let config_kind = match kind.as_str() {
        "seed" | "plant" => GameConfigKind::Plant,
        "fruit" | "item" => GameConfigKind::Item,
        _ => return Err(IpcError::from(AppError::BadRequest(format!("unknown kind: {kind}")))),
    };
    let removed = config::delete_game_config(config_kind, id_num).map_err(IpcError::from)?;
    Ok(json!({ "ok": true, "removed": removed }))
}

fn mutate_config(kind: &str, id: Option<String>, payload: Value) -> IpcResult<Value> {
    let image = maybe_save_image(&payload);
    let id = id
        .map(|raw| {
            raw.parse::<i64>()
                .map_err(|_| IpcError::from(AppError::BadRequest("invalid id".into())))
        })
        .transpose()?;
    match kind {
        "seed" => {
            let mut write: config::SeedConfigWrite = serde_json::from_value(payload)
                .map_err(|error| IpcError::from(AppError::BadRequest(error.to_string())))?;
            if write.seed_id.is_none() {
                write.seed_id = id;
            }
            config::upsert_seed_config(write, image.as_ref()).map_err(IpcError::from)?;
        }
        "fruit" => {
            let mut write: config::FruitConfigWrite = serde_json::from_value(payload)
                .map_err(|error| IpcError::from(AppError::BadRequest(error.to_string())))?;
            if write.id.is_none() {
                write.id = id;
            }
            config::upsert_fruit_config(write, image.as_ref()).map_err(IpcError::from)?;
        }
        "item" => {
            let mut write: config::ItemConfigWrite = serde_json::from_value(payload)
                .map_err(|error| IpcError::from(AppError::BadRequest(error.to_string())))?;
            if write.id.is_none() {
                write.id = id;
            }
            config::upsert_item_config(write, image.as_ref()).map_err(IpcError::from)?;
        }
        "plant" => {
            // 保留旧 IPC 的原始 Plant payload 兼容路径。
            let payload: qq_farm_core::config::game_config::Plant = serde_json::from_value(payload)
                .map_err(|error| IpcError::from(AppError::BadRequest(error.to_string())))?;
            if payload.id <= 0 {
                return Err(IpcError::from(AppError::BadRequest("plant id required".into())));
            }
            config::upsert_game_config(GameConfigEntry::Plant(payload), image.as_ref())
                .map_err(IpcError::from)?;
        }
        _ => return Err(IpcError::from(AppError::BadRequest(format!("unknown kind: {kind}")))),
    }
    Ok(json!({ "ok": true }))
}
