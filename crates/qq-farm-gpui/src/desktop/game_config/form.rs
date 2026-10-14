use super::super::*;

pub(crate) const EXTRA_FIELDS: &[(&str, &str)] = &[
    ("growPhases", "生长阶段"),
    ("seasons", "季节"),
    ("fruitCount", "收获数"),
    ("price", "价格"),
    ("priceId", "货币"),
    ("exp", "经验"),
    ("size", "占地"),
    ("rarity", "稀有度"),
    ("maxCount", "堆叠上限"),
    ("interactionType", "交互类型"),
    ("canUse", "可使用"),
    ("effectDesc", "效果描述"),
];

pub(super) fn set_field(
    fields: &HashMap<String, Entity<InputState>>,
    key: &str,
    value: &str,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(state) = fields.get(key) {
        state.update(cx, |state, cx| state.set_value(value, window, cx));
    }
}

pub(super) fn set_textarea_field(
    fields: &HashMap<String, Entity<TextareaState>>,
    key: &str,
    value: &str,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(state) = fields.get(key) {
        state.update(cx, |state, cx| state.set_value(value, window, cx));
    }
}

pub(super) fn read_extra_field(
    fields: &HashMap<String, Entity<InputState>>,
    textareas: &HashMap<String, Entity<TextareaState>>,
    key: &str,
    cx: &App,
) -> String {
    fields
        .get(key)
        .map(|state| state.read(cx).value().to_string())
        .or_else(|| textareas.get(key).map(|state| state.read(cx).value().to_string()))
        .unwrap_or_default()
}

pub(super) fn set_extra_field(
    fields: &HashMap<String, Entity<InputState>>,
    textareas: &HashMap<String, Entity<TextareaState>>,
    key: &str,
    value: &str,
    window: &mut Window,
    cx: &mut App,
) {
    if fields.contains_key(key) {
        set_field(fields, key, value, window, cx);
    } else {
        set_textarea_field(textareas, key, value, window, cx);
    }
}

pub(super) fn read_image(
    path: &str,
    language: &str,
) -> Result<Option<qq_farm_app::config::GameConfigImage>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Ok(None);
    }
    let supported = ["png", "jpg", "jpeg", "webp"]
        .iter()
        .any(|extension| path.to_lowercase().ends_with(&format!(".{extension}")));
    if !supported {
        return Err(crate::i18n::text(
            language,
            "图片格式仅支持 PNG、JPG 和 WebP。",
            "Supported image formats are PNG, JPG, and WebP.",
        )
        .into());
    }
    let bytes = std::fs::read(path).map_err(|error| {
        if language == "en-US" {
            format!("Could not read image: {error}")
        } else {
            format!("读取图片失败：{error}")
        }
    })?;
    use base64::Engine as _;
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("config-image.png")
        .to_string();
    Ok(Some(qq_farm_app::config::GameConfigImage {
        name,
        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    }))
}

pub(super) fn plant_id(plant: &serde_json::Value) -> Option<i64> {
    plant
        .get("plantId")
        .or_else(|| plant.get("id"))
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.trim().parse().ok()))
}

pub(super) fn plant_seed_id(plant: &serde_json::Value) -> Option<i64> {
    plant
        .get("seedId")
        .or_else(|| plant.get("seed_id"))
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.trim().parse().ok()))
}

pub(super) fn plant_option_label(
    name: &str,
    plant_id: i64,
    seed_id: Option<i64>,
    language: &str,
) -> String {
    let seed_id = seed_id.map_or_else(|| "-".to_string(), |id| id.to_string());
    if language == "en-US" {
        format!("{name} (Seed {seed_id} / Plant {plant_id})")
    } else {
        format!("{name} (种子{seed_id} / 植物{plant_id})")
    }
}

#[cfg(test)]
mod tests {
    use super::{plant_id, plant_option_label, plant_seed_id};
    use serde_json::json;

    #[test]
    fn plant_option_data_matches_vue_plant_labels_and_ids() {
        let plant = json!({
            "plantId": 1001,
            "seed_id": 2001,
            "name": "草莓"
        });
        assert_eq!(plant_id(&plant), Some(1001));
        assert_eq!(plant_seed_id(&plant), Some(2001));
        assert_eq!(
            plant_option_label("草莓", 1001, Some(2001), "zh-CN"),
            "草莓 (种子2001 / 植物1001)"
        );
        assert_eq!(
            plant_option_label("Strawberry", 1001, Some(2001), "en-US"),
            "Strawberry (Seed 2001 / Plant 1001)"
        );
    }
}
