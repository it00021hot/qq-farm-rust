//! AccountConfig normalization helpers（共享给 store 各模块）。
//!
//! 1:1 翻译原 `core/src/models/store/shared-state.ts` 中的 normalize 函数。
//!
//! 这些函数**纯函数**——只做参数清洗 / 范围限制 / 默认值补全，**不读写任何状态**。

use std::collections::HashSet;

use crate::models::types::{
    AccountConfig, AutomationConfig, BagSeedFallbackStrategy, FertilizerLandType,
    IntervalConfig, PlantingStrategy, QuietHoursConfig,
};

/// 间隔上限
pub const INTERVAL_MAX_SEC: i64 = 86_400;
/// 已知 GID 同步冷却默认
pub const DEFAULT_KNOWN_FRIEND_GID_SYNC_COOLDOWN_SEC: i64 = 300;
/// 好友列表缓存 TTL 默认
pub const DEFAULT_FRIENDS_LIST_CACHE_TTL_SEC: i64 = 60;
/// 上一版默认 client version（用于 migration 检测）
pub const PREVIOUS_DEFAULT_CLIENT_VERSION: &str = "1.12.5.29_20260721";

/// 允许的种植策略
pub const ALLOWED_PLANTING_STRATEGIES: [PlantingStrategy; 7] = [
    PlantingStrategy::Preferred,
    PlantingStrategy::Level,
    PlantingStrategy::MaxExp,
    PlantingStrategy::MaxFertExp,
    PlantingStrategy::MaxProfit,
    PlantingStrategy::MaxFertProfit,
    PlantingStrategy::BagPriority,
];

/// 允许的背包种子兜底策略（排除 bag_priority）
pub const ALLOWED_BAG_SEED_FALLBACK_STRATEGIES: [BagSeedFallbackStrategy; 6] = [
    BagSeedFallbackStrategy::Preferred,
    BagSeedFallbackStrategy::Level,
    BagSeedFallbackStrategy::MaxExp,
    BagSeedFallbackStrategy::MaxFertExp,
    BagSeedFallbackStrategy::MaxProfit,
    BagSeedFallbackStrategy::MaxFertProfit,
];

/// 推送渠道白名单
pub const PUSHOO_CHANNELS: &[&str] = &[
    "webhook", "qmsg", "serverchan", "pushplus", "pushplushxtrip", "dingtalk", "wecom",
    "bark", "gocqhttp", "onebot", "atri", "pushdeer", "igot", "telegram", "feishu",
    "ifttt", "wecombot", "discord", "wxpusher",
];

/// 默认施肥土地类型
pub const DEFAULT_FERTILIZER_LAND_TYPES: &[FertilizerLandType] = &[
    FertilizerLandType::PurpleGold,
    FertilizerLandType::Gold,
    FertilizerLandType::Black,
    FertilizerLandType::Red,
    FertilizerLandType::Normal,
];

/// 默认植物黑名单（id 列表，对应原 TS plantBlacklist）
pub const DEFAULT_PLANT_BLACKLIST: &[i64] = &[
    20_002, 20_003, 20_059, 20_065, 20_064, 20_060, 20_061,
];

/// 默认 bag seed priority
pub const DEFAULT_BAG_SEED_PRIORITY: &[i64] = &[20_329, 21_037, 26_032, 29_003];

/// 规范化已知 GID 列表
pub fn normalize_known_friend_gids(input: impl Into<Option<Vec<i64>>>, fallback: &[i64]) -> Vec<i64> {
    let source = input.into().unwrap_or_else(|| fallback.to_vec());
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for v in source {
        if v <= 0 {
            continue;
        }
        if seen.insert(v) {
            normalized.push(v);
        }
    }
    normalized
}

/// 规范化 GID 同步冷却（30 ~ INTERVAL_MAX_SEC）
pub fn normalize_known_friend_gid_sync_cooldown_sec(input: impl Into<Option<i64>>) -> i64 {
    let v = input.into().unwrap_or(DEFAULT_KNOWN_FRIEND_GID_SYNC_COOLDOWN_SEC);
    v.clamp(30, INTERVAL_MAX_SEC)
}

/// 规范化好友列表缓存 TTL（10 ~ INTERVAL_MAX_SEC）
pub fn normalize_friends_list_cache_ttl_sec(input: impl Into<Option<i64>>) -> i64 {
    let v = input.into().unwrap_or(DEFAULT_FRIENDS_LIST_CACHE_TTL_SEC);
    v.clamp(10, INTERVAL_MAX_SEC)
}

/// 规范化 bag seed priority
pub fn normalize_bag_seed_priority(input: impl Into<Option<Vec<i64>>>) -> Vec<i64> {
    let source = input.into().unwrap_or_default();
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for v in source {
        if v <= 0 {
            continue;
        }
        if seen.insert(v) {
            normalized.push(v);
        }
    }
    normalized
}

/// 规范化 bag seed fallback strategy
pub fn normalize_bag_seed_fallback_strategy(
    input: impl Into<Option<BagSeedFallbackStrategy>>,
    fallback: BagSeedFallbackStrategy,
) -> BagSeedFallbackStrategy {
    match input.into() {
        Some(s) if ALLOWED_BAG_SEED_FALLBACK_STRATEGIES.contains(&s) => s,
        _ => fallback,
    }
}

/// 规范化施肥土地类型
pub fn normalize_fertilizer_land_types(
    input: impl Into<Option<Vec<FertilizerLandType>>>,
    fallback: &[FertilizerLandType],
) -> Vec<FertilizerLandType> {
    let source = input.into().unwrap_or_else(|| fallback.to_vec());
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for v in source {
        if seen.insert(v) {
            normalized.push(v);
        }
    }
    normalized
}

/// 规范化 HH:MM 字符串
pub fn normalize_time_string(v: impl Into<Option<String>>, fallback: &str) -> String {
    let raw = v.into().unwrap_or_default().trim().to_string();
    if let Some((h, m)) = raw.split_once(':') {
        if let (Ok(hh), Ok(mm)) = (h.trim().parse::<i64>(), m.trim().parse::<i64>()) {
            if (0..24).contains(&hh) && (0..60).contains(&mm) {
                return format!("{hh:02}:{mm:02}");
            }
        }
    }
    fallback.to_string()
}

/// 规范化 IntervalConfig
pub fn normalize_intervals(intervals: IntervalConfig) -> IntervalConfig {
    let to_sec = |v: i64, d: i64| v.max(1);
    let farm = to_sec(intervals.farm, 2);

    let mut farm_min = to_sec(intervals.farm_min, farm);
    let mut farm_max = to_sec(intervals.farm_max, farm);
    if farm_min > farm_max {
        std::mem::swap(&mut farm_min, &mut farm_max);
    }

    let mut help_min = to_sec(intervals.help_min, 10);
    let mut help_max = to_sec(intervals.help_max, 10);
    if help_min > help_max {
        std::mem::swap(&mut help_min, &mut help_max);
    }

    let mut steal_min = to_sec(intervals.steal_min, 10);
    let mut steal_max = to_sec(intervals.steal_max, 10);
    if steal_min > steal_max {
        std::mem::swap(&mut steal_min, &mut steal_max);
    }

    IntervalConfig {
        farm,
        farm_min,
        farm_max,
        help_min,
        help_max,
        steal_min,
        steal_max,
        extra: intervals.extra,
    }
}

/// 默认 AccountConfig
#[must_use]
pub fn default_account_config() -> AccountConfig {
    AccountConfig {
        automation: AutomationConfig {
            farm: true,
            farm_push: true,
            land_upgrade: true,
            friend: true,
            friend_help_exp_limit: false,
            friend_steal: true,
            friend_steal_activity_only: true,
            friend_help: false,
            friend_bad: false,
            task: true,
            fertilizer_gift: false,
            fertilizer_buy_organic: false,
            fertilizer_buy_normal: false,
            sell: true,
            fertilizer: crate::models::types::FertilizerMode::Smart,
            fertilizer_multi_season: true,
            fertilizer_land_types: DEFAULT_FERTILIZER_LAND_TYPES.to_vec(),
            fertilizer_smart_seconds: 360,
            skip_own_weed_bug: false,
        },
        planting_strategy: PlantingStrategy::Preferred,
        preferred_seed_id: 0,
        intervals: IntervalConfig {
            farm: 2,
            farm_min: 20,
            farm_max: 25,
            help_min: 20,
            help_max: 25,
            steal_min: 10,
            steal_max: 15,
            extra: Default::default(),
        },
        friend_quiet_hours: QuietHoursConfig {
            enabled: false,
            start: "01:00".to_string(),
            end: "07:30".to_string(),
        },
        known_friend_gids: vec![],
        known_friend_gid_sync_cooldown_sec: DEFAULT_KNOWN_FRIEND_GID_SYNC_COOLDOWN_SEC,
        friends_list_cache_ttl_sec: DEFAULT_FRIENDS_LIST_CACHE_TTL_SEC,
        friend_blacklist: vec![],
        plant_blacklist: DEFAULT_PLANT_BLACKLIST.to_vec(),
        steal_delay_seconds: 1,
        plant_order_random: true,
        plant_delay_seconds: 2,
        fertilizer_buy_organic_count: 1,
        fertilizer_buy_organic_threshold_hours: 10,
        fertilizer_buy_normal_count: 1,
        fertilizer_buy_normal_threshold_hours: 10,
        fertilizer_buy_check_interval_minutes: 60,
        bag_seed_priority: DEFAULT_BAG_SEED_PRIORITY.to_vec(),
        bag_seed_fallback_strategy: BagSeedFallbackStrategy::Level,
    }
}

/// 规范化 QuietHours
pub fn normalize_quiet_hours(input: &QuietHoursConfig, old: &QuietHoursConfig) -> QuietHoursConfig {
    QuietHoursConfig {
        enabled: input.enabled,
        start: normalize_time_string(input.start.clone(), &old.start),
        end: normalize_time_string(input.end.clone(), &old.end),
    }
}

/// 规范化正整数列表
pub fn normalize_positive_int_list(input: impl Into<Option<Vec<i64>>>) -> Vec<i64> {
    input
        .into()
        .unwrap_or_default()
        .into_iter()
        .filter(|n| *n > 0)
        .collect()
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_friend_gids_dedup() {
        let r = normalize_known_friend_gids(Some(vec![1, 2, 1, 3, 0, -1, 2]), &[]);
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn known_friend_gids_clamp_range() {
        assert_eq!(normalize_known_friend_gid_sync_cooldown_sec(Some(5)), 30);
        assert_eq!(normalize_known_friend_gid_sync_cooldown_sec(Some(999_999)), INTERVAL_MAX_SEC);
        assert_eq!(normalize_known_friend_gid_sync_cooldown_sec(None), DEFAULT_KNOWN_FRIEND_GID_SYNC_COOLDOWN_SEC);
    }

    #[test]
    fn friends_list_cache_ttl_clamp() {
        assert_eq!(normalize_friends_list_cache_ttl_sec(Some(1)), 10);
        assert_eq!(normalize_friends_list_cache_ttl_sec(Some(999_999)), INTERVAL_MAX_SEC);
    }

    #[test]
    fn time_string_normalize() {
        assert_eq!(normalize_time_string(Some("9:5".to_string()), "00:00"), "09:05");
        assert_eq!(normalize_time_string(Some("23:59".to_string()), "00:00"), "23:59");
        assert_eq!(normalize_time_string(Some("bad".to_string()), "12:00"), "12:00");
        assert_eq!(normalize_time_string(Some("25:00".to_string()), "12:00"), "12:00");
    }

    #[test]
    fn intervals_swap_min_max() {
        let i = IntervalConfig {
            farm: 2,
            farm_min: 50,
            farm_max: 10,
            help_min: 20,
            help_max: 25,
            steal_min: 10,
            steal_max: 15,
            extra: Default::default(),
        };
        let n = normalize_intervals(i);
        assert_eq!(n.farm_min, 10);
        assert_eq!(n.farm_max, 50);
    }

    #[test]
    fn default_account_config_valid() {
        let cfg = default_account_config();
        assert_eq!(cfg.planting_strategy, PlantingStrategy::Preferred);
        assert_eq!(cfg.automation.fertilizer_land_types.len(), 5);
        assert!(cfg.automation.farm);
        assert_eq!(cfg.fertilizer_buy_organic_threshold_hours, 10);
    }

    #[test]
    fn plant_blacklist_default_not_empty() {
        assert!(!DEFAULT_PLANT_BLACKLIST.is_empty());
        assert!(DEFAULT_PLANT_BLACKLIST.contains(&20_002));
    }

    #[test]
    fn bag_seed_priority_default() {
        assert!(!DEFAULT_BAG_SEED_PRIORITY.is_empty());
        assert_eq!(DEFAULT_BAG_SEED_PRIORITY[0], 20_329);
    }

    #[test]
    fn fertilizer_land_types_default_all_5() {
        assert_eq!(DEFAULT_FERTILIZER_LAND_TYPES.len(), 5);
    }
}
