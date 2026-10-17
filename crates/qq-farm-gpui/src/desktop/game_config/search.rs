use crate::pages::DisplayRow;

/// Section and applied-select filters shared by the rendered catalog rows and
/// the pagination footer count.
pub(in crate::desktop) fn game_config_filters_match(
    row: &DisplayRow,
    section: &str,
    seasons: Option<i64>,
    rarity: Option<i64>,
    item_type: Option<i64>,
) -> bool {
    (section == "all" || row.section == section)
        && (section != "seeds"
            || seasons.is_none_or(|seasons| {
                row.record.get("seasons").and_then(serde_json::Value::as_i64) == Some(seasons)
            }))
        && (!matches!(section, "fruits" | "items")
            || rarity.is_none_or(|rarity| {
                row.record.get("rarity").and_then(serde_json::Value::as_i64) == Some(rarity)
            }))
        && (section != "items"
            || item_type.is_none_or(|item_type| {
                row.record.get("type").and_then(serde_json::Value::as_i64) == Some(item_type)
            }))
}

pub(crate) fn matches_vue_keyword(row: &DisplayRow, keyword: &str) -> bool {
    let keyword = keyword.trim().to_lowercase();
    if keyword.is_empty() {
        return true;
    }

    let name_matches = row
        .record
        .get("name")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|name| name.to_lowercase().contains(&keyword));
    if name_matches {
        return true;
    }

    let id_fields: &[&str] = match row.section.as_str() {
        "seeds" => &["seedId"],
        "fruits" => &["id", "plantId"],
        "items" => &["id"],
        _ => &[],
    };
    id_fields.iter().any(|field| {
        row.record
            .get(*field)
            .filter(|value| !value.is_null())
            .map(super::super::json_text::value_to_action_string)
            .is_some_and(|value| value.contains(&keyword))
    })
}

#[cfg(test)]
mod tests {
    use super::matches_vue_keyword;
    use crate::pages::DisplayRow;
    use serde_json::json;

    fn row(section: &str, record: serde_json::Value) -> DisplayRow {
        DisplayRow {
            id: "row".into(),
            section: section.into(),
            title: record
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .into(),
            detail: "unsearchable presentation detail".into(),
            state: String::new(),
            record,
        }
    }

    #[test]
    fn search_fields_match_vue_for_each_catalog_tab() {
        let seed = row(
            "seeds",
            json!({"name":"Golden Wheat", "seedId":1204, "requiredLevel":18, "description":"rare crop"}),
        );
        assert!(matches_vue_keyword(&seed, "  GOLDEN "));
        assert!(matches_vue_keyword(&seed, "1204"));
        assert!(!matches_vue_keyword(&seed, "18"));
        assert!(!matches_vue_keyword(&seed, "rare crop"));
        assert!(!matches_vue_keyword(&seed, "unsearchable presentation detail"));

        let fruit = row("fruits", json!({"name":"Wheat", "id":4404, "plantId":1204}));
        assert!(matches_vue_keyword(&fruit, "1204"));
        assert!(matches_vue_keyword(&fruit, "4404"));

        let item = row("items", json!({"name":"Watering can", "id":77, "type":9}));
        assert!(matches_vue_keyword(&item, "77"));
        assert!(!matches_vue_keyword(&item, "9"));
    }
}
