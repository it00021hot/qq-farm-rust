//! 土地加成与升级/解锁条件详情。
//!
//! 1:1 翻译原 `core/src/services/farm/land-upgrade.ts`（bot `e5f3552`，2026-10-02）。
//!
//! 数据来源优先级与 bot 一致：
//! - 当前等级加成：服务器随 `LandInfo.buff` 下发的实时加成，缺省回落官方 `LandLevel.json`；
//! - 升级条件：服务器随 `LandInfo.upgrade_condition` 下发（condition_type=1 为等级 +
//!   required_items），缺省回落官方 `LandLevelNeed.json`（land_id + level 定位）。

use std::sync::OnceLock;

use serde::Deserialize;

use crate::config::game_config::global as global_game_config;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::plantpb::LandInfo;
use crate::services::farm::land_analysis::LandMap;

const LAND_LEVEL_JSON: &str = include_str!("../../../../../assets/game_config/LandLevel.json");
const LAND_LEVEL_NEED_JSON: &str =
    include_str!("../../../../../assets/game_config/LandLevelNeed.json");

/// `LandLevel.json` 单条（官方静态配置；只取详情用到的字段）
#[derive(Debug, Deserialize)]
struct LandLevelEntry {
    id: i64,
    #[serde(default)]
    level_name: Option<String>,
    #[serde(default)]
    plant_yield_bonus: Option<i64>,
    #[serde(default)]
    planting_time_reduction: Option<i64>,
    #[serde(default)]
    plant_exp_bonus: Option<i64>,
    #[serde(default)]
    plant_mutant_bonus: Option<i64>,
    #[serde(default)]
    plant_mutant_exp_bonus: Option<i64>,
}

/// `LandLevelNeed.json` 单条：升级到 `level` 需要的等级与材料
#[derive(Debug, Deserialize)]
struct LandLevelNeedEntry {
    land_id: i64,
    level: i64,
    level_need: Option<i64>,
    #[serde(default)]
    items_need: Option<String>,
}

fn land_levels() -> &'static Vec<LandLevelEntry> {
    static CACHE: OnceLock<Vec<LandLevelEntry>> = OnceLock::new();
    CACHE.get_or_init(|| serde_json::from_str(LAND_LEVEL_JSON).expect("LandLevel.json 解析失败"))
}

fn land_level_needs() -> &'static Vec<LandLevelNeedEntry> {
    static CACHE: OnceLock<Vec<LandLevelNeedEntry>> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str(LAND_LEVEL_NEED_JSON).expect("LandLevelNeed.json 解析失败")
    })
}

/// bot `amount`：非负整数字面量原样转字符串，否则按 "0"
fn amount(count: i64) -> String {
    if count >= 0 {
        count.to_string()
    } else {
        "0".to_string()
    }
}

/// bot `ratio`：把万分比配置除成面板数值；整数值省略小数位（对齐 JS number 序列化）
fn ratio(value: i64, divisor: f64) -> serde_json::Value {
    let x = value as f64 / divisor;
    if x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        serde_json::json!(x as i64)
    } else {
        serde_json::json!(x)
    }
}

/// 某等级的加成面板（对齐 TS `levelInfo`）。
///
/// buff 覆盖语义：bot 用 protobufjs 的字段 presence（`buff?.plant_yield_bonus ?? config`），
/// prost 的标量无法区分「缺省 0」与「显式 0」，这里近似为——`LandInfo.buff` 消息存在则
/// 整体采用其字段，否则回落官方配置（实践上服务器总是下发完整 buff）。
fn level_info(
    level: i64,
    buff: Option<&crate::proto::generated::gamepb::plantpb::land_info::Buff>,
) -> Option<serde_json::Value> {
    let config = land_levels().iter().find(|entry| entry.id == level)?;
    let buff = buff;
    let pick = |buff_value: i64, config_value: Option<i64>| -> i64 {
        match buff {
            Some(_) => buff_value,
            None => config_value.unwrap_or(0),
        }
    };
    Some(serde_json::json!({
        "level": level,
        "name": config.level_name.clone().unwrap_or_default(),
        "image": format!("/land-assets/{level}.png"),
        "yieldBonus": ratio(pick(
            buff.map_or(0, |b| b.plant_yield_bonus),
            config.plant_yield_bonus,
        ), 100.0),
        "growthReduction": ratio(pick(
            buff.map_or(0, |b| b.planting_time_reduction),
            config.planting_time_reduction,
        ), 100.0),
        "expBonus": ratio(pick(
            buff.map_or(0, |b| b.plant_exp_bonus),
            config.plant_exp_bonus,
        ), 100.0),
        "mutantMultiplier": ratio(config.plant_mutant_bonus.unwrap_or(0), 10_000.0),
        "resonanceBonus": ratio(config.plant_mutant_exp_bonus.unwrap_or(0), 100.0),
    }))
}

/// 成本条目（对齐 TS `costs`）：过滤无效项并补名称/图标
fn cost_entries(entries: impl IntoIterator<Item = (i64, i64)>) -> Vec<serde_json::Value> {
    let gc = global_game_config();
    entries
        .into_iter()
        .filter(|(id, count)| *id > 0 && *count > 0)
        .map(|(id, count)| {
            let name =
                gc.get_item_by_id(id).map(|item| item.name).unwrap_or_else(|| format!("物品 {id}"));
            serde_json::json!({
                "id": id.to_string(),
                "count": amount(count),
                "name": name,
                "image": gc.get_seed_image_by_seed_id(id).unwrap_or_default(),
            })
        })
        .collect()
}

/// proto `required_items` → (id, count)
fn proto_items(items: &[Item]) -> Vec<(i64, i64)> {
    items.iter().map(|item| (item.id, item.count)).collect()
}

/// 解析官方 `LandLevelNeed.items_need`（`"1001:200000;"` 分号/冒号分隔）
fn parse_items_need(items_need: &str) -> Vec<(i64, i64)> {
    items_need
        .split(';')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (id, count) = part.split_once(':')?;
            Some((id.parse::<i64>().ok()?, count.parse::<i64>().ok()?))
        })
        .collect()
}

/// 构造土地升级/解锁详情（对齐 TS `buildLandUpgradeDetail`）。
///
/// - 未解锁 → `mode: "unlock"`：解锁等级、前置地块、金币成本；
/// - 已满级 → `mode: "max"`：只展示当前加成；
/// - 其余 → `mode: "upgrade"`：当前/下一级加成对比 + 服务器（或官方配置回退）升级成本。
///
/// `lands_map` 用于判定前置地块是否已解锁（好友模式不调用本函数）。
#[must_use]
pub fn build_land_upgrade_detail(land: &LandInfo, lands_map: &LandMap) -> serde_json::Value {
    let id = land.id;
    let level = land.level;
    let max_level = land.max_level;
    let current = if land.unlocked { level_info(level, land.buff.as_ref()) } else { None };

    if !land.unlocked {
        let condition = land.unlock_condition.as_ref();
        let preceding_id = condition.map(|c| c.preceding_land_id).unwrap_or(0);
        let costs = cost_entries(vec![(1_001, condition.map_or(0, |c| c.need_gold))]);
        return serde_json::json!({
            "mode": "unlock",
            "current": current,
            "next": level_info(1, None),
            "serverAllowed": land.could_unlock,
            "needLevel": condition.map(|c| c.need_level),
            "prerequisite": if preceding_id != 0 {
                serde_json::json!({
                    "landId": preceding_id,
                    "met": lands_map.get(&preceding_id).is_some_and(|l| l.unlocked),
                })
            } else {
                serde_json::Value::Null
            },
            "costs": costs,
            "unknown": condition.is_none(),
        });
    }

    if max_level > 0 && level >= max_level {
        return serde_json::json!({
            "mode": "max",
            "current": current,
            "next": null,
            "costs": [],
            "unknown": current.is_none(),
        });
    }

    let next_level = level + 1;
    let next = level_info(next_level, None);
    let condition = land.upgrade_condition.as_ref();
    let config =
        land_level_needs().iter().find(|entry| entry.land_id == id && entry.level == next_level);
    let config_items = config
        .and_then(|entry| entry.items_need.as_deref())
        .map(parse_items_need)
        .unwrap_or_default();
    let condition_type = condition.map_or(0, |c| c.condition_type);
    let need_level = match condition {
        Some(c) => {
            if condition_type == 1 {
                Some(c.condition_value)
            } else {
                None
            }
        }
        None => config.and_then(|entry| entry.level_need),
    };
    let costs = match condition {
        Some(c) => cost_entries(proto_items(&c.required_items)),
        None => cost_entries(config_items),
    };
    let unknown = next.is_none()
        || match condition {
            Some(_) => condition_type != 1,
            None => config.is_none(),
        };
    serde_json::json!({
        "mode": "upgrade",
        "current": current,
        "next": next,
        "serverAllowed": land.could_upgrade,
        "needLevel": need_level,
        "prerequisite": null,
        "costs": costs,
        "unknown": unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::proto::generated::gamepb::plantpb::land_info::Buff;
    use crate::proto::generated::gamepb::plantpb::{LandUnlockCondition, LandUpgradeCondition};

    fn land(id: i64) -> LandInfo {
        LandInfo { id, ..Default::default() }
    }

    /// bot 用例 1：服务器条件优先，加成按 buff 覆盖，金币与金豆并存
    #[test]
    fn server_conditions_and_currency_costs() {
        let mut l = land(21);
        l.unlocked = true;
        l.level = 4;
        l.max_level = 5;
        l.buff = Some(Buff {
            plant_yield_bonus: 30_000,
            planting_time_reduction: 2_000,
            plant_exp_bonus: 2_000,
        });
        l.upgrade_condition = Some(LandUpgradeCondition {
            condition_type: 1,
            condition_value: 150,
            required_items: vec![
                Item { id: 1_001, count: 450_000_000, ..Default::default() },
                Item { id: 1_005, count: 18_670, ..Default::default() },
            ],
        });
        let info = build_land_upgrade_detail(&l, &HashMap::new());
        assert_eq!(info["mode"], "upgrade");
        assert_eq!(info["needLevel"], 150);
        let costs = info["costs"].as_array().unwrap();
        assert_eq!(costs.len(), 2);
        assert_eq!(costs[0]["id"], "1001");
        assert_eq!(costs[0]["count"], "450000000");
        assert_eq!(costs[1]["id"], "1005");
        assert_eq!(costs[1]["count"], "18670");
        assert_eq!(info["current"]["expBonus"], 20);
        assert_eq!(info["next"]["expBonus"], 25);
        assert_eq!(info["next"]["mutantMultiplier"], 1.2);
        assert_eq!(info["next"]["resonanceBonus"], 25);
        assert_eq!(info["unknown"], false);
    }

    /// bot 用例 2：满级不展示升级成本；服务器加成覆盖静态默认
    #[test]
    fn max_level_has_no_costs_and_buff_overrides_config() {
        let mut l = land(21);
        l.unlocked = true;
        l.level = 5;
        l.max_level = 5;
        l.buff = Some(Buff { plant_exp_bonus: 5_000, ..Default::default() });
        let info = build_land_upgrade_detail(&l, &HashMap::new());
        assert_eq!(info["mode"], "max");
        assert_eq!(info["current"]["expBonus"], 50);
        assert!(info["next"].is_null());
        assert_eq!(info["costs"].as_array().unwrap().len(), 0);
    }

    /// bot 用例 3：解锁态展示前置地块与精确金币
    #[test]
    fn unlock_shows_prerequisite_and_price() {
        let mut raw = land(8);
        raw.unlock_condition =
            Some(LandUnlockCondition { preceding_land_id: 7, need_level: 10, need_gold: 100_000 });
        let mut map = HashMap::new();
        let mut prev = land(7);
        prev.unlocked = true;
        map.insert(7, prev);
        let info = build_land_upgrade_detail(&raw, &map);
        assert_eq!(info["mode"], "unlock");
        assert_eq!(info["needLevel"], 10);
        assert_eq!(info["prerequisite"]["met"], true);
        assert_eq!(info["costs"][0]["count"], "100000");

        let info = build_land_upgrade_detail(&raw, &HashMap::new());
        assert_eq!(info["prerequisite"]["met"], false);
    }

    /// bot 用例 4：未知条件类型保持 unknown；官方配置回退不暗示可升级
    #[test]
    fn unknown_condition_type_stays_unknown_and_fallback_uses_config() {
        let mut l = land(21);
        l.unlocked = true;
        l.level = 4;
        l.max_level = 5;
        l.upgrade_condition = Some(LandUpgradeCondition {
            condition_type: 99,
            condition_value: 150,
            required_items: Vec::new(),
        });
        let info = build_land_upgrade_detail(&l, &HashMap::new());
        assert_eq!(info["unknown"], true);
        assert!(info["needLevel"].is_null());

        let mut fallback = land(21);
        fallback.unlocked = true;
        fallback.level = 4;
        fallback.max_level = 5;
        let info = build_land_upgrade_detail(&fallback, &HashMap::new());
        assert_eq!(info["needLevel"], 150);
        assert_eq!(info["serverAllowed"], false);
        assert_eq!(info["costs"][1]["count"], "18670");
    }
}
