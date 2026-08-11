//! 游戏静态配置（作物、道具、等级等）。
//!
//! 原项目位于 `core/src/gameConfig/`，17MB 静态资源。
//! 阶段 0：仅定义占位结构，加载逻辑留到阶段 1。

use serde::{Deserialize, Serialize};

/// 游戏配置总入口（占位）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameConfig {
    /// 作物列表
    pub plants: Vec<PlantInfo>,
    /// 道具列表
    pub items: Vec<ItemInfo>,
}

/// 作物静态信息（占位字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantInfo {
    pub id: u32,
    pub name: String,
    pub seed_id: u32,
    pub growth_time_secs: u32,
    pub exp_per_harvest: u32,
    pub gold_per_harvest: u32,
}

/// 道具静态信息（占位字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemInfo {
    pub id: u32,
    pub name: String,
    pub kind: String,
}
