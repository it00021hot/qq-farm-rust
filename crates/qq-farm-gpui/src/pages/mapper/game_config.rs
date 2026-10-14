use super::common::append_entity_rows;
use crate::pages::DisplayRow;
use serde_json::Value;
pub(super) fn rows(raw: &Value) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    if let Some(o) = raw.as_object() {
        for (k, l) in [("seeds", "种子"), ("fruits", "果实"), ("items", "道具")] {
            if let Some(v) = o.get(k) {
                append_entity_rows(v, k, l, &mut rows);
            }
        }
    }
    rows
}
