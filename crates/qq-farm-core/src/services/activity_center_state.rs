//! 活动中心状态持久化（1:1 对应原 `services/activity-center-state.ts` 256 行）。
//!
//! 维护每账号的活动中心（星座 / 战令 / 赛季 / 充值）状态合并：
//! - 内存合并（in-flight）：`mergeConstellationStates`
//! - 持久化（按账号 SHA256 文件名）：`loadConstellationState` / `persistConstellationState`
//! - 从动态节点构建状态：`stateFromDynamicNodes`
//! - 标记"无可领取"日：`stateWithNoClaimableDay`

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::config::paths::get_data_file;
use crate::services::json_db::{read_json_or, write_text_file_atomic};

/// 状态文件版本号（必须 == 1，rust 端 schema 改变时升 2）
pub const STATE_FILE_VERSION: i32 = 1;
/// 状态文件名前缀
pub const STATE_FILE_PREFIX: &str = "activity-center-state";

/// 活动状态身份（season + activity + catalog version）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ActivityStateIdentity {
    #[serde(default, rename = "seasonId")]
    pub season_id: String,
    #[serde(default, rename = "activityId")]
    pub activity_id: String,
    #[serde(default, rename = "catalogVersion")]
    pub catalog_version: i32,
}

/// 单日"无可领取"观察记录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoClaimableDayObservation {
    #[serde(rename = "observedAt")]
    pub observed_at: String,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

/// 星座活动状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConstellationActivityState {
    #[serde(flatten)]
    pub identity: ActivityStateIdentity,
    #[serde(default, rename = "confirmedOpenedNodeIds")]
    pub confirmed_opened_node_ids: Vec<String>,
    #[serde(default, rename = "confirmedLitNodeIds")]
    pub confirmed_lit_node_ids: Vec<String>,
    #[serde(default, rename = "noClaimableDays")]
    pub no_claimable_days: BTreeMap<String, NoClaimableDayObservation>,
}

/// 状态文件
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActivityCenterStateFile {
    pub version: i32,
    #[serde(default)]
    pub records: BTreeMap<String, ConstellationActivityState>,
}

/// 文件路径选项（测试可注入）
#[derive(Debug, Clone, Default)]
pub struct StateFileOptions {
    pub file_path: Option<String>,
}

// ===== normalize helpers =====

/// 字符串 → digit-only id（不是纯数字返回空）
pub fn normalize_id(value: Option<&str>) -> String {
    let s = value.unwrap_or("").trim();
    if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
        s.to_string()
    } else {
        String::new()
    }
}

pub fn normalize_catalog_version(value: Option<i64>) -> i32 {
    match value {
        Some(v) if (1..=i32::MAX as i64).contains(&v) => v as i32,
        _ => 0,
    }
}

pub fn normalize_identity(identity: Option<&ActivityStateIdentity>) -> ActivityStateIdentity {
    ActivityStateIdentity {
        season_id: normalize_id(identity.and_then(|i| i.season_id.as_str().into())),
        activity_id: normalize_id(identity.and_then(|i| i.activity_id.as_str().into())),
        catalog_version: normalize_catalog_version(Some(
            identity.map(|i| i.catalog_version as i64).unwrap_or(0),
        )),
    }
}

pub fn create_empty_constellation_state(
    identity: &ActivityStateIdentity,
) -> ConstellationActivityState {
    ConstellationActivityState {
        identity: normalize_identity(Some(identity)),
        confirmed_opened_node_ids: Vec::new(),
        confirmed_lit_node_ids: Vec::new(),
        no_claimable_days: BTreeMap::new(),
    }
}

/// node_ids 去重 + 数字升序
pub fn normalize_node_ids(values: &[String]) -> Vec<String> {
    let mut set: BTreeSet<u64> = BTreeSet::new();
    for v in values {
        if let Ok(n) = v.parse::<u64>() {
            if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) {
                set.insert(n);
            }
        }
    }
    set.into_iter().map(|n| n.to_string()).collect()
}

pub fn normalize_no_claimable_days(
    value: serde_json::Value,
) -> BTreeMap<String, NoClaimableDayObservation> {
    let mut out = BTreeMap::new();
    if let Some(map) = value.as_object() {
        for (raw_day, raw_observation) in map {
            let day = match raw_day.parse::<i64>() {
                Ok(d) if (1..=28).contains(&d) => d,
                _ => continue,
            };
            let obs = match raw_observation.as_object() {
                Some(o) => o,
                None => continue,
            };
            let observed_at =
                obs.get("observedAt").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            let server_time = normalize_id(obs.get("serverTime").and_then(|v| v.as_str()).into());
            if observed_at.is_empty() || server_time.is_empty() {
                continue;
            }
            out.insert(day.to_string(), NoClaimableDayObservation { observed_at, server_time });
        }
    }
    out
}

pub fn normalize_constellation_state(
    value: serde_json::Value,
    identity: &ActivityStateIdentity,
) -> ConstellationActivityState {
    let expected = normalize_identity(Some(identity));
    let Some(obj) = value.as_object() else {
        return create_empty_constellation_state(&expected);
    };
    let actual = ActivityStateIdentity {
        season_id: obj
            .get("seasonId")
            .or_else(|| obj.get("season_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default(),
        activity_id: obj
            .get("activityId")
            .or_else(|| obj.get("activity_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default(),
        catalog_version: obj
            .get("catalogVersion")
            .or_else(|| obj.get("catalog_version"))
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(0),
    };
    if actual.season_id != expected.season_id
        || actual.activity_id != expected.activity_id
        || actual.catalog_version != expected.catalog_version
    {
        return create_empty_constellation_state(&expected);
    }
    ConstellationActivityState {
        identity: expected,
        confirmed_opened_node_ids: normalize_node_ids(
            &obj.get("confirmedOpenedNodeIds")
                .or_else(|| obj.get("confirmed_opened_node_ids"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        ),
        confirmed_lit_node_ids: normalize_node_ids(
            &obj.get("confirmedLitNodeIds")
                .or_else(|| obj.get("confirmed_lit_node_ids"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        ),
        no_claimable_days: normalize_no_claimable_days(
            obj.get("noClaimableDays")
                .or_else(|| obj.get("no_claimable_days"))
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default())),
        ),
    }
}

/// 多源合并：opened ∪ lit（lit 包含 opened），noClaimableDays 取 serverTime 最大
pub fn merge_constellation_states(
    identity: &ActivityStateIdentity,
    states: &[serde_json::Value],
) -> ConstellationActivityState {
    let expected = normalize_identity(Some(identity));
    let mut opened: BTreeSet<u64> = BTreeSet::new();
    let mut lit: BTreeSet<u64> = BTreeSet::new();
    let mut no_claimable_days: BTreeMap<String, NoClaimableDayObservation> = BTreeMap::new();

    for s in states {
        let state = normalize_constellation_state(s.clone(), &expected);
        for id in &state.confirmed_opened_node_ids {
            if let Ok(n) = id.parse::<u64>() {
                opened.insert(n);
            }
        }
        for id in &state.confirmed_lit_node_ids {
            if let Ok(n) = id.parse::<u64>() {
                lit.insert(n);
                opened.insert(n);
            }
        }
        for (day, observation) in &state.no_claimable_days {
            let replace = match no_claimable_days.get(day) {
                Some(existing) => {
                    let new_t: u64 = observation.server_time.parse().unwrap_or(0);
                    let old_t: u64 = existing.server_time.parse().unwrap_or(0);
                    new_t >= old_t
                }
                None => true,
            };
            if replace {
                no_claimable_days.insert(day.clone(), observation.clone());
            }
        }
    }

    ConstellationActivityState {
        identity: expected,
        confirmed_opened_node_ids: opened.iter().map(|n| n.to_string()).collect(),
        confirmed_lit_node_ids: lit.iter().map(|n| n.to_string()).collect(),
        no_claimable_days,
    }
}

pub fn state_record_key(identity: &ActivityStateIdentity) -> String {
    let n = normalize_identity(Some(identity));
    format!("{}:{}:v{}", n.season_id, n.activity_id, n.catalog_version)
}

pub fn resolve_account_id(account_id: Option<&str>) -> String {
    let from_env = std::env::var("FARM_ACCOUNT_ID").ok();
    let raw = account_id.map(String::from).or(from_env).unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "default".to_string()
    } else {
        trimmed.to_string()
    }
}

// re-export for tests
pub use resolve_account_id as resolve_account_id_export;

fn safe_account_file_token(account_id: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(resolve_account_id(account_id).as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn get_activity_center_state_file(
    account_id: Option<&str>,
    options: &StateFileOptions,
) -> String {
    if let Some(p) = &options.file_path {
        return p.clone();
    }
    let token = safe_account_file_token(account_id);
    get_data_file(&format!("{STATE_FILE_PREFIX}-{token}.json")).to_string_lossy().into_owned()
}

fn empty_state_file() -> ActivityCenterStateFile {
    ActivityCenterStateFile { version: STATE_FILE_VERSION, records: BTreeMap::new() }
}

pub fn normalize_state_file(value: serde_json::Value) -> ActivityCenterStateFile {
    let Some(obj) = value.as_object() else {
        return empty_state_file();
    };
    let version = obj.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
    if version != STATE_FILE_VERSION as i64 {
        return empty_state_file();
    }
    let records_raw =
        obj.get("records").cloned().unwrap_or(serde_json::Value::Object(Default::default()));
    let Some(rec_obj) = records_raw.as_object() else {
        return empty_state_file();
    };
    let mut records = BTreeMap::new();
    for (k, v) in rec_obj {
        if let Ok(state) = serde_json::from_value::<ConstellationActivityState>(v.clone()) {
            records.insert(k.clone(), state);
        }
    }
    ActivityCenterStateFile { version: STATE_FILE_VERSION, records }
}

pub fn load_constellation_state(
    identity: &ActivityStateIdentity,
    account_id: Option<&str>,
    options: &StateFileOptions,
) -> ConstellationActivityState {
    let file =
        normalize_state_file(read_json_or(&get_activity_center_state_file(account_id, options)));
    let key = state_record_key(identity);
    let raw = serde_json::to_value(file.records.get(&key)).unwrap_or(serde_json::Value::Null);
    normalize_constellation_state(raw, identity)
}

pub fn persist_constellation_state(
    state_value: serde_json::Value,
    identity: &ActivityStateIdentity,
    account_id: Option<&str>,
    options: &StateFileOptions,
) -> ConstellationActivityState {
    let file_path = get_activity_center_state_file(account_id, options);
    let raw = read_json_or(&file_path);
    let mut file = normalize_state_file(raw);
    let key = state_record_key(identity);
    let existing_value =
        serde_json::to_value(file.records.get(&key)).unwrap_or(serde_json::Value::Null);
    let merged = merge_constellation_states(identity, &[existing_value, state_value]);
    file.records.insert(key, merged.clone());
    if let Ok(text) = serde_json::to_string_pretty(&file) {
        let _ = write_text_file_atomic(&file_path, &text);
    }
    merged
}

fn json_node_id(node: &serde_json::Value) -> Option<String> {
    let value = node.get("node_id").or_else(|| node.get("nodeId")).or_else(|| node.get("id"))?;
    if let Some(s) = value.as_str() {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    } else if let Some(n) = value.as_i64() {
        Some(n.to_string())
    } else if let Some(n) = value.as_u64() {
        Some(n.to_string())
    } else {
        None
    }
}

pub fn state_from_dynamic_nodes(
    identity: &ActivityStateIdentity,
    nodes: serde_json::Value,
) -> ConstellationActivityState {
    let mut opened: Vec<String> = Vec::new();
    let mut lit: Vec<String> = Vec::new();
    if let Some(arr) = nodes.as_array() {
        for node in arr {
            let id_str = json_node_id(node);
            let Some(id) = id_str else { continue };
            if !id.chars().all(|c| c.is_ascii_digit()) || id.is_empty() {
                continue;
            }
            let opened_flag = node
                .get("field_2")
                .or_else(|| node.get("field2"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let lit_flag = node
                .get("field_3")
                .or_else(|| node.get("field3"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if opened_flag {
                opened.push(id.clone());
            }
            if lit_flag {
                opened.push(id.clone());
                lit.push(id);
            }
        }
    }
    let dynamic_state = ConstellationActivityState {
        identity: normalize_identity(Some(identity)),
        confirmed_opened_node_ids: opened,
        confirmed_lit_node_ids: lit,
        no_claimable_days: BTreeMap::new(),
    };
    merge_constellation_states(
        identity,
        &[serde_json::to_value(dynamic_state).unwrap_or(serde_json::Value::Null)],
    )
}

pub fn state_with_no_claimable_day(
    identity: &ActivityStateIdentity,
    day: i64,
    server_time: &str,
    observed_at: Option<&str>,
) -> ConstellationActivityState {
    let normalized_day = day;
    let mut day_state = create_empty_constellation_state(identity);
    if (1..=28).contains(&normalized_day) {
        let observed = observed_at.map(String::from).unwrap_or_else(|| chrono_like_now_iso());
        day_state.no_claimable_days.insert(
            normalized_day.to_string(),
            NoClaimableDayObservation {
                observed_at: observed,
                server_time: normalize_id(Some(server_time)),
            },
        );
    }
    normalize_constellation_state(
        serde_json::to_value(day_state).unwrap_or(serde_json::Value::Null),
        identity,
    )
}

fn chrono_like_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    // 简化：原 TS `new Date().toISOString()`，我们用 RFC3339
    format_unix_to_iso8601(now)
}

fn format_unix_to_iso8601(secs: i64) -> String {
    // 1970-01-01T00:00:00Z 简化格式（不严格按月份计算，但单测可 mock）
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let h = secs_of_day / 3_600;
    let m = (secs_of_day % 3_600) / 60;
    let s = secs_of_day % 60;
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z",)
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    // Howard Hinnant date algorithm
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_id_digit_only() {
        assert_eq!(normalize_id(Some("123")), "123");
        assert_eq!(normalize_id(Some("")), "");
        assert_eq!(normalize_id(None), "");
        assert_eq!(normalize_id(Some("abc")), "");
        assert_eq!(normalize_id(Some("12a")), "");
        assert_eq!(normalize_id(Some("  456  ")), "456");
    }

    #[test]
    fn normalize_catalog_version_bounds() {
        assert_eq!(normalize_catalog_version(Some(0)), 0);
        assert_eq!(normalize_catalog_version(Some(1)), 1);
        assert_eq!(normalize_catalog_version(Some(i32::MAX as i64)), i32::MAX);
        // 超过 i32::MAX 截断为 0（原 TS Number.isSafeInteger 在 JS 是双精度浮点，等价 i53，这里不模拟）
        assert_eq!(normalize_catalog_version(Some(i64::MAX)), 0);
        assert_eq!(normalize_catalog_version(None), 0);
    }

    #[test]
    fn empty_state_init() {
        let id = ActivityStateIdentity {
            season_id: "1".into(),
            activity_id: "2".into(),
            catalog_version: 1,
        };
        let s = create_empty_constellation_state(&id);
        assert!(s.confirmed_opened_node_ids.is_empty());
        assert!(s.confirmed_lit_node_ids.is_empty());
        assert!(s.no_claimable_days.is_empty());
    }

    #[test]
    fn merge_opened_and_lit() {
        let id = ActivityStateIdentity {
            season_id: "10".into(),
            activity_id: "20".into(),
            catalog_version: 1,
        };
        let a = serde_json::json!({
            "seasonId": "10",
            "activityId": "20",
            "catalogVersion": 1,
            "confirmedOpenedNodeIds": ["1", "2"],
            "confirmedLitNodeIds": [],
            "noClaimableDays": {},
        });
        let b = serde_json::json!({
            "seasonId": "10",
            "activityId": "20",
            "catalogVersion": 1,
            "confirmedOpenedNodeIds": ["3"],
            "confirmedLitNodeIds": ["2", "3"],
            "noClaimableDays": {},
        });
        let merged = merge_constellation_states(&id, &[a, b]);
        assert_eq!(merged.confirmed_opened_node_ids, vec!["1", "2", "3"]);
        assert_eq!(merged.confirmed_lit_node_ids, vec!["2", "3"]);
    }

    #[test]
    fn state_record_key_format() {
        let id = ActivityStateIdentity {
            season_id: "100".into(),
            activity_id: "200".into(),
            catalog_version: 3,
        };
        assert_eq!(state_record_key(&id), "100:200:v3");
    }

    #[test]
    fn resolve_account_id_fallback() {
        assert_eq!(resolve_account_id(Some("alice")), "alice");
        assert_eq!(resolve_account_id(None), "default");
        assert_eq!(resolve_account_id(Some("")), "default");
        assert_eq!(resolve_account_id(Some("   ")), "default");
    }

    #[test]
    fn state_with_no_claimable_day_valid() {
        let id = ActivityStateIdentity {
            season_id: "1".into(),
            activity_id: "1".into(),
            catalog_version: 1,
        };
        let s = state_with_no_claimable_day(&id, 5, "123456", Some("2026-01-01T00:00:00Z"));
        assert!(s.no_claimable_days.contains_key("5"));
        assert_eq!(s.no_claimable_days.get("5").unwrap().server_time, "123456");
    }

    #[test]
    fn state_with_no_claimable_day_out_of_range() {
        let id = ActivityStateIdentity {
            season_id: "1".into(),
            activity_id: "1".into(),
            catalog_version: 1,
        };
        let s = state_with_no_claimable_day(&id, 30, "123", None);
        assert!(s.no_claimable_days.is_empty());
    }

    #[test]
    fn state_from_dynamic_nodes_extracts() {
        let id = ActivityStateIdentity {
            season_id: "1".into(),
            activity_id: "1".into(),
            catalog_version: 1,
        };
        let nodes = serde_json::json!([
            {"node_id": "1", "field_2": true, "field_3": false},
            {"node_id": "2", "field_2": false, "field_3": true},
            {"node_id": "3"},
        ]);
        let s = state_from_dynamic_nodes(&id, nodes);
        assert!(s.confirmed_opened_node_ids.contains(&"1".to_string()));
        assert!(s.confirmed_opened_node_ids.contains(&"2".to_string()));
        assert!(s.confirmed_lit_node_ids.contains(&"2".to_string()));
        assert!(!s.confirmed_opened_node_ids.contains(&"3".to_string()));
    }

    #[test]
    fn normalize_constellation_state_identity_mismatch() {
        let id = ActivityStateIdentity {
            season_id: "1".into(),
            activity_id: "2".into(),
            catalog_version: 1,
        };
        let v = serde_json::json!({
            "seasonId": "99",
            "activityId": "99",
            "catalogVersion": 1,
            "confirmedOpenedNodeIds": ["1"],
        });
        let s = normalize_constellation_state(v, &id);
        assert!(s.confirmed_opened_node_ids.is_empty());
    }

    #[test]
    fn no_claimable_merge_picks_latest_server_time() {
        let id = ActivityStateIdentity {
            season_id: "1".into(),
            activity_id: "1".into(),
            catalog_version: 1,
        };
        let a = serde_json::json!({
            "seasonId": "1",
            "activityId": "1",
            "catalogVersion": 1,
            "noClaimableDays": {"3": {"observedAt": "a", "serverTime": "100"}},
        });
        let b = serde_json::json!({
            "seasonId": "1",
            "activityId": "1",
            "catalogVersion": 1,
            "noClaimableDays": {"3": {"observedAt": "b", "serverTime": "200"}},
        });
        let merged = merge_constellation_states(&id, &[a, b]);
        assert_eq!(merged.no_claimable_days.get("3").unwrap().server_time, "200");
    }
}
