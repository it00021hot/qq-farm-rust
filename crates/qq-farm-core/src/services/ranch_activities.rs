//! 牧场活动目录。时间保持服务器秒，不乘 1000。

use std::sync::Arc;

use chrono::{Datelike, Timelike};
use prost::Message as _;
use serde_json::{json, Value};

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::activitypb::{
    ActivityContent, ActivityData, ActivityListReply, ActivityListRequest,
};

const SERVICE: &str = "gamepb.activitypb.ActivityService";
const RANCH_SCENE: i32 = 2;

struct Window {
    id: String,
    name: String,
    start: i64,
    end: i64,
}

struct Group {
    id: String,
    name: String,
    start: i64,
    end: i64,
    ids: Vec<String>,
}

pub struct RanchActivities {
    gateway: Arc<Gateway>,
}

impl RanchActivities {
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 只取场景 2 实际下发的活动组。`all_activities` 是农场和牧场共用的时间目录，不在这里展示。
    pub async fn snapshot(&self) -> Result<Value> {
        let body = self
            .gateway
            .request(
                SERVICE,
                "List",
                &ActivityListRequest { game_scene_id: RANCH_SCENE }.encode_to_vec(),
            )
            .await?;
        let reply = ActivityListReply::decode(body.as_slice())?;
        Ok(directory(&reply, crate::utils::time::get_server_time_secs()))
    }
}

pub fn directory(reply: &ActivityListReply, now: i64) -> Value {
    let mut rows = group(flatten(reply))
        .into_iter()
        .map(|row| {
            let status = status(row.start, row.end, now);
            json!({
                "id": row.id,
                "name": row.name,
                "startTime": row.start,
                "endTime": row.end,
                "status": status,
                "when": format_range(row.start, row.end),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        status_rank(left.get("status").and_then(Value::as_str).unwrap_or(""))
            .cmp(&status_rank(right.get("status").and_then(Value::as_str).unwrap_or("")))
            .then_with(|| {
                let left_status = left.get("status").and_then(Value::as_str).unwrap_or("");
                let left_end = left.get("endTime").and_then(Value::as_i64).unwrap_or(0);
                let right_end = right.get("endTime").and_then(Value::as_i64).unwrap_or(0);
                let left_start = left.get("startTime").and_then(Value::as_i64).unwrap_or(0);
                let right_start = right.get("startTime").and_then(Value::as_i64).unwrap_or(0);
                if left_status == "ended" {
                    right_end.cmp(&left_end)
                } else {
                    left_start.cmp(&right_start)
                }
            })
    });
    json!({ "serverTime": now, "activities": rows })
}

fn flatten(reply: &ActivityListReply) -> Vec<Window> {
    let mut windows = Vec::new();
    for group in &reply.activities {
        walk(group, &mut windows);
    }
    windows
}

fn walk(data: &ActivityData, windows: &mut Vec<Window>) {
    if let Some(activity) = &data.activity {
        if let Some(window) = window_from(activity) {
            windows.push(window);
        }
    }
    for child in &data.children {
        walk(child, windows);
    }
}

fn window_from(activity: &ActivityContent) -> Option<Window> {
    if activity.activity_id <= 0 {
        return None;
    }
    let id = activity.activity_id.to_string();
    let name = if activity.name.trim().is_empty() {
        format!("活动 {id}")
    } else {
        activity.name.trim().to_owned()
    };
    Some(Window { id, name, start: activity.begin_time, end: activity.end_time })
}

fn group(windows: Vec<Window>) -> Vec<Group> {
    let mut groups = Vec::<Group>::new();
    for window in windows {
        let overlap = groups.iter_mut().find(|entry| {
            entry.name == window.name
                && (entry.end <= 0 || window.start <= 0 || entry.end >= window.start)
                && (window.end <= 0 || entry.start <= 0 || window.end >= entry.start)
        });
        if let Some(entry) = overlap {
            entry.start = if entry.start > 0 && window.start > 0 {
                entry.start.min(window.start)
            } else {
                entry.start.max(window.start)
            };
            entry.end = entry.end.max(window.end);
            if !entry.id.ends_with("00") && window.id.ends_with("00") {
                entry.id = window.id.clone();
            }
            entry.ids.push(window.id);
            continue;
        }
        groups.push(Group {
            id: window.id.clone(),
            name: window.name,
            start: window.start,
            end: window.end,
            ids: vec![window.id],
        });
    }
    for entry in &mut groups {
        if entry.ids.iter().any(|id| {
            matches!(id.as_str(), "2026090100" | "2026090101" | "2026090102" | "2026090103")
        }) {
            entry.name = "萌宠成长日记".to_owned();
        }
    }
    groups
}

fn status(start: i64, end: i64, now: i64) -> &'static str {
    if end > 0 && now >= end {
        "ended"
    } else if start > 0 && now < start {
        "upcoming"
    } else {
        "active"
    }
}

fn status_rank(status: &str) -> i32 {
    match status {
        "active" => 0,
        "upcoming" => 1,
        "ended" => 2,
        _ => 3,
    }
}

fn format_range(start: i64, end: i64) -> String {
    match (start > 0, end > 0) {
        (true, true) => format!("{} – {}", stamp(start), stamp(end)),
        (true, false) => stamp(start),
        (false, true) => stamp(end),
        (false, false) => String::new(),
    }
}

fn stamp(secs: i64) -> String {
    let Some(utc) = chrono::DateTime::from_timestamp(secs, 0) else {
        return String::new();
    };
    let local = utc.with_timezone(&chrono_tz::Asia::Shanghai);
    format!("{:02}-{:02} {:02}:{:02}", local.month(), local.day(), local.hour(), local.minute())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(
        id: i64,
        name: &str,
        start: i64,
        end: i64,
        children: Vec<ActivityData>,
    ) -> ActivityData {
        ActivityData {
            activity: Some(ActivityContent {
                activity_id: id,
                name: name.to_owned(),
                begin_time: start,
                end_time: end,
                ..ActivityContent::default()
            }),
            children,
            ..ActivityData::default()
        }
    }

    #[test]
    fn directory_keeps_seconds_and_merges_overlapping_names() {
        let now = 1_700_000_050;
        let reply = ActivityListReply {
            activities: vec![activity(
                100,
                "牧草节",
                1_700_000_000,
                1_700_086_400,
                vec![activity(101, "牧草节", 1_700_000_100, 1_700_090_000, Vec::new())],
            )],
            ..ActivityListReply::default()
        };
        let page = directory(&reply, now);
        let rows = page.get("activities").and_then(Value::as_array).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("startTime").and_then(Value::as_i64), Some(1_700_000_000));
        assert_eq!(rows[0].get("endTime").and_then(Value::as_i64), Some(1_700_090_000));
        assert_eq!(rows[0].get("status").and_then(Value::as_str), Some("active"));
        assert!(rows[0].get("when").and_then(Value::as_str).unwrap_or("").contains("–"));
    }

    #[test]
    fn directory_sorts_active_before_upcoming_and_renames_the_pet_diary() {
        let now = 1_700_000_000;
        let reply = ActivityListReply {
            activities: vec![
                activity(2026090101, "日记", 1_699_000_000, 1_800_000_000, Vec::new()),
                activity(8, "即将开始", now + 100, now + 200, Vec::new()),
            ],
            ..ActivityListReply::default()
        };
        let page = directory(&reply, now);
        let rows = page.get("activities").and_then(Value::as_array).unwrap();
        assert_eq!(rows[0].get("name").and_then(Value::as_str), Some("萌宠成长日记"));
        assert_eq!(rows[0].get("status").and_then(Value::as_str), Some("active"));
        assert_eq!(rows[1].get("status").and_then(Value::as_str), Some("upcoming"));
    }
}
