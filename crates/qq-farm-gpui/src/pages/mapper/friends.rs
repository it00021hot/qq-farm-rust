use super::common::append_entity_rows;
use crate::pages::DisplayRow;
use serde_json::Value;

mod visitors;

fn integer(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| value.as_i64().or_else(|| value.as_str()?.trim().parse().ok()))
}

fn integer_at(value: &Value, path: &str) -> i64 {
    integer(value.pointer(path)).unwrap_or_default()
}

pub(super) fn rows(raw: &Value) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    let Some(object) = raw.as_object() else { return rows };
    let all = object.get("friends").and_then(Value::as_array).cloned().unwrap_or_default();
    let blacklist = object.get("blacklist").and_then(Value::as_array).cloned().unwrap_or_default();
    let ids = blacklist.iter().filter_map(|v| integer(Some(v))).collect::<Vec<_>>();
    let visible = all
        .iter()
        .filter(|f| !ids.contains(&integer(f.get("gid")).unwrap_or_default()))
        .cloned()
        .collect::<Vec<_>>();
    append_entity_rows(&Value::Array(visible), "friends", "好友", &mut rows);
    let blocked =
        ids.iter()
            .map(|gid| {
                all.iter().find(|f| integer(f.get("gid")) == Some(*gid)).cloned().unwrap_or_else(
                    || serde_json::json!({"gid":gid,"nickname":format!("GID:{gid}")}),
                )
            })
            .collect::<Vec<_>>();
    append_entity_rows(&Value::Array(blocked), "blacklist", "黑名单", &mut rows);
    if let Some(records) = object.get("records") {
        visitors::append_rows(records, &mut rows);
    }
    if let Some(items) = object.get("interactionItems") {
        append_entity_rows(items, "interactionItems", "好友互动道具", &mut rows);
    }
    rows
}
pub(super) fn sort_rows(rows: &mut [DisplayRow]) {
    rows.sort_by(|l, r| {
        let order = |s: &str| match s {
            "friends" => 0,
            "blacklist" => 1,
            "records" => 2,
            "interactionItems" => 3,
            _ => 4,
        };
        let ord = order(&l.section).cmp(&order(&r.section));
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        if l.section != "friends" {
            return std::cmp::Ordering::Equal;
        }
        let counts = |row: &DisplayRow| {
            let steal = integer_at(&row.record, "/plant/stealNum");
            let help = ["dryNum", "weedNum", "insectNum"]
                .iter()
                .map(|field| integer_at(&row.record, &format!("/plant/{field}")))
                .sum::<i64>();
            let level = integer(row.record.get("level")).unwrap_or_default();
            let gid = integer(row.record.get("gid")).unwrap_or_default();
            (steal, help, level, gid)
        };
        let (a, b, c, d) = counts(l);
        let (e, f, g, h) = counts(r);
        e.cmp(&a).then_with(|| f.cmp(&b)).then_with(|| g.cmp(&c)).then_with(|| d.cmp(&h))
    })
}
