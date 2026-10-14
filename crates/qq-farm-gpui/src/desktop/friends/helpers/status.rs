use serde_json::Value;

use super::search::friend_value_number;

pub(crate) fn friend_number_at(value: &Value, path: &str) -> i64 {
    friend_value_number(value.pointer(path))
}

fn vue_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn vue_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.as_f64().map_or_else(
            || value.to_string(),
            |number| {
                if number.fract() == 0. {
                    format!("{number:.0}")
                } else {
                    number.to_string()
                }
            },
        ),
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(|value| if value.is_null() { String::new() } else { vue_string(value) })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

pub(crate) fn friend_status_localized(record: &Value, language: &str) -> String {
    let plant = record.get("plant").unwrap_or(&Value::Null);
    let english = language == "en-US";
    let status = [
        ("stealNum", if english { "Steal" } else { "偷" }),
        ("dryNum", if english { "Water" } else { "水" }),
        ("weedNum", if english { "Weed" } else { "草" }),
        ("insectNum", if english { "Remove bugs" } else { "虫" }),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        plant.get(key).filter(|value| vue_truthy(value)).map(|value| {
            let count = vue_string(value);
            if english {
                format!("{label} {count}")
            } else {
                format!("{label}{count}")
            }
        })
    })
    .collect::<Vec<_>>();
    if status.is_empty() {
        crate::i18n::text(language, "无操作", "No action").into()
    } else {
        status.join(" ")
    }
}

pub(crate) fn has_friend_action(record: &Value) -> bool {
    let Some(plant) = record.get("plant") else { return false };
    ["stealNum", "dryNum", "weedNum", "insectNum"]
        .iter()
        .any(|key| plant.get(*key).is_some_and(vue_truthy))
}
