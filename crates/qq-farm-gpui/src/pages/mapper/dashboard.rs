use serde_json::Value;

use crate::pages::{DisplayRow, Page};

use super::common::append_value_rows;
use super::format::{number_at, string_at};

pub(super) fn rows(raw: &Value) -> Vec<DisplayRow> {
    let Some(object) = raw.as_object() else {
        let mut rows = Vec::new();
        append_value_rows(raw, Page::Dashboard, "", &mut rows);
        return rows;
    };
    let logs = object.get("logs").and_then(Value::as_array).map_or(0, Vec::len);
    let mut rows = Vec::with_capacity(logs + 1);
    if let Some(status) = object.get("status") {
        rows.push(DisplayRow {
            id: "account-status".into(),
            section: "status".into(),
            title: format!(
                "{} · Lv.{}",
                string_at(status, &["nick", "status.name"], "账号"),
                number_at(status, &["level", "status.level"])
            ),
            detail: format!(
                "金币 {} · 经验 {}",
                number_at(status, &["gold"]),
                number_at(status, &["exp"])
            ),
            state: if status.get("online").and_then(Value::as_bool).unwrap_or(false) {
                "在线".into()
            } else if status.get("running").and_then(Value::as_bool).unwrap_or(false) {
                "连接中".into()
            } else {
                "未运行".into()
            },
            record: status.clone(),
        });
    }
    rows.extend(
        object
            .get("logs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|entry| {
                entry.pointer("/meta/event").and_then(Value::as_str) != Some("avatar_probe")
            })
            .map(|entry| {
                let warn = entry.get("isWarn").and_then(Value::as_bool).unwrap_or(false);
                DisplayRow {
                    id: format!(
                        "log-{}",
                        entry.get("ts").and_then(Value::as_i64).unwrap_or_default()
                    ),
                    section: if warn { "warnings" } else { "logs" }.into(),
                    title: string_at(entry, &["msg", "message"], "运行记录"),
                    detail: string_at(entry, &["time", "tag"], ""),
                    state: if warn { "提醒".into() } else { String::new() },
                    record: entry.clone(),
                }
            }),
    );
    rows
}

#[cfg(test)]
mod tests {
    use super::rows;

    #[test]
    fn dashboard_snapshot_preserves_status_warning_and_hidden_event_semantics() {
        let raw = serde_json::json!({
            "status": {"nick": "小农夫", "level": 12, "gold": 345, "exp": 67, "online": true},
            "logs": [
                {"ts": 1, "time": "12:00:00", "tag": "农场", "msg": "收获完成", "isWarn": false, "meta": {"event": "harvest_crop"}},
                {"ts": 2, "time": "12:00:01", "tag": "错误", "msg": "登录失效", "isWarn": true, "meta": {"event": "ws_400"}},
                {"ts": 3, "time": "12:00:02", "tag": "系统", "msg": "诊断", "isWarn": false, "meta": {"event": "avatar_probe"}}
            ]
        });

        let mapped = rows(&raw);
        assert_eq!(mapped[0].section, "status");
        assert!(mapped[0].title.contains("小农夫"));
        assert!(mapped.iter().any(|row| row.section == "logs" && row.title == "收获完成"));
        assert!(mapped.iter().any(|row| row.section == "warnings" && row.title == "登录失效"));
        assert!(!mapped.iter().any(|row| row.title == "诊断"));
    }
}
