use serde_json::Value;
use std::collections::HashSet;

use super::{search::friend_value_number, status::friend_number_at};

pub(crate) fn friend_pet_sync_progress(page: &Value) -> (usize, usize) {
    let blocked = page
        .get("blacklist")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| friend_value_number(Some(value)))
        .collect::<HashSet<_>>();
    let friends = page.get("friends").and_then(Value::as_array).into_iter().flatten();
    let mut known = 0;
    let mut total = 0;
    for friend in friends {
        let gid = friend_value_number(friend.get("gid"));
        if gid > 0 && blocked.contains(&gid) {
            continue;
        }
        total += 1;
        if matches!(friend.get("petState").and_then(Value::as_str), Some("protect" | "other")) {
            known += 1;
        }
    }
    (known, total)
}

pub(crate) fn friend_plant_summary_from_lands(lands: &[Value]) -> Value {
    let mut steal = 0;
    let mut water = 0;
    let mut weed = 0;
    let mut bug = 0;
    for land in lands {
        if land.get("unlocked").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        steal += i64::from(land.get("status").and_then(Value::as_str) == Some("stealable"));
        water += i64::from(land.get("needWater").and_then(Value::as_bool) == Some(true));
        weed += i64::from(land.get("needWeed").and_then(Value::as_bool) == Some(true));
        bug += i64::from(land.get("needBug").and_then(Value::as_bool) == Some(true));
    }
    serde_json::json!({
        "stealNum": steal,
        "dryNum": water,
        "weedNum": weed,
        "insectNum": bug,
    })
}

pub(crate) fn apply_friend_operation_snapshot(record: &mut Value, gid: i64, operation: &str) {
    if friend_number_at(record, "/gid") != gid {
        return;
    }
    let Some(plant) = record.get_mut("plant").and_then(Value::as_object_mut) else {
        return;
    };
    match operation {
        "steal" => plant.insert("stealNum".into(), Value::from(0)),
        "help" => {
            plant.insert("dryNum".into(), Value::from(0));
            plant.insert("weedNum".into(), Value::from(0));
            plant.insert("insectNum".into(), Value::from(0));
            None
        }
        _ => None,
    };
}
