use super::*;

pub(super) fn append_dashboard_log(this: &mut FarmDesktop, payload: serde_json::Value) -> bool {
    if payload.pointer("/meta/event").and_then(serde_json::Value::as_str) == Some("avatar_probe") {
        return false;
    }
    this.dashboard.logs_follow_tail = this.dashboard.logs_are_near_bottom();
    if !this.page_raw.get("logs").is_some_and(serde_json::Value::is_array) {
        this.page_raw["logs"] = serde_json::json!([]);
    }
    let Some(logs) = this.page_raw["logs"].as_array_mut() else { return false };
    if logs.iter().any(|existing| existing == &payload) {
        return false;
    }
    logs.push(payload);
    logs.sort_by_key(|entry| {
        entry.get("ts").and_then(serde_json::Value::as_i64).unwrap_or_default()
    });
    if logs.len() > 1000 {
        let overflow = logs.len() - 1000;
        logs.drain(..overflow);
    }
    this.rows = pages::page_data(Page::Dashboard, this.page_raw.clone()).rows;
    if this.dashboard.logs_follow_tail {
        this.dashboard.logs_scroll.scroll_to_bottom();
    }
    true
}

pub(super) fn merge_json_object(target: &mut serde_json::Value, source: &serde_json::Value) {
    if !target.is_object() {
        *target = serde_json::json!({});
    }
    if let (Some(target), Some(source)) = (target.as_object_mut(), source.as_object()) {
        target.extend(source.iter().map(|(key, value)| (key.clone(), value.clone())));
    }
}

pub(super) fn unix_time_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

pub(in crate::desktop) fn bag_item_count(bag: &serde_json::Value, item_id: i64) -> i64 {
    bag.pointer("/items")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            bag.pointer("/systemItems").and_then(serde_json::Value::as_array).into_iter().flatten(),
        )
        .find(|item| item.get("id").and_then(serde_json::Value::as_i64) == Some(item_id))
        .and_then(|item| item.get("count"))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default()
}
