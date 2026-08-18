//! 跨模块共享类型（按需提取）。
//!
//! 对应原项目 `core/src/types/`，**但不是 1:1 全搬**——只把 ≥3 个模块都用到的
//! 类型提到这里，**单模块用的留在各模块内部**（Rust 习惯 + 避免臃肿）。
//!
//! ## 提取原则
//!
//! 1. 跨 ≥3 个 service / controller 的 enum / struct → 放这里
//! 2. 跨 ≥2 个 service / controller 的类型 → 评估后决定
//! 3. 单模块用的 → 留在原模块
//!
//! ## 保留本地的类型
//!
//! - `AccountSession` (models/account.rs)：运行时 worker 会话
//! - `AccountRecord` (models/store/accounts.rs)：持久化 store 记录
//! - `WorkerMessage` / `WorkerState` (runtime/)：只有 runtime 用
//! - `IPCPayload` / `IPCResponse` (controllers/)：只有 controllers 用
//! - `DeviceInfo`：原 TS 类型，实际数据走 proto
//! - `ShopItem` / `BagItem`：mall / warehouse 各用，留在本地
//! - `FriendLandData` / `FriendCheckResult`：只在 friend service

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// =====================================================================
// 种植策略 / 施肥策略
// =====================================================================

/// 种植策略（从原 TS `PlantingStrategy` 1:1 翻译）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantingStrategy {
    /// 偏好种子（按 preferred_seed_id）
    Preferred,
    /// 按当前等级选
    Level,
    /// 最大经验
    MaxExp,
    /// 最大化肥经验
    MaxFertExp,
    /// 最大利润
    MaxProfit,
    /// 最大化肥利润
    MaxFertProfit,
    /// 背包优先
    BagPriority,
}

/// 背包种子兜底策略（不能是 `BagPriority`）
pub type BagSeedFallbackStrategy = PlantingStrategy;

impl Default for PlantingStrategy {
    fn default() -> Self {
        Self::Level
    }
}

/// 施肥模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FertilizerMode {
    /// 普通 + 有机肥都施
    Both,
    /// 仅普通肥
    Normal,
    /// 仅有机肥
    Organic,
    /// 智能：根据剩余成熟时间施肥
    Smart,
    /// 不施肥
    None,
}

impl Default for FertilizerMode {
    fn default() -> Self {
        Self::None
    }
}

/// 施肥土地类型筛选
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FertilizerLandType {
    /// 紫金
    #[serde(rename = "purple-gold")]
    PurpleGold,
    /// 金
    Gold,
    /// 黑
    Black,
    /// 红
    Red,
    /// 普通
    Normal,
}

impl FertilizerLandType {
    /// 字符串表示
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Gold => "gold",
            Self::Black => "black",
            Self::Red => "red",
            Self::PurpleGold => "purple-gold",
        }
    }

    /// 从字符串解析
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "normal" => Some(Self::Normal),
            "gold" => Some(Self::Gold),
            "black" => Some(Self::Black),
            "red" => Some(Self::Red),
            "purple-gold" => Some(Self::PurpleGold),
            _ => None,
        }
    }
}

// =====================================================================
// 土地 / 农场（UI 形态，不是 proto 形态）
// =====================================================================

/// 土地颜色等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LandType {
    /// 普通
    Normal,
    /// 金
    Gold,
    /// 黑
    Black,
    /// 红
    Red,
    /// 紫金
    #[serde(rename = "purple-gold")]
    PurpleGold,
}

impl Default for LandType {
    fn default() -> Self {
        Self::Normal
    }
}

impl LandType {
    /// 字符串表示
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Gold => "gold",
            Self::Black => "black",
            Self::Red => "red",
            Self::PurpleGold => "purple-gold",
        }
    }

    /// 从字符串解析
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "normal" => Some(Self::Normal),
            "gold" => Some(Self::Gold),
            "black" => Some(Self::Black),
            "red" => Some(Self::Red),
            "purple-gold" => Some(Self::PurpleGold),
            _ => None,
        }
    }
}

/// 植物阶段（生长周期）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantPhase {
    /// 空闲（未种）
    Empty,
    /// 种子期
    Seed,
    /// 发芽期
    Sprout,
    /// 成长期
    Growing,
    /// 成熟期
    Mature,
    /// 可收获
    Harvestable,
    /// 枯萎
    Withered,
}

impl Default for PlantPhase {
    fn default() -> Self {
        Self::Empty
    }
}

/// 单块土地运行时数据（UI 形态）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandData {
    pub land_id: i64,
    pub plant_id: i64,
    pub plant_phase: PlantPhase,
    pub water_time: i64,
    pub fertilizer_time: i64,
    pub harvest_time: i64,
    pub land_level: i64,
    pub land_type: LandType,
    pub status: i64,
}

/// 农场整体状态（UI 形态）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FarmStatus {
    pub lands: Vec<LandData>,
    pub harvest_count: i64,
    pub water_count: i64,
    pub weed_count: i64,
    pub insect_count: i64,
}

// =====================================================================
// 好友
// =====================================================================

/// 好友操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FriendOperation {
    /// 浇水
    Water,
    /// 除草
    Weed,
    /// 除虫
    Insecticide,
    /// 偷菜
    Steal,
    /// 施肥
    Fertilize,
    /// 一键务农（帮：除草 + 除虫 + 浇水）
    Farming,
    /// 捣乱（放虫 + 放草）
    Bad,
}

impl FriendOperation {
    /// 字符串名
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Water => "water",
            Self::Weed => "weed",
            Self::Insecticide => "insecticide",
            Self::Steal => "steal",
            Self::Fertilize => "fertilize",
            Self::Farming => "farming",
            Self::Bad => "bad",
        }
    }

    /// 从字符串解析
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "water" => Some(Self::Water),
            "weed" => Some(Self::Weed),
            // Go / web: `bug` == 除虫
            "insecticide" | "bug" => Some(Self::Insecticide),
            "steal" => Some(Self::Steal),
            "fertilize" => Some(Self::Fertilize),
            // Go / web: `help` == 一键务农
            "farming" | "help" => Some(Self::Farming),
            "bad" => Some(Self::Bad),
            _ => None,
        }
    }
}

/// 好友限额（每日操作上限）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OperationLimits {
    pub water: LimitEntry,
    pub weed: LimitEntry,
    pub insecticide: LimitEntry,
    pub steal: LimitEntry,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LimitEntry {
    pub used: i64,
    pub max: i64,
}

// =====================================================================
// 自动化配置
// =====================================================================

/// 单账号自动化开关
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationConfig {
    pub farm: bool,
    pub farm_push: bool,
    pub land_upgrade: bool,
    pub friend: bool,
    pub friend_help_exp_limit: bool,
    pub friend_steal: bool,
    pub friend_steal_activity_only: bool,
    pub friend_help: bool,
    pub friend_bad: bool,
    pub task: bool,
    pub fertilizer_gift: bool,
    pub fertilizer_buy_organic: bool,
    pub fertilizer_buy_normal: bool,
    pub sell: bool,
    pub fertilizer: FertilizerMode,
    pub fertilizer_multi_season: bool,
    pub fertilizer_land_types: Vec<FertilizerLandType>,
    pub fertilizer_smart_seconds: i64,
    pub skip_own_weed_bug: bool,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        crate::models::store::normalize::default_account_config().automation
    }
}

/// 间隔配置（秒）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntervalConfig {
    pub farm: i64,
    pub farm_min: i64,
    pub farm_max: i64,
    pub help_min: i64,
    pub help_max: i64,
    pub steal_min: i64,
    pub steal_max: i64,
    /// 其它自定义键
    #[serde(flatten)]
    pub extra: HashMap<String, i64>,
}

impl Default for IntervalConfig {
    fn default() -> Self {
        crate::models::store::normalize::default_account_config().intervals
    }
}

/// 安静时段（不巡访）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuietHoursConfig {
    pub enabled: bool,
    /// HH:MM
    pub start: String,
    /// HH:MM
    pub end: String,
}

/// 单账号完整配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountConfig {
    pub automation: AutomationConfig,
    pub planting_strategy: PlantingStrategy,
    pub preferred_seed_id: i64,
    pub intervals: IntervalConfig,
    pub friend_quiet_hours: QuietHoursConfig,
    pub known_friend_gids: Vec<i64>,
    pub known_friend_gid_sync_cooldown_sec: i64,
    pub friends_list_cache_ttl_sec: i64,
    pub friend_blacklist: Vec<i64>,
    pub plant_blacklist: Vec<i64>,
    pub steal_delay_seconds: i64,
    pub plant_order_random: bool,
    pub plant_delay_seconds: i64,
    pub fertilizer_buy_organic_count: i64,
    pub fertilizer_buy_organic_threshold_hours: i64,
    pub fertilizer_buy_normal_count: i64,
    pub fertilizer_buy_normal_threshold_hours: i64,
    pub fertilizer_buy_check_interval_minutes: i64,
    pub bag_seed_priority: Vec<i64>,
    pub bag_seed_fallback_strategy: BagSeedFallbackStrategy,
}

impl Default for AccountConfig {
    fn default() -> Self {
        // 单一权威源：与 Go DefaultAccountConfig / normalize::default_account_config 一致
        crate::models::store::normalize::default_account_config()
    }
}

/// UI 配置（用户层）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UIConfig {
    /// 主题
    pub theme: String,
    /// 语言
    pub language: String,
}

/// 配置快照（`AccountConfig + ui`）。
///
/// 1:1 对应原 TS `getConfigSnapshot()` 的返回类型。
/// 序列化时 `ui` 字段默认不输出（如 runtime_state 中 `obj.remove("ui")`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfigSnapshot {
    #[serde(flatten)]
    pub config: AccountConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UIConfig>,
}

impl Default for AccountConfigSnapshot {
    fn default() -> Self {
        Self { config: AccountConfig::default(), ui: None }
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planting_strategy_default_is_level() {
        assert_eq!(PlantingStrategy::default(), PlantingStrategy::Level);
    }

    #[test]
    fn planting_strategy_serde_roundtrip() {
        for s in [
            PlantingStrategy::Preferred,
            PlantingStrategy::Level,
            PlantingStrategy::MaxExp,
            PlantingStrategy::MaxFertExp,
            PlantingStrategy::MaxProfit,
            PlantingStrategy::MaxFertProfit,
            PlantingStrategy::BagPriority,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: PlantingStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn land_type_string_roundtrip() {
        for lt in
            [LandType::Normal, LandType::Gold, LandType::Black, LandType::Red, LandType::PurpleGold]
        {
            assert_eq!(LandType::from_str_opt(lt.as_str()), Some(lt));
        }
        assert_eq!(LandType::from_str_opt("unknown"), None);
    }

    #[test]
    fn fertilizer_land_type_kebab_serde() {
        let s = serde_json::to_string(&FertilizerLandType::PurpleGold).unwrap();
        assert_eq!(s, "\"purple-gold\"");
        let back: FertilizerLandType = serde_json::from_str(&s).unwrap();
        assert_eq!(back, FertilizerLandType::PurpleGold);
    }

    #[test]
    fn friend_operation_as_str() {
        assert_eq!(FriendOperation::Water.as_str(), "water");
        assert_eq!(FriendOperation::Steal.as_str(), "steal");
        assert_eq!(FriendOperation::Fertilize.as_str(), "fertilize");
        assert_eq!(FriendOperation::from_str_opt("help"), Some(FriendOperation::Farming));
        assert_eq!(FriendOperation::from_str_opt("farming"), Some(FriendOperation::Farming));
        assert_eq!(FriendOperation::from_str_opt("bug"), Some(FriendOperation::Insecticide));
    }

    #[test]
    fn account_config_default_serde() {
        let cfg = AccountConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AccountConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.planting_strategy, PlantingStrategy::MaxExp);
        assert_eq!(back.intervals.farm, 2);
        assert_eq!(back.intervals.steal_min, 20);
        assert!(back.automation.farm);
        assert!(back.automation.sell);
        assert!(back.automation.skip_own_weed_bug);
        assert!(!back.automation.friend_help);
        assert!(!back.automation.friend_bad);
        assert!(!back.automation.friend_help_exp_limit);
        assert!(back.automation.fertilizer_gift);
        assert_eq!(back.automation.fertilizer_smart_seconds, 360);
        assert!(back.bag_seed_priority.is_empty());
    }

    #[test]
    fn farm_status_default() {
        let s = FarmStatus::default();
        assert_eq!(s.lands.len(), 0);
        assert_eq!(s.harvest_count, 0);
    }
}
