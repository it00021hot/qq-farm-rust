use std::collections::HashSet;

use serde_json::Value;

pub(crate) fn bag_category_for_type(item_type: i64) -> &'static str {
    match item_type {
        6 | 17 => "fruit",
        5 => "seed",
        11 => "tool",
        _ => "other",
    }
}

const VUE_HIDDEN_BAG_IDS: [i64; 8] = [
    1,
    1001,
    qq_farm_core::constants::COUPON_ITEM_ID,
    1101,
    qq_farm_core::constants::NORMAL_CONTAINER_ID,
    qq_farm_core::constants::ORGANIC_CONTAINER_ID,
    qq_farm_core::constants::COLLECTION_NORMAL_ID,
    qq_farm_core::constants::COLLECTION_RARE_ID,
];

pub(in crate::desktop::personal) const BAG_CATEGORIES: [(&str, &str); 5] =
    [("all", "全部"), ("fruit", "果实"), ("seed", "种子"), ("tool", "道具"), ("other", "其他")];

pub(super) fn category_label(category: &str, language: &str) -> &'static str {
    match category {
        "all" => crate::i18n::text(language, "全部", "All"),
        "fruit" => crate::i18n::text(language, "果实", "Fruit"),
        "seed" => crate::i18n::text(language, "种子", "Seed"),
        "tool" => crate::i18n::text(language, "道具", "Tool"),
        _ => crate::i18n::text(language, "其他", "Other"),
    }
}

pub(super) fn visible_items(raw: &Value) -> Vec<&Value> {
    raw.pointer("/bag/items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| !is_hidden(item))
        .collect()
}

pub(super) fn is_hidden(item: &Value) -> bool {
    VUE_HIDDEN_BAG_IDS.contains(&number(item.get("id")))
}

pub(super) fn category(item: &Value) -> &'static str {
    bag_category_for_type(number(item.get("itemType")))
}

pub(super) fn matches_category(item: &Value, selected: &str) -> bool {
    selected == "all" || category(item) == selected
}

pub(in crate::desktop::personal) fn can_sell(item: &Value) -> bool {
    matches!(number(item.get("itemType")), 6 | 17)
        && item.get("sellable").and_then(Value::as_bool).unwrap_or(false)
}

pub(super) fn can_use(item: &Value) -> bool {
    number(item.get("itemType")) == 11
}

pub(in crate::desktop::personal) fn filtered_items<'a>(
    raw: &'a Value,
    selected: &str,
    query: &str,
) -> Vec<&'a Value> {
    filtered_items_with_indices(raw, selected, query).into_iter().map(|(_, item)| item).collect()
}

pub(super) fn filtered_items_with_indices<'a>(
    raw: &'a Value,
    selected: &str,
    query: &str,
) -> Vec<(usize, &'a Value)> {
    let query = query.to_lowercase();
    raw.pointer("/bag/items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, item)| {
            !is_hidden(item) && matches_category(item, selected) && matches_query(item, &query)
        })
        .collect()
}

fn matches_query(item: &Value, query: &str) -> bool {
    query.is_empty()
        || item
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name.to_lowercase().contains(query))
        || item.to_string().to_lowercase().contains(query)
}

pub(in crate::desktop::personal) fn category_counts(raw: &Value) -> [usize; 5] {
    let items = visible_items(raw);
    let mut counts = [items.len(), 0, 0, 0, 0];
    for item in items {
        match category(item) {
            "fruit" => counts[1] += 1,
            "seed" => counts[2] += 1,
            "tool" => counts[3] += 1,
            _ => counts[4] += 1,
        }
    }
    counts
}

pub(in crate::desktop::personal) fn selected_sellable_count(
    raw: &Value,
    selected_category: &str,
    query: &str,
    selected_ids: &HashSet<i64>,
) -> usize {
    filtered_items(raw, selected_category, query)
        .into_iter()
        .filter(|item| can_sell(item) && selected_ids.contains(&number(item.get("id"))))
        .count()
}

pub(in crate::desktop::personal) fn number(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse::<i64>().ok()))
        .unwrap_or_default()
}

pub(super) fn text<'a>(value: Option<&'a Value>, fallback: &'a str) -> &'a str {
    value.and_then(Value::as_str).filter(|value| !value.is_empty()).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::{
        category_counts, filtered_items, filtered_items_with_indices, selected_sellable_count,
    };
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn bag_categories_hide_vue_system_items_and_count_visible_rows() {
        let raw = json!({"bag": {"items": [
            {"id": 1002, "itemType": 17, "name": "点券"},
            {"id": 7, "itemType": 17, "sellable": true, "name": "苹果"},
            {"id": 8, "itemType": 5, "name": "种子"},
            {"id": 9, "itemType": 11, "name": "肥料"}
        ]}});

        assert_eq!(category_counts(&raw), [3, 1, 1, 1, 0]);
        assert_eq!(filtered_items(&raw, "fruit", "").len(), 1);
        assert_eq!(filtered_items(&raw, "all", "点券").len(), 0);
    }

    #[test]
    fn sellable_count_tracks_the_filtered_category_like_vue() {
        let raw = json!({"bag": {"items": [
            {"id": 7, "itemType": 17, "sellable": true},
            {"id": 8, "itemType": 17, "sellable": false},
            {"id": 9, "itemType": 5, "sellable": true}
        ]}});
        let selected = HashSet::from([7, 9]);
        assert_eq!(selected_sellable_count(&raw, "fruit", "", &selected), 1);
        assert_eq!(selected_sellable_count(&raw, "seed", "", &selected), 0);
        assert_eq!(selected_sellable_count(&raw, "all", "", &selected), 1);
    }

    #[test]
    fn virtualized_bag_indices_preserve_source_order_and_filters() {
        let raw = json!({"bag": {"items": [
            {"id": 1, "itemType": 17, "name": "系统道具"},
            {"id": 7, "itemType": 17, "name": "红苹果"},
            {"id": 8, "itemType": 5, "name": "苹果种子"},
            {"id": 9, "itemType": 17, "name": "青苹果"}
        ]}});

        let indices = |category, query| {
            filtered_items_with_indices(&raw, category, query)
                .into_iter()
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        };
        assert_eq!(indices("fruit", "苹果"), [1, 3]);
        assert_eq!(indices("seed", "苹果"), [2]);
        assert_eq!(indices("all", "系统道具"), Vec::<usize>::new());
    }
}
