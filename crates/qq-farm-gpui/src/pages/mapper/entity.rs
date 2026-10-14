use serde_json::Value;

use crate::pages::DisplayRow;

use super::format::{humanize_key, number_at, state_at, string_at, value_string};

pub(super) fn entity_row(value: &Value, id: &str, label: &str) -> DisplayRow {
    let section = id.split('/').next().unwrap_or(id);
    if id.contains("/happy/logs/") {
        let seq = number_at(value, &["seq"]);
        let actor = string_at(value, &["actor.name"], &format!("玩家#{seq}"));
        let score = number_at(value, &["score"]);
        let timestamp = number_at(value, &["createdAt"]);
        return DisplayRow {
            id: id.into(),
            section: section.into(),
            title: actor,
            detail: format!("快乐值 {}{} · {timestamp}", if score > 0 { "+" } else { "" }, score),
            state: String::new(),
            record: value.clone(),
        };
    }
    let default_title = if section == "lands" {
        format!("第 {} 块地", number_at(value, &["id"]))
    } else {
        label.to_string()
    };
    let title = if section == "lands" {
        default_title
    } else {
        string_at(
            value,
            &["name", "title", "nickname", "nick", "plantName", "reward.name", "id"],
            &default_title,
        )
    };
    let mut details = ["level", "count", "price.count", "status", "description", "desc"]
        .iter()
        .filter_map(|path| {
            value
                .pointer(&format!("/{}", path.replace('.', "/")))
                .filter(|v| !v.is_null())
                .map(|v| format!("{} {}", humanize_key(path), value_string(v)))
        })
        .collect::<Vec<_>>();
    if section == "lands" {
        for (key, label) in [
            ("phaseName", "阶段"),
            ("matureInSec", "成熟倒计时"),
            ("needWater", "需要浇水"),
            ("needWeed", "需要除草"),
            ("needBug", "需要除虫"),
        ] {
            if let Some(value) = value.get(key).filter(|value| {
                !value.is_null() && value.as_bool() != Some(false) && value.as_i64() != Some(0)
            }) {
                details.push(format!("{label} {}", value_string(value)));
            }
        }
        if value.get("unlocked").and_then(Value::as_bool) == Some(false) {
            details.push("未解锁".into());
        }
    } else if section == "bag" {
        for (key, label) in
            [("category", "分类"), ("sellStatus", "出售状态"), ("priceUnit", "售价")]
        {
            if let Some(value) = value.get(key).filter(|value| !value.is_null()) {
                details.push(format!("{label} {}", value_string(value)));
            }
        }
    } else if section == "interactionItems" {
        let sellable = number_at(value, &["saleConditionSatisfiedCount"]);
        if sellable > 0 {
            details.push(format!("可出售 {sellable}"));
        }
    } else if section == "friends" {
        if let Some(level) = value.get("level").and_then(Value::as_i64) {
            details.push(format!("Lv{level}"));
        }
        if let Some(gold) = value.get("gold").and_then(Value::as_i64) {
            details.push(format!("金币 {}", format_number(gold)));
        }
        for (key, label) in [
            ("plant.stealNum", "可偷"),
            ("plant.dryNum", "待浇水"),
            ("plant.weedNum", "待除草"),
            ("plant.insectNum", "待除虫"),
        ] {
            if let Some(value) = value.pointer(&format!("/{}", key.replace('.', "/"))) {
                details.push(format!("{label} {}", value_string(value)));
            }
        }
    }
    if section == "bag" {
        let count = value.get("count").and_then(Value::as_i64).unwrap_or(0);
        let title = if count > 0 { format!("{title} ×{count}") } else { title };
        return DisplayRow {
            id: id.into(),
            section: section.into(),
            title,
            detail: details.join("  ·  "),
            state: if value.get("sellable").and_then(Value::as_bool).unwrap_or(false) {
                "可出售".into()
            } else {
                String::new()
            },
            record: value.clone(),
        };
    }
    let detail = if details.is_empty() { label.to_string() } else { details.join("  ·  ") };
    let state = if section == "friends" {
        let actions = [
            ("plant.stealNum", "偷"),
            ("plant.dryNum", "水"),
            ("plant.weedNum", "草"),
            ("plant.insectNum", "虫"),
        ]
        .into_iter()
        .filter_map(|(key, label)| {
            let count = value.pointer(&format!("/{}", key.replace('.', "/")))?.as_i64()?;
            (count > 0).then(|| format!("{label}{count}"))
        })
        .collect::<Vec<_>>();
        if actions.is_empty() {
            "暂无可执行互动".into()
        } else {
            actions.join(" ")
        }
    } else {
        state_at(value)
    };
    DisplayRow {
        id: id.into(),
        section: section.into(),
        title,
        detail,
        state,
        record: value.clone(),
    }
}

fn format_number(value: i64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    formatted
}
