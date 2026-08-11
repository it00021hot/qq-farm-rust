//! 土地领域模型。

use serde::{Deserialize, Serialize};

/// 土地状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandStatus {
    /// 空闲
    Empty,
    /// 已种植，生长中
    Growing,
    /// 可收获
    Ripe,
    /// 已枯死
    Dead,
}

/// 单块土地运行时数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Land {
    /// 土地 ID（在农场内唯一）
    pub id: u32,
    /// 当前种植的作物 ID（None = 空闲）
    pub crop_id: Option<u32>,
    /// 状态
    pub status: LandStatus,
    /// 剩余成熟时间（秒）
    pub remaining_secs: u32,
}

impl Land {
    /// 创建新土地（默认空闲）
    #[must_use]
    pub fn new(id: u32) -> Self {
        Self {
            id,
            crop_id: None,
            status: LandStatus::Empty,
            remaining_secs: 0,
        }
    }
}
