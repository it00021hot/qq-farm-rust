use super::super::super::activity_section_for_record;

fn timestamp_ms(record: &serde_json::Value, key: &str) -> i64 {
    let value = record.get(key).unwrap_or(&serde_json::Value::Null);
    let parsed = match value {
        serde_json::Value::Null => 0.0,
        serde_json::Value::Bool(value) => f64::from(u8::from(*value)),
        serde_json::Value::Number(value) => value.as_f64().unwrap_or_default(),
        serde_json::Value::String(value) if value.trim().is_empty() => 0.0,
        serde_json::Value::String(value) => value.trim().parse::<f64>().unwrap_or_default(),
        serde_json::Value::Array(values) if values.is_empty() => 0.0,
        serde_json::Value::Array(values) if values.len() == 1 => values[0]
            .as_str()
            .map(|value| value.trim().parse::<f64>().unwrap_or_default())
            .or_else(|| values[0].as_f64())
            .unwrap_or_default(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => 0.0,
    };
    if parsed == 0.0 || !parsed.is_finite() {
        return 0;
    }
    let milliseconds = if parsed > 1_000_000_000_000.0 { parsed } else { parsed * 1_000.0 };
    milliseconds as i64
}

fn js_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

fn js_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(values) => values
            .iter()
            .map(|value| if value.is_null() { String::new() } else { js_string(value) })
            .collect::<Vec<_>>()
            .join(","),
        serde_json::Value::Object(_) => "[object Object]".into(),
    }
}

fn first_truthy_field<'a>(
    source: &'a serde_json::Value,
    fields: &[&str],
) -> Option<&'a serde_json::Value> {
    fields.iter().filter_map(|field| source.get(field)).find(|value| js_truthy(value))
}

fn fallback_entry(
    raw: &serde_json::Value,
    key: &str,
    name: &str,
    target: &str,
    id_fields: &[&str],
    name_fields: &[&str],
) -> Option<serde_json::Value> {
    let source = raw.get(key)?;
    let id = first_truthy_field(source, id_fields).map(js_string).unwrap_or_else(|| key.into());
    let name =
        first_truthy_field(source, name_fields).map(js_string).unwrap_or_else(|| name.into());
    Some(serde_json::json!({
        "id": id,
        "name": name,
        "startTime": source.get("startTime").cloned().unwrap_or(serde_json::Value::Null),
        "endTime": source.get("endTime").cloned().unwrap_or(serde_json::Value::Null),
        "gameplayKey": if target == "travel" || target == "constellation" || target == "shop" || target == "solar" { "stellar" } else { target },
        "detailTarget": target,
    }))
}

fn stellar_available(raw: &serde_json::Value) -> bool {
    ["/season/title", "/shop/title", "/constellation/title"].into_iter().any(|path| {
        raw.pointer(path).and_then(serde_json::Value::as_str).is_some_and(|title| !title.is_empty())
    }) || raw
        .pointer("/solarTerms/terms")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|terms| !terms.is_empty())
}

fn stellar_fallback_entry(raw: &serde_json::Value, language: &str) -> serde_json::Value {
    let season = raw.get("season").unwrap_or(&serde_json::Value::Null);
    let id = raw
        .pointer("/season/pass/activityId")
        .filter(|value| js_truthy(value))
        .or_else(|| season.get("id").filter(|value| js_truthy(value)))
        .map(js_string)
        .unwrap_or_else(|| "stellar".into());
    let name = season
        .get("title")
        .filter(|value| js_truthy(value))
        .map(js_string)
        .unwrap_or_else(|| crate::i18n::text(language, "千星游记", "Travel Pass").into());
    serde_json::json!({
        "id": id,
        "name": name,
        "startTime": season.get("startTime").cloned().unwrap_or(serde_json::Value::Null),
        "endTime": season.get("endTime").cloned().unwrap_or(serde_json::Value::Null),
        "gameplayKey": "stellar",
        "gameplayTargets": ["travel", "constellation", "shop", "solar"],
        "detailTarget": "travel",
    })
}

fn contains_section(records: &[serde_json::Value], section: &str) -> bool {
    records.iter().any(|record| activity_section_for_record(record) == Some(section))
}

fn contains_green_plum_entry(records: &[serde_json::Value]) -> bool {
    records.iter().any(|record| {
        record.get("gameplayKey").and_then(serde_json::Value::as_str) == Some("greenPlum")
            || record.get("detailTarget").and_then(serde_json::Value::as_str) == Some("greenPlum")
    })
}

fn has_truthy_value(value: Option<&serde_json::Value>) -> bool {
    value.is_some_and(js_truthy)
}

pub(super) fn entries(raw: &serde_json::Value, language: &str) -> Vec<serde_json::Value> {
    let mut result =
        raw.get("activities").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
    if result.is_empty() && stellar_available(raw) {
        result.push(stellar_fallback_entry(raw, language));
    }

    if !contains_section(&result, "qixi")
        && raw.get("qixi").is_some_and(|value| {
            value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| !name.is_empty())
                || has_truthy_value(value.get("groupId"))
        })
    {
        if let Some(entry) = fallback_entry(
            raw,
            "qixi",
            crate::i18n::text(language, "鹊桥寄情", "Magpie Bridge"),
            "qixi",
            &["groupId", "activityId"],
            &["name", "title"],
        ) {
            result.push(entry);
        }
    }
    if !contains_green_plum_entry(&result)
        && raw.pointer("/greenPlum/known").and_then(serde_json::Value::as_bool) == Some(true)
        && raw.pointer("/greenPlum/active").and_then(serde_json::Value::as_bool) == Some(true)
    {
        if let Some(entry) = fallback_entry(
            raw,
            "greenPlum",
            crate::i18n::text(language, "青梅", "Green Plum"),
            "greenPlum",
            &["activityId", "brewActivityId"],
            &["name"],
        ) {
            result.push(entry);
        }
    }
    if !contains_section(&result, "charity")
        && raw.get("charity").is_some_and(|value| {
            value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| !name.is_empty())
                || has_truthy_value(value.get("activityId"))
        })
    {
        if let Some(entry) = fallback_entry(
            raw,
            "charity",
            crate::i18n::text(language, "公益小红花", "Red Flower Charity"),
            "charity",
            &["groupId", "activityId"],
            &["name", "title"],
        ) {
            result.push(entry);
        }
    }
    if !contains_section(&result, "weather") {
        let weather = raw.get("weatherActivity");
        result.push(serde_json::json!({
            "id": "2026070300",
            "name": crate::i18n::text(language, "雨落成诗", "Rainfall Poems"),
            "startTime": weather.and_then(|value| value.pointer("/activity/startTime")).or_else(|| weather.and_then(|value| value.get("startTime"))).cloned().unwrap_or(serde_json::Value::Null),
            "endTime": weather.and_then(|value| value.pointer("/activity/endTime")).or_else(|| weather.and_then(|value| value.get("endTime"))).cloned().unwrap_or(serde_json::Value::Null),
            "gameplayKey": "weather",
            "detailTarget": "weather",
        }));
    }
    result
}

pub(super) fn status(record: &serde_json::Value, now_ms: i64) -> (&'static str, u8) {
    let start = timestamp_ms(record, "startTime");
    let end = timestamp_ms(record, "endTime");
    if end > 0 && now_ms >= end {
        ("已结束", 2)
    } else if start > 0 && now_ms < start {
        ("未开始", 1)
    } else {
        ("进行中", 0)
    }
}

pub(super) fn sort_records(records: &mut [serde_json::Value], now_ms: i64) {
    records.sort_by_key(|record| (status(record, now_ms).1, timestamp_ms(record, "startTime")));
}

fn format_day(timestamp_ms: i64) -> Option<String> {
    (timestamp_ms > 0).then(|| {
        chrono::DateTime::from_timestamp_millis(timestamp_ms)
            .map(|value| value.with_timezone(&chrono::Local).format("%Y/%m/%d").to_string())
    })?
}

pub(super) fn period(record: &serde_json::Value, language: &str) -> String {
    let start = format_day(timestamp_ms(record, "startTime"));
    let end = format_day(timestamp_ms(record, "endTime"));
    match (start, end) {
        (Some(start), Some(end)) => format!("{start} - {end}"),
        (Some(start), None) => {
            format!("{start} {}", crate::i18n::text(language, "未开始", "Upcoming"))
        }
        (None, Some(end)) => {
            format!("{end} {}", crate::i18n::text(language, "已结束", "Ended"))
        }
        (None, None) => crate::i18n::text(language, "活动时间待定", "Schedule TBD").into(),
    }
}

#[cfg(test)]
mod tests;
