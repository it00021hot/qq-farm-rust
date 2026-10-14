use std::collections::HashMap;

use serde_json::{json, Value};

pub(in crate::desktop::activities::actions) fn prepare(
    page_raw: &Value,
    selected_ingredients: &HashMap<String, i64>,
    language: &str,
) -> Result<Value, String> {
    let selected = ingredients(page_raw)
        .iter()
        .filter_map(|item| {
            let uid = item.get("uid").map(crate::desktop::value_to_action_string)?;
            if uid.is_empty() {
                return None;
            }
            let count = selected_ingredients.get(&uid)?;
            Some(json!({ "uid": uid, "count": count }))
        })
        .collect::<Vec<_>>();

    if selected.is_empty() {
        return Err(crate::i18n::text(
            language,
            "请先选择至少一项酿造原料。",
            "Select at least one brewing ingredient.",
        )
        .into());
    }

    for item in &selected {
        let uid = item.get("uid").map(crate::desktop::value_to_action_string).unwrap_or_default();
        let count = item.get("count").and_then(Value::as_i64).unwrap_or_default();
        let stock = ingredients(page_raw)
            .iter()
            .find(|candidate| {
                candidate.get("uid").is_some_and(|candidate_uid| {
                    crate::desktop::value_to_action_string(candidate_uid).as_str() == uid.as_str()
                })
            })
            .and_then(|candidate| candidate.get("count"))
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
            .unwrap_or_default();
        if uid.is_empty() || count <= 0 || count > stock {
            return Err(if language == "en-US" {
                format!("Ingredient {uid} has an invalid quantity or exceeds current stock.")
            } else {
                format!("酿造配方中的 uid「{uid}」或数量无效，请检查当前库存。")
            });
        }
    }

    Ok(json!({ "ingredients": selected }))
}

fn ingredients(page_raw: &Value) -> &[Value] {
    page_raw
        .pointer("/qingMei/ingredients")
        .or_else(|| page_raw.pointer("/greenPlum/ingredients"))
        .or_else(|| page_raw.pointer("/qingmei/ingredients"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::prepare;
    use serde_json::json;

    #[test]
    fn recipe_uses_available_inventory_aliases_and_rejects_overdraw() {
        let raw = json!({"greenPlum":{"ingredients":[{"uid":"seed-a","count":"3"}]}});
        let selected = HashMap::from([("seed-a".to_string(), 2)]);

        assert_eq!(
            prepare(&raw, &selected, "zh-CN").unwrap(),
            json!({"ingredients":[{"uid":"seed-a","count":2}]})
        );
        assert_eq!(
            prepare(&raw, &HashMap::from([("seed-a".to_string(), 4)]), "en-US").unwrap_err(),
            "Ingredient seed-a has an invalid quantity or exceeds current stock."
        );
        assert_eq!(
            prepare(&raw, &HashMap::new(), "en-US").unwrap_err(),
            "Select at least one brewing ingredient."
        );

        let numeric_uid = json!({"qingMei":{"ingredients":[{"uid":7,"count":"3"}]}});
        assert_eq!(
            prepare(&numeric_uid, &HashMap::from([("7".into(), 2)]), "zh-CN").unwrap(),
            json!({"ingredients":[{"uid":"7","count":2}]})
        );
    }
}
