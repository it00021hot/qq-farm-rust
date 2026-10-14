use super::common::append_entity_rows;
use crate::pages::DisplayRow;
use serde_json::Value;
pub(super) fn rows(raw: &Value) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    if let Some(object) = raw.as_object() {
        for (key, label) in [("rankings", "收益排名"), ("plantBlacklist", "种植黑名单")] {
            if let Some(v) = object.get(key) {
                append_entity_rows(v, key, label, &mut rows);
            }
        }
    }
    rows
}
