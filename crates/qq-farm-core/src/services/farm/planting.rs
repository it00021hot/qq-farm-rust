//! 种植引擎 —— 选种子、拖动种植、按配置施肥。
//!
//! 对应原 `core/src/services/farm/planting.ts`（1021 行）。
//!
//! 阶段 1C.1 范围：基础占位 + 配置结构。
//! 阶段 1C.2 范围：核心种植流程（plantSeeds / runFertilizerByConfig）。
//! 阶段 1C.3 范围：种植策略（max_exp / max_profit / preferred / bag_priority）。

use serde::{Deserialize, Serialize};

/// 种植策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlantingStrategy {
    #[default]
    MaxExp,
    MaxProfit,
    MaxFertExp,
    MaxFertProfit,
    Level,
    Preferred,
    BagPriority,
}

/// 施肥模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FertilizeMode {
    /// 智能（默认）
    #[default]
    Smart,
    /// 有机 + 普通
    Both,
    /// 仅有机
    Organic,
    /// 仅普通
    Normal,
    /// 关闭
    None,
}

/// 种植配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantingConfig {
    /// 总开关
    pub enabled: bool,
    /// 策略
    pub strategy: PlantingStrategy,
    /// 优先种子 ID（`strategy=Preferred` 时用）
    pub preferred_seed_id: i64,
    /// 施肥模式
    pub fertilize_mode: FertilizeMode,
    /// 有机肥自动购买
    pub auto_buy_organic: bool,
    /// 普通肥自动购买
    pub auto_buy_normal: bool,
    /// 有机肥阈值
    pub organic_threshold: u32,
    /// 普通肥阈值
    pub normal_threshold: u32,
    /// 有机肥购买数量
    pub organic_buy_count: u32,
    /// 普通肥购买数量
    pub normal_buy_count: u32,
}

impl Default for PlantingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: PlantingStrategy::MaxExp,
            preferred_seed_id: 0,
            fertilize_mode: FertilizeMode::Smart,
            auto_buy_organic: false,
            auto_buy_normal: false,
            organic_threshold: 10,
            normal_threshold: 10,
            organic_buy_count: 50,
            normal_buy_count: 50,
        }
    }
}

/// 选择应种植的种子 ID
///
/// 阶段 1C.1 占位：返回 preferred_seed_id。
/// 阶段 1C.2 真正实现策略（从 game config 计算 max exp/profit）。
#[must_use]
pub fn select_seed(config: &PlantingConfig) -> i64 {
    config.preferred_seed_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let c = PlantingConfig::default();
        assert!(c.enabled);
        assert_eq!(c.strategy, PlantingStrategy::MaxExp);
        assert_eq!(c.fertilize_mode, FertilizeMode::Smart);
    }

    #[test]
    fn select_seed_returns_preferred() {
        let c = PlantingConfig { preferred_seed_id: 42, ..Default::default() };
        assert_eq!(select_seed(&c), 42);
    }

    #[test]
    fn strategy_serde_roundtrip() {
        let s = serde_json::to_string(&PlantingStrategy::MaxProfit).unwrap();
        assert_eq!(s, "\"MaxProfit\"");
    }
}
