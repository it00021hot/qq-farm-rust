use std::collections::HashSet;

use serde_json::Value;

use super::status::friend_number_at;

pub(crate) fn friend_value_number(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.trim().parse().ok()))
        .unwrap_or_default()
}

fn js_number_string(value: &str) -> Option<f64> {
    if value.trim().is_empty() {
        return Some(0.);
    }
    let value = value.trim();
    value.parse::<f64>().ok().or_else(|| {
        [("0x", 16), ("0X", 16), ("0b", 2), ("0B", 2), ("0o", 8), ("0O", 8)].into_iter().find_map(
            |(prefix, radix)| {
                value.strip_prefix(prefix).and_then(|digits| {
                    u64::from_str_radix(digits, radix).ok().map(|number| number as f64)
                })
            },
        )
    })
}

fn js_array_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some(String::new()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value.clone()),
        Value::Array(values) => values
            .iter()
            .map(js_array_string)
            .collect::<Option<Vec<_>>>()
            .map(|values| values.join(",")),
        Value::Object(_) => Some("[object Object]".into()),
    }
}

/// Match JavaScript's `Number(value)` conversion used by the Vue visitor filter
/// and action badges, including scalar and array values in JSON payloads.
pub(crate) fn friend_action_type_number(value: Option<&Value>) -> Option<f64> {
    value.and_then(|value| match value {
        Value::Null => Some(0.),
        Value::Bool(value) => Some(if *value { 1. } else { 0. }),
        Value::Number(value) => value.as_f64(),
        Value::String(value) => js_number_string(value),
        Value::Array(_) => js_number_string(&js_array_string(value)?),
        Value::Object(_) => None,
    })
}

pub(crate) fn stealable_friends_matching(page: &Value) -> Vec<Value> {
    let blocked = page
        .get("blacklist")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|gid| gid.as_i64().or_else(|| gid.as_str()?.parse::<i64>().ok()))
        .collect::<HashSet<_>>();
    let mut friends = page
        .get("friends")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|friend| {
            let gid = friend_number_at(friend, "/gid");
            gid > 0 && friend_number_at(friend, "/plant/stealNum") > 0 && !blocked.contains(&gid)
        })
        .cloned()
        .collect::<Vec<_>>();
    friends.sort_by(|left, right| {
        let steal_left = friend_number_at(left, "/plant/stealNum");
        let steal_right = friend_number_at(right, "/plant/stealNum");
        let help_left = ["dryNum", "weedNum", "insectNum"]
            .into_iter()
            .map(|key| friend_number_at(left, &format!("/plant/{key}")))
            .sum::<i64>();
        let help_right = ["dryNum", "weedNum", "insectNum"]
            .into_iter()
            .map(|key| friend_number_at(right, &format!("/plant/{key}")))
            .sum::<i64>();
        let level_left = friend_number_at(left, "/level");
        let level_right = friend_number_at(right, "/level");
        let gid_left = friend_number_at(left, "/gid");
        let gid_right = friend_number_at(right, "/gid");
        steal_right
            .cmp(&steal_left)
            .then_with(|| help_right.cmp(&help_left))
            .then_with(|| level_right.cmp(&level_left))
            .then_with(|| gid_left.cmp(&gid_right))
    });
    friends
}
