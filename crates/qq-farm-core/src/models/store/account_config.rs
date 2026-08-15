//! 单账号配置管理。
//!
//! 1:1 翻译原 `core/src/models/store/account-config.ts`（425 行）的核心 API。
//!
//! ## 状态
//!
//! 内存中持有 `GlobalConfig`，包含：
//! - `accountConfigs: HashMap<accountId, AccountConfig>` 各账号配置
//! - `defaultAccountConfig: AccountConfig` 全局默认
//! - `accountFallbackConfig: AccountConfig` 回退（账号未注册时用）
//!
//! ## 持久化
//!
//! 通过 `save_global_config` / `load_global_config` 钩子访问文件，文件层在 controllers
//! 阶段（2A）注入。本模块只做内存 + 状态管理。

use std::sync::Arc;

use parking_lot::RwLock;

use crate::models::store::normalize::{
    default_account_config, normalize_bag_seed_fallback_strategy,
    normalize_bag_seed_priority, normalize_friends_list_cache_ttl_sec,
    normalize_known_friend_gid_sync_cooldown_sec, normalize_known_friend_gids,
    normalize_fertilizer_land_types, normalize_intervals, normalize_positive_int_list,
    normalize_quiet_hours, default_account_config as full_default,
};
use crate::models::types::{
    AccountConfig, AccountConfigSnapshot, AutomationConfig, BagSeedFallbackStrategy,
    IntervalConfig, PlantingStrategy, QuietHoursConfig,
};

// =====================================================================
// 全局 AccountConfig 状态
// =====================================================================

/// 全局账号配置状态
#[derive(Clone)]
pub struct AccountConfigState {
    /// 各账号配置
    pub account_configs: std::collections::HashMap<String, AccountConfig>,
    /// 全局默认配置
    pub default_account_config: AccountConfig,
    /// 账号未注册时回退
    pub account_fallback_config: AccountConfig,
}

impl AccountConfigState {
    #[must_use]
    pub fn new() -> Self {
        let default = full_default();
        Self {
            account_configs: std::collections::HashMap::new(),
            default_account_config: default.clone(),
            account_fallback_config: default,
        }
    }
}

impl Default for AccountConfigState {
    fn default() -> Self {
        Self::new()
    }
}

static STATE: once_cell::sync::Lazy<Arc<RwLock<AccountConfigState>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(AccountConfigState::new())));

/// 获取全局状态（Arc clone，调用方自行 lock）
#[must_use]
pub fn state() -> Arc<RwLock<AccountConfigState>> {
    Arc::clone(&STATE)
}

/// 替换全局状态
pub fn set_state(new: AccountConfigState) {
    *STATE.write() = new;
}

/// 解析 account id（参数 / 环境变量 FARM_ACCOUNT_ID）
#[must_use]
pub fn resolve_account_id(account_id: Option<&str>) -> String {
    if let Some(s) = account_id {
        if !s.is_empty() {
            return s.to_string();
        }
    }
    std::env::var("FARM_ACCOUNT_ID").unwrap_or_default()
}

/// 获取单账号配置快照
#[must_use]
pub fn get_account_config_snapshot(account_id: Option<&str>) -> AccountConfig {
    let id = resolve_account_id(account_id);
    let state = STATE.read();
    if id.is_empty() {
        return state.account_fallback_config.clone();
    }
    state
        .account_configs
        .get(&id)
        .cloned()
        .unwrap_or_else(|| state.account_fallback_config.clone())
}

/// 获取配置快照（`AccountConfig + ui`）。
///
/// 1:1 对应原 TS `getConfigSnapshot()` 的返回类型。
/// runtime_state 中用 `obj.remove("ui")` 把 ui 字段剥掉，注入 `__revision`。
#[must_use]
pub fn get_config_snapshot(account_id: Option<&str>) -> AccountConfigSnapshot {
    AccountConfigSnapshot {
        config: get_account_config_snapshot(account_id),
        ui: None,
    }
}

/// 设置单账号配置（persist=true 时会触发 save 钩子，2A 注入）
pub fn set_account_config_snapshot(
    account_id: Option<&str>,
    next_config: AccountConfig,
    persist: bool,
) -> AccountConfig {
    let id = resolve_account_id(account_id);
    let mut state = STATE.write();
    if id.is_empty() {
        state.account_fallback_config = next_config.clone();
        state.default_account_config = next_config.clone();
        drop(state);
        if persist {
            let _ = crate::models::store::global_config::save_global_config();
        }
        return next_config;
    }
    state.account_configs.insert(id, next_config.clone());
    drop(state);
    if persist {
        let _ = crate::models::store::global_config::save_global_config();
    }
    next_config
}

/// 删除某账号配置
pub fn remove_account_config(account_id: &str) {
    let mut state = STATE.write();
    if state.account_configs.remove(account_id).is_some() {
        drop(state);
        let _ = crate::models::store::global_config::save_global_config();
    }
}

/// 确保某账号有配置（首次创建用 default）
pub fn ensure_account_config(account_id: &str) -> Option<AccountConfig> {
    let mut state = STATE.write();
    if let Some(c) = state.account_configs.get(account_id) {
        return Some(c.clone());
    }
    let cfg = default_account_config();
    state.account_configs.insert(account_id.to_string(), cfg.clone());
    drop(state);
    let _ = crate::models::store::global_config::save_global_config();
    Some(cfg)
}

// =====================================================================
// 单字段 getter
// =====================================================================

/// 自动化配置
#[must_use]
pub fn get_automation(account_id: Option<&str>) -> AutomationConfig {
    let mut a = get_account_config_snapshot(account_id).automation;
    let cur = a.fertilizer_land_types.clone();
    a.fertilizer_land_types = normalize_fertilizer_land_types(Some(cur), &a.fertilizer_land_types);
    a
}

/// 偏好种子
#[must_use]
pub fn get_preferred_seed(account_id: Option<&str>) -> i64 {
    get_account_config_snapshot(account_id).preferred_seed_id
}

/// 种植策略
#[must_use]
pub fn get_planting_strategy(account_id: Option<&str>) -> PlantingStrategy {
    get_account_config_snapshot(account_id).planting_strategy
}

/// bag seed priority（拷贝）
#[must_use]
pub fn get_bag_seed_priority(account_id: Option<&str>) -> Vec<i64> {
    get_account_config_snapshot(account_id).bag_seed_priority
}

/// bag seed fallback strategy
#[must_use]
pub fn get_bag_seed_fallback_strategy(account_id: Option<&str>) -> BagSeedFallbackStrategy {
    let s = get_account_config_snapshot(account_id).bag_seed_fallback_strategy;
    normalize_bag_seed_fallback_strategy(Some(s), s)
}

/// intervals
#[must_use]
pub fn get_intervals(account_id: Option<&str>) -> IntervalConfig {
    let i = get_account_config_snapshot(account_id).intervals;
    normalize_intervals(i)
}

/// friend quiet hours
#[must_use]
pub fn get_friend_quiet_hours(account_id: Option<&str>) -> QuietHoursConfig {
    get_account_config_snapshot(account_id).friend_quiet_hours
}

/// 已知 GID（先看 config，再看文件缓存）
#[must_use]
pub fn get_known_friend_gids(account_id: Option<&str>) -> Vec<i64> {
    let id = resolve_account_id(account_id);
    let config_gids = get_account_config_snapshot(account_id).known_friend_gids;
    if !config_gids.is_empty() {
        return config_gids;
    }
    if !id.is_empty() {
        if let Some(cached) = crate::models::store::gid_cache::read_cache(&id) {
            if !cached.is_empty() {
                return cached;
            }
        }
    }
    vec![]
}

/// 设置已知 GID
pub fn set_known_friend_gids(account_id: &str, list: Vec<i64>) -> Vec<i64> {
    let normalized = normalize_known_friend_gids(Some(list), &[]);
    let current = get_account_config_snapshot(Some(account_id));
    let next = AccountConfig {
        known_friend_gids: normalized.clone(),
        ..current
    };
    set_account_config_snapshot(Some(account_id), next, true);
    let _ = crate::models::store::gid_cache::write_cache(account_id, &normalized);
    normalized
}

/// 添加单个已知 GID
pub fn add_known_friend_gid(account_id: &str, gid: i64) -> Vec<i64> {
    if gid <= 0 {
        return get_known_friend_gids(Some(account_id));
    }
    let current = get_known_friend_gids(Some(account_id));
    if current.contains(&gid) {
        return current;
    }
    let mut next = current;
    next.push(gid);
    set_known_friend_gids(account_id, next)
}

/// 移除单个已知 GID
pub fn remove_known_friend_gid(account_id: &str, gid: i64) -> Vec<i64> {
    let current = get_known_friend_gids(Some(account_id));
    let next: Vec<i64> = current.into_iter().filter(|x| *x != gid).collect();
    set_known_friend_gids(account_id, next)
}

/// 批量添加已知 GID
pub fn add_known_friend_gids(account_id: &str, gids: &[i64]) -> Vec<i64> {
    let mut current = get_known_friend_gids(Some(account_id));
    for gid in gids {
        if *gid > 0 && !current.contains(gid) {
            current.push(*gid);
        }
    }
    set_known_friend_gids(account_id, current)
}

/// 批量移除已知 GID
pub fn remove_known_friend_gids(account_id: &str, gids: &[i64]) -> Vec<i64> {
    let current = get_known_friend_gids(Some(account_id));
    let next: Vec<i64> = current.into_iter().filter(|x| !gids.contains(x)).collect();
    set_known_friend_gids(account_id, next)
}

/// 已知 GID 同步冷却
#[must_use]
pub fn get_known_friend_gid_sync_cooldown_sec(account_id: Option<&str>) -> i64 {
    normalize_known_friend_gid_sync_cooldown_sec(Some(
        get_account_config_snapshot(account_id).known_friend_gid_sync_cooldown_sec,
    ))
}

/// 设置 GID 同步冷却
pub fn set_known_friend_gid_sync_cooldown_sec(account_id: &str, sec: i64) -> i64 {
    let normalized = normalize_known_friend_gid_sync_cooldown_sec(Some(sec));
    let current = get_account_config_snapshot(Some(account_id));
    let next = AccountConfig {
        known_friend_gid_sync_cooldown_sec: normalized,
        ..current
    };
    set_account_config_snapshot(Some(account_id), next, true);
    normalized
}

/// 好友列表缓存 TTL
#[must_use]
pub fn get_friends_list_cache_ttl_sec(account_id: Option<&str>) -> i64 {
    normalize_friends_list_cache_ttl_sec(Some(
        get_account_config_snapshot(account_id).friends_list_cache_ttl_sec,
    ))
}

/// 设置好友列表缓存 TTL
pub fn set_friends_list_cache_ttl_sec(account_id: &str, sec: i64) -> i64 {
    let normalized = normalize_friends_list_cache_ttl_sec(Some(sec));
    let current = get_account_config_snapshot(Some(account_id));
    let next = AccountConfig {
        friends_list_cache_ttl_sec: normalized,
        ..current
    };
    set_account_config_snapshot(Some(account_id), next, true);
    normalized
}

/// 好友黑名单
#[must_use]
pub fn get_friend_blacklist(account_id: Option<&str>) -> Vec<i64> {
    get_account_config_snapshot(account_id).friend_blacklist
}

/// 设置好友黑名单
pub fn set_friend_blacklist(account_id: &str, list: Vec<i64>) -> Vec<i64> {
    let normalized = normalize_positive_int_list(Some(list));
    let current = get_account_config_snapshot(Some(account_id));
    let next = AccountConfig {
        friend_blacklist: normalized.clone(),
        ..current
    };
    set_account_config_snapshot(Some(account_id), next, true);
    normalized
}

/// 加入黑名单
pub fn add_friend_to_blacklist(account_id: &str, gid: i64) -> bool {
    if gid <= 0 {
        return false;
    }
    let current = get_friend_blacklist(Some(account_id));
    if current.contains(&gid) {
        return false;
    }
    let mut new_list = current;
    new_list.push(gid);
    set_friend_blacklist(account_id, new_list);
    true
}

/// 切换黑名单（在/不在）
pub fn toggle_friend_blacklist(account_id: &str, gid: i64) -> Vec<i64> {
    if gid <= 0 {
        return get_friend_blacklist(Some(account_id));
    }
    let current = get_friend_blacklist(Some(account_id));
    let next: Vec<i64> = if current.contains(&gid) {
        current.into_iter().filter(|x| *x != gid).collect()
    } else {
        let mut v = current;
        v.push(gid);
        v
    };
    set_friend_blacklist(account_id, next)
}

/// 植物黑名单
#[must_use]
pub fn get_plant_blacklist(account_id: Option<&str>) -> Vec<i64> {
    get_account_config_snapshot(account_id).plant_blacklist
}

/// 设置植物黑名单
pub fn set_plant_blacklist(account_id: &str, list: Vec<i64>) -> Vec<i64> {
    let normalized = normalize_positive_int_list(Some(list));
    let current = get_account_config_snapshot(Some(account_id));
    let next = AccountConfig {
        plant_blacklist: normalized.clone(),
        ..current
    };
    set_account_config_snapshot(Some(account_id), next, true);
    normalized
}

/// 偷菜延迟
#[must_use]
pub fn get_steal_delay_seconds(account_id: Option<&str>) -> i64 {
    get_account_config_snapshot(account_id).steal_delay_seconds.clamp(0, 300)
}

/// 种植顺序随机
#[must_use]
pub fn get_plant_order_random(account_id: Option<&str>) -> bool {
    get_account_config_snapshot(account_id).plant_order_random
}

/// 种植延迟
#[must_use]
pub fn get_plant_delay_seconds(account_id: Option<&str>) -> i64 {
    get_account_config_snapshot(account_id).plant_delay_seconds.clamp(0, 60)
}

/// 默认 AccountConfig
#[must_use]
pub fn get_default_account_config() -> AccountConfig {
    default_account_config()
}

// =====================================================================
// applyConfigSnapshot
// =====================================================================

/// 应用配置快照（部分覆盖）
pub fn apply_config_snapshot(
    snapshot: serde_json::Value,
    account_id: Option<&str>,
    persist: bool,
) -> serde_json::Value {
    let current = get_account_config_snapshot(account_id);
    let mut next = current.clone();

    if let Some(auto) = snapshot.get("automation").and_then(|v| v.as_object()) {
        for (k, v) in auto {
            match k.as_str() {
                "fertilizer" => {
                    if let Some(s) = v.as_str() {
                        next.automation.fertilizer = match s {
                            "both" => crate::models::types::FertilizerMode::Both,
                            "normal" => crate::models::types::FertilizerMode::Normal,
                            "organic" => crate::models::types::FertilizerMode::Organic,
                            "smart" => crate::models::types::FertilizerMode::Smart,
                            "none" => crate::models::types::FertilizerMode::None,
                            _ => next.automation.fertilizer,
                        };
                    }
                }
                "fertilizer_land_types" => {
                    if let Some(arr) = v.as_array() {
                        let types: Vec<_> = arr
                            .iter()
                            .filter_map(|x| x.as_str().and_then(crate::models::types::FertilizerLandType::from_str_opt))
                            .collect();
                        next.automation.fertilizer_land_types =
                            normalize_fertilizer_land_types(Some(types), &next.automation.fertilizer_land_types);
                    }
                }
                "fertilizer_smart_seconds" => {
                    next.automation.fertilizer_smart_seconds =
                        v.as_i64().unwrap_or(300).clamp(30, 3600);
                }
                _ => {
                    if let Some(b) = v.as_bool() {
                        apply_automation_bool(&mut next.automation, k, b);
                    }
                }
            }
        }
    }

    if let Some(s) = snapshot.get("plantingStrategy").and_then(|v| v.as_str()) {
        if let Ok(p) = serde_json::from_value::<PlantingStrategy>(serde_json::Value::String(s.to_string())) {
            next.planting_strategy = p;
        }
    }

    if let Some(p) = snapshot.get("preferredSeedId").and_then(|v| v.as_i64()) {
        next.preferred_seed_id = p.max(0);
    }

    if let Some(intervals) = snapshot.get("intervals").and_then(|v| v.as_object()) {
        for (k, v) in intervals {
            if let Some(n) = v.as_i64() {
                match k.as_str() {
                    "farm" => next.intervals.farm = n.max(1),
                    "farmMin" => next.intervals.farm_min = n.max(1),
                    "farmMax" => next.intervals.farm_max = n.max(1),
                    "helpMin" => next.intervals.help_min = n.max(1),
                    "helpMax" => next.intervals.help_max = n.max(1),
                    "stealMin" => next.intervals.steal_min = n.max(1),
                    "stealMax" => next.intervals.steal_max = n.max(1),
                    _ => {}
                }
            }
        }
        next.intervals = normalize_intervals(next.intervals);
    }

    if let Some(qh) = snapshot.get("friendQuietHours").and_then(|v| v.as_object()) {
        let input = QuietHoursConfig {
            enabled: qh.get("enabled").and_then(|v| v.as_bool()).unwrap_or(next.friend_quiet_hours.enabled),
            start: qh.get("start").and_then(|v| v.as_str()).unwrap_or(&next.friend_quiet_hours.start).to_string(),
            end: qh.get("end").and_then(|v| v.as_str()).unwrap_or(&next.friend_quiet_hours.end).to_string(),
        };
        next.friend_quiet_hours = normalize_quiet_hours(&input, &next.friend_quiet_hours);
    }

    if let Some(arr) = snapshot.get("friendBlacklist").and_then(|v| v.as_array()) {
        next.friend_blacklist = arr
            .iter()
            .filter_map(|n| n.as_i64())
            .filter(|n| *n > 0)
            .collect();
    }

    if let Some(arr) = snapshot.get("plantBlacklist").and_then(|v| v.as_array()) {
        next.plant_blacklist = arr
            .iter()
            .filter_map(|n| n.as_i64())
            .filter(|n| *n > 0)
            .collect();
    }

    if let Some(n) = snapshot.get("stealDelaySeconds").and_then(|v| v.as_i64()) {
        next.steal_delay_seconds = n.clamp(0, 300);
    }
    if let Some(b) = snapshot.get("plantOrderRandom").and_then(|v| v.as_bool()) {
        next.plant_order_random = b;
    }
    if let Some(n) = snapshot.get("plantDelaySeconds").and_then(|v| v.as_i64()) {
        next.plant_delay_seconds = n.clamp(0, 60);
    }
    if let Some(n) = snapshot.get("fertilizerBuyOrganicCount").and_then(|v| v.as_i64()) {
        next.fertilizer_buy_organic_count = n.clamp(0, 10000);
    }
    if let Some(n) = snapshot.get("fertilizerBuyOrganicThresholdHours").and_then(|v| v.as_i64()) {
        next.fertilizer_buy_organic_threshold_hours = n.clamp(0, 990);
    }
    if let Some(n) = snapshot.get("fertilizerBuyNormalCount").and_then(|v| v.as_i64()) {
        next.fertilizer_buy_normal_count = n.clamp(0, 10000);
    }
    if let Some(n) = snapshot.get("fertilizerBuyNormalThresholdHours").and_then(|v| v.as_i64()) {
        next.fertilizer_buy_normal_threshold_hours = n.clamp(0, 990);
    }
    if let Some(n) = snapshot.get("fertilizerBuyCheckIntervalMinutes").and_then(|v| v.as_i64()) {
        next.fertilizer_buy_check_interval_minutes = n.clamp(1, 1440);
    }

    if let Some(arr) = snapshot.get("bagSeedPriority").and_then(|v| v.as_array()) {
        let ids: Vec<i64> = arr.iter().filter_map(|n| n.as_i64()).collect();
        next.bag_seed_priority = normalize_bag_seed_priority(Some(ids));
    }

    if let Some(s) = snapshot.get("bagSeedFallbackStrategy").and_then(|v| v.as_str()) {
        if let Ok(p) = serde_json::from_value::<BagSeedFallbackStrategy>(serde_json::Value::String(s.to_string())) {
            next.bag_seed_fallback_strategy =
                normalize_bag_seed_fallback_strategy(Some(p), next.bag_seed_fallback_strategy);
        }
    }

    set_account_config_snapshot(account_id, next.clone(), persist);
    if persist {
        let _ = crate::models::store::global_config::save_global_config();
    }
    serde_json::to_value(&next).unwrap_or_default()
}

fn apply_automation_bool(a: &mut AutomationConfig, key: &str, value: bool) {
    match key {
        "farm" => a.farm = value,
        "farm_push" | "farmPush" => a.farm_push = value,
        "land_upgrade" | "landUpgrade" => a.land_upgrade = value,
        "friend" => a.friend = value,
        "friend_help_exp_limit" | "friendHelpExpLimit" => a.friend_help_exp_limit = value,
        "friend_steal" | "friendSteal" => a.friend_steal = value,
        "friend_steal_activity_only" | "friendStealActivityOnly" => a.friend_steal_activity_only = value,
        "friend_help" | "friendHelp" => a.friend_help = value,
        "friend_bad" | "friendBad" => a.friend_bad = value,
        "task" => a.task = value,
        "fertilizer_gift" | "fertilizerGift" => a.fertilizer_gift = value,
        "fertilizer_buy_organic" | "fertilizerBuyOrganic" => a.fertilizer_buy_organic = value,
        "fertilizer_buy_normal" | "fertilizerBuyNormal" => a.fertilizer_buy_normal = value,
        "sell" => a.sell = value,
        "fertilizer_multi_season" | "fertilizerMultiSeason" => a.fertilizer_multi_season = value,
        "skip_own_weed_bug" | "skipOwnWeedBug" => a.skip_own_weed_bug = value,
        _ => {}
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn reset() {
        set_state(AccountConfigState::new());
    }

    #[test]
    #[serial(account_config)]
    fn get_snapshot_no_id_returns_fallback() {
        reset();
        // FARM_ACCOUNT_ID not set
        let s = get_account_config_snapshot(None);
        assert_eq!(s.planting_strategy, PlantingStrategy::MaxExp);
    }

    #[test]
    #[serial(account_config)]
    fn set_then_get_roundtrip() {
        reset();
        let mut cfg = default_account_config();
        cfg.preferred_seed_id = 12345;
        cfg.planting_strategy = PlantingStrategy::MaxExp;
        set_account_config_snapshot(Some("acc1"), cfg.clone(), false);

        let got = get_account_config_snapshot(Some("acc1"));
        assert_eq!(got.preferred_seed_id, 12345);
        assert_eq!(got.planting_strategy, PlantingStrategy::MaxExp);
    }

    #[test]
    #[serial(account_config)]
    fn ensure_creates_default() {
        reset();
        let cfg = ensure_account_config("acc2").expect("ensure");
        assert_eq!(cfg.planting_strategy, PlantingStrategy::MaxExp);
        // 第二次调用应返回已存在的
        let cfg2 = ensure_account_config("acc2").expect("ensure2");
        assert_eq!(cfg.preferred_seed_id, cfg2.preferred_seed_id);
    }

    #[test]
    #[serial(account_config)]
    fn known_friend_gids_with_file_cache_fallback() {
        reset();
        let aid = "test_known_gids_acc";
        // 写文件缓存
        crate::models::store::gid_cache::write_cache(aid, &[100, 200]).unwrap();
        let gids = get_known_friend_gids(Some(aid));
        assert_eq!(gids, vec![100, 200]);
        // 清理
        let _ = crate::models::store::gid_cache::remove_cache(aid);
    }

    #[test]
    #[serial(account_config)]
    fn add_to_blacklist_idempotent() {
        reset();
        let aid = "test_blacklist_acc";
        let _ = remove_account_config(aid); // 先清
        assert!(add_friend_to_blacklist(aid, 100));
        assert!(!add_friend_to_blacklist(aid, 100));
        assert!(add_friend_to_blacklist(aid, 200));
        let bl = get_friend_blacklist(Some(aid));
        assert_eq!(bl, vec![100, 200]);
    }

    #[test]
    #[serial(account_config)]
    fn apply_config_snapshot_overrides_planting() {
        reset();
        let mut s = serde_json::Map::new();
        s.insert("plantingStrategy".to_string(), serde_json::json!("max_profit"));
        s.insert("preferredSeedId".to_string(), serde_json::json!(7777));
        let v = apply_config_snapshot(serde_json::Value::Object(s), Some("acc3"), false);
        let cfg: AccountConfig = serde_json::from_value(v).expect("parse");
        assert_eq!(cfg.planting_strategy, PlantingStrategy::MaxProfit);
        assert_eq!(cfg.preferred_seed_id, 7777);
    }

    #[test]
    #[serial(account_config)]
    fn apply_config_snapshot_clamp_intervals() {
        reset();
        let mut s = serde_json::Map::new();
        let mut intervals = serde_json::Map::new();
        intervals.insert("farm".to_string(), serde_json::json!(0));
        intervals.insert("farmMin".to_string(), serde_json::json!(999));
        s.insert("intervals".to_string(), serde_json::Value::Object(intervals));
        let v = apply_config_snapshot(serde_json::Value::Object(s), Some("acc4"), false);
        let cfg: AccountConfig = serde_json::from_value(v).expect("parse");
        assert!(cfg.intervals.farm >= 1, "farm={}", cfg.intervals.farm);
    }

    #[test]
    fn env_account_id_fallback() {
        // FARM_ACCOUNT_ID not set
        std::env::remove_var("FARM_ACCOUNT_ID");
        assert_eq!(resolve_account_id(None), "");
        assert_eq!(resolve_account_id(Some("explicit")), "explicit");
    }
}
