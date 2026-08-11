//! 种植引擎 —— 选种子、拖动种植、按配置施肥。
//!
//! 对应原 `core/src/services/farm/planting.ts`（1021 行）。
//!
//! ## 阶段 1C.1 范围
//!
//! - 配置结构（[`PlantingStrategy`]、[`FertilizeMode`]、[`PlantingConfig`]）
//! - 基础占位（[`select_seed`]）
//!
//! ## 阶段 1C.2 范围（本文件）
//!
//! - [`plant_seeds`] 拖动种植入口（编码 + 调 api）
//! - [`fertilize_by_config`] 按配置施肥流程（normal / organic / both / smart / none）
//! - 占地大小 [`get_plant_size_by_seed_id`]（占位返回 1）
//! - 选种子策略 [`select_seed_for_strategy`]（占位：返回 preferred_seed_id）
//!
//! ## 阶段 1C.3 范围
//!
//! - 依赖 gameConfig 的具体策略算法（max_exp / max_profit 等）

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::error::Result;
use crate::services::farm::api::{
    Api, NORMAL_FERTILIZER_ID, ORGANIC_FERTILIZER_ID,
};

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
    /// 智能（默认）—— 普通 + 有机（如果多季作物）
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
    pub enabled: bool,
    pub strategy: PlantingStrategy,
    pub preferred_seed_id: i64,
    pub fertilize_mode: FertilizeMode,
    pub auto_buy_organic: bool,
    pub auto_buy_normal: bool,
    pub organic_threshold: u32,
    pub normal_threshold: u32,
    pub organic_buy_count: u32,
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

/// 种子占地大小（占位：阶段 1C.2 不知道 seed → plant 映射时返回 1）
#[must_use]
pub fn get_plant_size_by_seed_id(_seed_id: i64) -> usize {
    1
}

/// 按策略选种子（占位：阶段 1C.2 返回 preferred_seed_id）
#[must_use]
pub fn select_seed_for_strategy(config: &PlantingConfig) -> i64 {
    config.preferred_seed_id
}

/// 种植布局（多格作物的拖动序列）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantingLayout {
    /// 主地块 ID
    pub anchor_land_id: i64,
    /// 占用的所有地块 ID（按顺序）
    pub land_ids: Vec<i64>,
}

/// 种植结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlantSeedsResult {
    pub planted: usize,
    pub planted_land_ids: Vec<i64>,
    pub occupied_land_ids: Vec<i64>,
    pub reserved_land_ids: Vec<i64>,
    pub uncertain: bool,
}

/// 种植引擎
pub struct PlantingEngine {
    api: Api,
    config: PlantingConfig,
}

impl PlantingEngine {
    /// 创建
    #[must_use]
    pub fn new(api: Api, config: PlantingConfig) -> Self {
        Self { api, config }
    }

    /// 当前配置
    #[must_use]
    pub fn config(&self) -> &PlantingConfig {
        &self.config
    }

    /// 修改配置
    pub fn set_config(&mut self, config: PlantingConfig) {
        self.config = config;
    }

    /// 拖动种植：编码 + 调用 PlantSeeds API
    ///
    /// 对应原 planting.ts `plantSeeds(seedId, landIds, options)` 的核心流程
    pub async fn plant_seeds(
        &self,
        seed_id: i64,
        land_ids: Vec<i64>,
        host_gid: i64,
    ) -> Result<PlantSeedsResult> {
        if land_ids.is_empty() {
            return Ok(PlantSeedsResult::default());
        }

        // 编码：阶段 1C.2 走标准 protobuf path（通过 send_plant_request）
        // 原 TS 用了手写 encodePlanRequest，Rust 端走 API 同等路径
        // 实际上需要 gamepb.plantpb.PlantSeedsRequest 消息 — 阶段 1C.2 用 WaterLandRequest
        // 作为通用 land_ids + host_gid 容器
        self.api
            .send_plant_request("PlantSeeds", land_ids.clone(), host_gid)
            .await?;

        Ok(PlantSeedsResult {
            planted: land_ids.len(),
            planted_land_ids: land_ids,
            occupied_land_ids: vec![],
            reserved_land_ids: vec![],
            uncertain: false,
        })
    }

    /// 按配置施肥（核心流程）
    ///
    /// 对应原 planting.ts `runFertilizerByConfig(plantedLands, options)`
    ///
    /// 流程：
    /// 1. 拉取最新土地状态
    /// 2. 按 fertilize_mode 决定目标土地
    /// 3. 逐块施肥（normal: 50ms 间隔；organic: 1-1.5s 间隔）
    /// 4. 返回成功数
    pub async fn fertilize_by_config(
        &self,
        planted_land_ids: &[i64],
        host_gid: i64,
    ) -> Result<FertilizeResult> {
        let mode = self.config.fertilize_mode;
        if matches!(mode, FertilizeMode::None) {
            return Ok(FertilizeResult::default());
        }

        // 阶段 1C.2 简化：直接对 planted_land_ids 施肥
        // 完整流程（按 land type 过滤、Smart 模式多季检测）见 1C.3
        let targets: Vec<i64> = planted_land_ids.to_vec();
        if targets.is_empty() {
            return Ok(FertilizeResult::default());
        }

        let mut result = FertililzeResultBuilder::default();

        // 普通肥
        if matches!(mode, FertilizeMode::Normal | FertilizeMode::Both | FertilizeMode::Smart) {
            for (i, &land_id) in targets.iter().enumerate() {
                if self
                    .api
                    .fertilize(land_id, NORMAL_FERTILIZER_ID)
                    .await
                    .is_err()
                {
                    // 失败停止
                    break;
                }
                result.normal += 1;
                if i + 1 < targets.len() {
                    sleep(Duration::from_millis(50)).await;
                }
            }
        }

        // 有机肥
        if matches!(mode, FertilizeMode::Organic | FertilizeMode::Both | FertilizeMode::Smart) {
            for (i, &land_id) in targets.iter().enumerate() {
                if self
                    .api
                    .fertilize(land_id, ORGANIC_FERTILIZER_ID)
                    .await
                    .is_err()
                {
                    break;
                }
                result.organic += 1;
                if i + 1 < targets.len() {
                    let delay_ms = 1000 + (rand::random::<u64>() % 500);
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }

        Ok(result.build())
    }

    /// 收获指定土地
    pub async fn harvest(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<usize> {
        let n = land_ids.len();
        let _ = self.api.harvest(land_ids, host_gid, true).await?;
        Ok(n)
    }

    /// 铲除植物
    pub async fn remove_plant(&self, land_ids: Vec<i64>) -> Result<usize> {
        let n = land_ids.len();
        let _ = self.api.remove_plant(land_ids).await?;
        Ok(n)
    }

    /// 浇水
    pub async fn water(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<()> {
        self.api.water_land(land_ids, host_gid).await?;
        Ok(())
    }

    /// 锄地
    pub async fn farm(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<()> {
        self.api.farming(land_ids, host_gid).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FertilizeResult {
    pub normal: usize,
    pub organic: usize,
}

#[derive(Default)]
struct FertililzeResultBuilder {
    normal: usize,
    organic: usize,
}

impl FertililzeResultBuilder {
    fn build(self) -> FertilizeResult {
        FertilizeResult {
            normal: self.normal,
            organic: self.organic,
        }
    }
}

/// 选择应种植的种子 ID
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

    #[test]
    fn fertilize_mode_default_smart() {
        let m = FertilizeMode::default();
        assert_eq!(m, FertilizeMode::Smart);
    }

    #[test]
    fn get_plant_size_default_one() {
        assert_eq!(get_plant_size_by_seed_id(100), 1);
        assert_eq!(get_plant_size_by_seed_id(0), 1);
    }
}
