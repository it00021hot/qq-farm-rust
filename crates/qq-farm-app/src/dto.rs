//! 第一批面板 DTO：status / lands / bag / friend / logs。

use std::collections::HashMap;

use qq_farm_core::runtime::runtime_state::ExpProgress;
use qq_farm_core::services::farm::land_analysis::LandDetailSummary;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use qq_farm_core::runtime::runtime_state::LogEntry as PanelLogEntry;
pub use qq_farm_core::services::friend::visit_strategy::{FriendPlantSummary, FriendSummary};
pub use qq_farm_core::services::warehouse::BagDetail;

/// 看板状态（扁平字段 + 保留嵌套 `status` 给 web）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelStatus {
    pub account_id: String,
    pub running: bool,
    pub online: bool,
    pub run_status: i64,
    #[serde(default)]
    pub nick: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub level: i64,
    #[serde(default)]
    pub exp: i64,
    #[serde(default)]
    pub gold: i64,
    #[serde(default)]
    pub land_count: Option<i64>,
    #[serde(default)]
    pub friend_count: Option<i64>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub uptime: Option<u64>,
    #[serde(default)]
    pub session_exp_gained: Option<i64>,
    #[serde(default)]
    pub session_gold_gained: Option<i64>,
    #[serde(default)]
    pub level_progress: Option<ExpProgress>,
    #[serde(default)]
    pub operations: HashMap<String, i64>,
    #[serde(default)]
    pub connection: Option<Value>,
    #[serde(default)]
    pub status: Option<Value>,
    #[serde(default)]
    pub ws_error: Option<String>,
    #[serde(default)]
    pub next_checks: Option<Value>,
}

impl PanelStatus {
    /// 从引擎 `panel_status` JSON 扁平化。
    #[must_use]
    pub fn from_engine_value(raw: &Value, account_id: &str, running: bool) -> Self {
        let nested = raw.get("status").cloned().unwrap_or(Value::Null);
        let connected = raw
            .pointer("/connection/connected")
            .and_then(Value::as_bool)
            .or_else(|| raw.get("online").and_then(Value::as_bool))
            .unwrap_or(false);
        let nick = nested
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| raw.get("nick").and_then(Value::as_str))
            .unwrap_or("")
            .to_string();
        let avatar = json_nonempty_str(&nested, &["avatar", "avatarUrl", "avatar_url"])
            .or_else(|| json_nonempty_str(raw, &["avatar", "avatarUrl", "avatar_url"]))
            .unwrap_or_default();
        let level_progress =
            raw.get("levelProgress").cloned().and_then(|v| serde_json::from_value(v).ok());
        let operations = raw
            .get("operations")
            .and_then(Value::as_object)
            .map(|m| m.iter().filter_map(|(k, v)| v.as_i64().map(|n| (k.clone(), n))).collect())
            .unwrap_or_default();
        let ws_error = raw
            .get("wsError")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        Self {
            account_id: account_id.to_string(),
            running,
            online: connected,
            run_status: if running { 1 } else { 0 },
            nick,
            avatar,
            level: raw
                .get("level")
                .and_then(Value::as_i64)
                .or_else(|| nested.get("level").and_then(Value::as_i64))
                .unwrap_or(0),
            exp: raw
                .get("exp")
                .and_then(Value::as_i64)
                .or_else(|| nested.get("exp").and_then(Value::as_i64))
                .unwrap_or(0),
            gold: raw
                .get("gold")
                .and_then(Value::as_i64)
                .or_else(|| nested.get("gold").and_then(Value::as_i64))
                .unwrap_or(0),
            land_count: raw.get("landCount").and_then(Value::as_i64),
            friend_count: raw.get("friendCount").and_then(Value::as_i64),
            last_error: raw
                .get("lastError")
                .or_else(|| raw.get("wsError"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            uptime: raw.get("uptime").and_then(Value::as_u64),
            session_exp_gained: raw.get("sessionExpGained").and_then(Value::as_i64),
            session_gold_gained: raw.get("sessionGoldGained").and_then(Value::as_i64),
            level_progress,
            operations,
            connection: raw.get("connection").cloned(),
            status: raw.get("status").cloned(),
            ws_error,
            next_checks: raw.get("nextChecks").cloned(),
        }
    }
}

/// 地块行（对齐 land_analysis 面板 JSON）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LandRow {
    pub id: i64,
    #[serde(default)]
    pub unlocked: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub plant_name: Option<String>,
    #[serde(default)]
    pub seed_id: Option<i64>,
    #[serde(default)]
    pub seed_image: Option<String>,
    #[serde(default)]
    pub phase_name: Option<String>,
    #[serde(default)]
    pub current_season: Option<i64>,
    #[serde(default)]
    pub total_season: Option<i64>,
    #[serde(default)]
    pub mature_in_sec: Option<i64>,
    #[serde(default)]
    pub total_grow_time: Option<i64>,
    #[serde(default)]
    pub need_water: bool,
    #[serde(default)]
    pub need_weed: bool,
    #[serde(default)]
    pub need_bug: bool,
    #[serde(default)]
    pub level: i64,
    #[serde(default)]
    pub occupied_by_master: bool,
    #[serde(default)]
    pub master_land_id: i64,
    #[serde(default)]
    pub occupied_land_ids: Vec<i64>,
    #[serde(default)]
    pub plant_size: Option<i64>,
    #[serde(default)]
    pub harvestable: bool,
    #[serde(default)]
    pub stealable: Option<bool>,
    #[serde(default)]
    pub max_level: Option<i64>,
    #[serde(default)]
    pub lands_level: Option<i64>,
    #[serde(default)]
    pub land_size: Option<i64>,
    #[serde(default)]
    pub could_unlock: Option<bool>,
    #[serde(default)]
    pub could_upgrade: Option<bool>,
}

/// 地块列表 + 汇总。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandsPayload {
    pub lands: Vec<LandRow>,
    pub summary: LandDetailSummary,
}

impl LandsPayload {
    #[must_use]
    pub fn from_values(lands: Vec<Value>, summary: LandDetailSummary) -> Self {
        let rows = lands.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect();
        Self { lands: rows, summary }
    }
}

pub fn friend_summaries_from_values(values: Vec<Value>) -> Vec<FriendSummary> {
    values.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect()
}

pub use BagDetail as PanelBagDetail;
pub use FriendSummary as PanelFriendSummary;

fn json_nonempty_str(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| {
        v.get(*k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_engine_value_reads_nested_avatar() {
        let raw = json!({
            "connection": { "connected": true },
            "status": {
                "name": "bob",
                "avatar": "https://cdn.example/a.png",
                "level": 3,
                "exp": 10,
                "gold": 20
            }
        });
        let status = PanelStatus::from_engine_value(&raw, "acc1", true);
        assert_eq!(status.avatar, "https://cdn.example/a.png");
        assert_eq!(status.nick, "bob");
    }

    #[test]
    fn from_engine_value_reads_avatar_url_alias() {
        let raw = json!({
            "status": { "avatarUrl": "https://cdn.example/b.png" }
        });
        let status = PanelStatus::from_engine_value(&raw, "acc1", false);
        assert_eq!(status.avatar, "https://cdn.example/b.png");
    }
}
