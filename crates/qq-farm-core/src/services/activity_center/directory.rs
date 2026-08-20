//! 活动目录 + gameplay 绑定（对齐 bot `activity-gameplay-registry.ts`）。

use crate::config::activity_windows::ActivityWindow;

use super::{ConstellationDto, SeasonDto, SolarTermsDto, StarSandShopDto};

#[derive(Clone)]
struct GameplayBinding {
    gameplay_key: &'static str,
    detail_target: &'static str,
    priority: i32,
}

fn normalize_activity_id(value: impl ToString) -> String {
    let id = value.to_string().trim().to_string();
    if id.chars().all(|c| c.is_ascii_digit()) && id != "0" && !id.is_empty() {
        id
    } else {
        String::new()
    }
}

fn push_binding(
    bindings: &mut std::collections::HashMap<String, Vec<GameplayBinding>>,
    ids: impl IntoIterator<Item = String>,
    key: &'static str,
    target: &'static str,
    priority: i32,
) {
    for id in ids {
        let id = normalize_activity_id(id);
        if id.is_empty() {
            continue;
        }
        let list = bindings.entry(id).or_default();
        if list.iter().any(|b| b.gameplay_key == key && b.detail_target == target) {
            continue;
        }
        list.push(GameplayBinding { gameplay_key: key, detail_target: target, priority });
        list.sort_by_key(|b| b.priority);
    }
}

/// 把 List 窗口与当前快照活动绑成目录。
#[must_use]
pub fn build_activity_directory(
    windows: &[ActivityWindow],
    season: Option<&SeasonDto>,
    shop: Option<&StarSandShopDto>,
    solar_terms: Option<&SolarTermsDto>,
    constellation: Option<&ConstellationDto>,
    qixi: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let mut bindings = std::collections::HashMap::new();
    if let Some(pass) = season.and_then(|s| s.pass.as_ref()) {
        push_binding(&mut bindings, [pass.activity_id.to_string()], "stellar", "travel", 10);
    }
    if let Some(c) = constellation {
        push_binding(&mut bindings, [c.activity_id.to_string()], "stellar", "constellation", 20);
    }
    if let Some(s) = shop {
        push_binding(&mut bindings, [s.activity_id.to_string()], "stellar", "shop", 30);
    }
    let mut solar_ids = Vec::new();
    if let Some(solar) = solar_terms {
        if let Some(cfg) = solar.current_config.as_ref() {
            solar_ids.push(cfg.activity_id.to_string());
        }
        for cfg in &solar.configs {
            solar_ids.push(cfg.activity_id.to_string());
        }
    }
    push_binding(&mut bindings, solar_ids, "stellar", "solar", 40);
    push_binding(
        &mut bindings,
        [
            json_id(qixi, "groupId"),
            json_id(qixi, "bridgeActivityId"),
            json_id(qixi, "giftActivityId"),
        ],
        "qixi",
        "qixi",
        50,
    );

    struct Group {
        id: String,
        name: String,
        start: i64,
        end: i64,
        activity_ids: Vec<String>,
    }
    let mut groups: Vec<Group> = Vec::new();
    for window in windows {
        let id = window.id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        let name = if window.name.trim().is_empty() {
            format!("活动 {id}")
        } else {
            window.name.trim().to_string()
        };
        let matched = groups.iter().position(|g| {
            g.name == name
                && (g.end <= 0 || window.begin_time <= 0 || g.end >= window.begin_time)
                && (window.end_time <= 0 || g.start <= 0 || window.end_time >= g.start)
        });
        if let Some(idx) = matched {
            let g = &mut groups[idx];
            g.activity_ids.push(id.clone());
            if g.start > 0 && window.begin_time > 0 {
                if window.begin_time < g.start {
                    g.start = window.begin_time;
                }
            } else if window.begin_time > g.start {
                g.start = window.begin_time;
            }
            if window.end_time > g.end {
                g.end = window.end_time;
            }
            if !g.id.ends_with("00") && id.ends_with("00") {
                g.id = id;
            }
            continue;
        }
        groups.push(Group {
            id,
            name,
            start: window.begin_time,
            end: window.end_time,
            activity_ids: vec![window.id.trim().to_string()],
        });
    }

    groups
        .into_iter()
        .map(|g| {
            let mut matches = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for id in &g.activity_ids {
                if let Some(list) = bindings.get(id) {
                    for binding in list {
                        let key = format!("{}:{}", binding.gameplay_key, binding.detail_target);
                        if seen.insert(key) {
                            matches.push(binding.clone());
                        }
                    }
                }
            }
            matches.sort_by_key(|b| b.priority);
            let mut gameplay_keys = Vec::new();
            let mut targets = Vec::new();
            for m in &matches {
                if !gameplay_keys.iter().any(|k| *k == m.gameplay_key) {
                    gameplay_keys.push(m.gameplay_key);
                }
                targets.push(m.detail_target);
            }
            serde_json::json!({
                "id": g.id,
                "name": g.name,
                "startTime": g.start,
                "endTime": g.end,
                "activityIds": g.activity_ids,
                "gameplayKey": gameplay_keys.first().copied(),
                "gameplayKeys": gameplay_keys,
                "detailTarget": targets.first().copied(),
                "gameplayTargets": targets,
            })
        })
        .collect()
}

fn json_id(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default()
}
