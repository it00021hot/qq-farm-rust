use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ShopCategory {
    pub(super) id: String,
    pub(super) name: String,
}

pub(super) fn shop_categories(shop: &Value) -> Vec<ShopCategory> {
    let Some(categories) = shop.get("categories").and_then(Value::as_array) else {
        return Vec::new();
    };

    categories
        .iter()
        .filter_map(|category| match category {
            Value::String(name) => {
                let name = name.trim();
                (!name.is_empty()).then(|| ShopCategory { id: name.into(), name: name.into() })
            }
            Value::Object(category) => {
                let empty = Value::Null;
                let id = js_string(
                    category
                        .get("id")
                        .filter(|value| !value.is_null())
                        .or_else(|| category.get("name"))
                        .unwrap_or(&empty),
                );
                let name = js_string(
                    category
                        .get("name")
                        .filter(|value| !value.is_null())
                        .or_else(|| category.get("id"))
                        .unwrap_or(&empty),
                );
                let id = id.trim();
                let name = name.trim();
                if id.is_empty() && name.is_empty() {
                    None
                } else {
                    Some(ShopCategory {
                        id: if id.is_empty() { name } else { id }.into(),
                        name: if name.is_empty() { id } else { name }.into(),
                    })
                }
            }
            _ => None,
        })
        .collect()
}

pub(super) fn goods_category(goods: &Value) -> String {
    ["categoryId", "categoryName", "category"]
        .iter()
        .filter_map(|key| goods.get(*key))
        .find(|value| is_truthy(value))
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(super) fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

pub(super) fn js_string(value: &Value) -> String {
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
            .map(|value| if value.is_null() { String::new() } else { js_string(value) })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{goods_category, shop_categories, ShopCategory};
    use serde_json::json;

    #[test]
    fn category_options_accept_strings_and_vue_id_name_objects() {
        assert_eq!(
            shop_categories(&json!({
                "categories": [
                    " 种子 ",
                    {"id": "tool-id", "name": "道具"},
                    {"name": "其他"},
                    {"id": "legacy-id"},
                    {"id": " ", "name": " "},
                    7
                ]
            })),
            [
                ShopCategory { id: "种子".into(), name: "种子".into() },
                ShopCategory { id: "tool-id".into(), name: "道具".into() },
                ShopCategory { id: "其他".into(), name: "其他".into() },
                ShopCategory { id: "legacy-id".into(), name: "legacy-id".into() },
            ]
        );
    }

    #[test]
    fn goods_category_uses_vue_truthy_field_precedence_and_trimming() {
        assert_eq!(
            goods_category(&json!({
                "categoryId": "  ", "categoryName": "道具", "category": "old"
            })),
            ""
        );
        assert_eq!(
            goods_category(&json!({
                "categoryId": "", "categoryName": "  道具  ", "category": "old"
            })),
            "道具"
        );
        assert_eq!(goods_category(&json!({"categoryId": 0, "categoryName": 12})), "12");
        assert_eq!(goods_category(&json!({"categoryId": "seeds", "category": "legacy"})), "seeds");
        assert_eq!(goods_category(&json!({"category": " legacy "})), "legacy");
    }
}
