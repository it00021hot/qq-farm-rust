use crate::pages::DisplayRow;
use serde_json::Value;

use super::super::common::append_entity_rows;

pub(super) fn append_rows(value: &Value, rows: &mut Vec<DisplayRow>) {
    let start = rows.len();
    append_entity_rows(value, "records", "互动记录", rows);
    for row in &mut rows[start..] {
        let Some(record) = row.record.as_object() else { continue };
        let visitor_gid = record
            .get("visitorGid")
            .map(|gid| match gid {
                Value::String(value) => value.clone(),
                _ => gid.to_string(),
            })
            .unwrap_or_else(|| "undefined".into());
        row.title = record
            .get("nick")
            .and_then(Value::as_str)
            .filter(|nick| !nick.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("GID:{visitor_gid}"));
        row.detail = record
            .get("actionDetail")
            .and_then(Value::as_str)
            .filter(|detail| !detail.is_empty())
            .or_else(|| record.get("actionLabel").and_then(Value::as_str))
            .unwrap_or_default()
            .to_owned();
        row.state = record
            .get("actionLabel")
            .and_then(Value::as_str)
            .filter(|label| !label.is_empty())
            .unwrap_or("互动")
            .to_owned();
    }
}
