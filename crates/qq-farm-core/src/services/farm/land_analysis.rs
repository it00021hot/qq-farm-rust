//! 土地状态分析工具。
//!
//! 对应原 `core/src/services/farm/land-analysis.ts`（650 行）。
//!
//! ## 阶段 1C.1 范围
//!
//! - 基础结构（land map、phase 枚举、判断可种植/可收获）
//! - 集合工具（collect / difference）
//!
//! ## 阶段 1C.2 范围
//!
//! - 完整布局、施肥目标、阶段判断
//!
//! ## 数据模型说明
//!
//! proto `LandInfo` 字段（prost 0.13 生成的 Rust）：
//! - `id: i64`
//! - `unlocked: bool`（注意是 bool 不是 i32）
//! - `level: i64` / `max_level: i64`
//! - `plant: Option<PlantInfo>`
//! - `master_land_id: i64` / `slave_land_ids: Vec<i64>`
//! - `is_shared: bool` / `can_share: bool`
//! - `land_size: i64`
//!
//! 阶段（phase）在 `PlantInfo.phases: Vec<PlantPhaseInfo>` 里，
//! 每个 `PlantPhaseInfo.phase: i32` 是当前阶段。

use std::collections::{HashMap, HashSet};

use crate::proto::generated::gamepb::plantpb::{LandInfo, PlantPhaseInfo};

/// 土地 map：land_id -> LandInfo
pub type LandMap = HashMap<i64, LandInfo>;

/// 构造 land_id -> LandInfo 映射
#[must_use]
pub fn build_land_map(lands: &[LandInfo]) -> LandMap {
    lands.iter().map(|l| (l.id, l.clone())).collect()
}

/// 阶段枚举（与原 TS 的 PlantPhase 对齐）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantPhase {
    Seed,
    Sprout,
    Growing,
    Ripe,
    Dead,
}

impl PlantPhase {
    /// 从 PlantPhaseInfo.phase 解析（取最后一个 phase 作为当前阶段）
    #[must_use]
    pub fn from_phases(phases: &[PlantPhaseInfo]) -> Option<Self> {
        phases.last().map(|p| Self::from_i32(p.phase))
    }

    /// 直接从 i32 解析
    #[must_use]
    pub fn from_i32(phase: i32) -> Self {
        match phase {
            0 => Self::Seed,
            1 => Self::Sprout,
            2 => Self::Growing,
            3 => Self::Ripe,
            4 => Self::Dead,
            _ => Self::Seed,
        }
    }

    /// 是否已种植物（有 plant 信息）
    #[must_use]
    pub fn is_planted(&self) -> bool {
        !matches!(self, Self::Seed)
    }
}

/// 土地当前阶段（无 plant 时为 Seed）
#[must_use]
pub fn current_phase(land: &LandInfo) -> PlantPhase {
    land.plant
        .as_ref()
        .and_then(|p| PlantPhase::from_phases(&p.phases))
        .unwrap_or(PlantPhase::Seed)
}

/// 判断土地是否可种植
#[must_use]
pub fn is_plantable(land: &LandInfo) -> bool {
    if !land.unlocked {
        return false;
    }
    matches!(current_phase(land), PlantPhase::Seed | PlantPhase::Dead)
}

/// 判断土地是否可收获
#[must_use]
pub fn is_harvestable(land: &LandInfo) -> bool {
    current_phase(land) == PlantPhase::Ripe
}

/// 判断土地是否枯死
#[must_use]
pub fn is_dead(land: &LandInfo) -> bool {
    current_phase(land) == PlantPhase::Dead
}

/// 统计各阶段土地数
#[derive(Debug, Default, Clone, Copy)]
pub struct LandSummary {
    pub plantable: usize,
    pub growing: usize,
    pub ripe: usize,
    pub dead: usize,
    pub total: usize,
}

/// 汇总土地状态
#[must_use]
pub fn summarize_lands(lands: &[LandInfo]) -> LandSummary {
    let mut s = LandSummary {
        total: lands.len(),
        ..Default::default()
    };
    for land in lands {
        match current_phase(land) {
            PlantPhase::Seed => {
                if land.unlocked {
                    s.plantable += 1;
                }
            }
            PlantPhase::Sprout | PlantPhase::Growing => s.growing += 1,
            PlantPhase::Ripe => s.ripe += 1,
            PlantPhase::Dead => s.dead += 1,
        }
    }
    s
}

/// 取出所有可种植土地的 id 列表
#[must_use]
pub fn collect_plantable(lands: &[LandInfo]) -> Vec<i64> {
    lands.iter().filter(|l| is_plantable(l)).map(|l| l.id).collect()
}

/// 取出所有可收获土地的 id 列表
#[must_use]
pub fn collect_harvestable(lands: &[LandInfo]) -> Vec<i64> {
    lands.iter().filter(|l| is_harvestable(l)).map(|l| l.id).collect()
}

/// 取出所有枯死土地的 id 列表
#[must_use]
pub fn collect_dead(lands: &[LandInfo]) -> Vec<i64> {
    lands.iter().filter(|l| is_dead(l)).map(|l| l.id).collect()
}

/// 集合差集
#[must_use]
pub fn land_ids_difference(a: &[i64], b: &[i64]) -> Vec<i64> {
    let b_set: HashSet<i64> = b.iter().copied().collect();
    a.iter().copied().filter(|id| !b_set.contains(id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::plantpb::PlantInfo;

    fn make_land(id: i64, unlocked: bool, phase: Option<PlantPhase>) -> LandInfo {
        let plant = phase.map(|p| PlantInfo {
            id: 1,
            name: String::new(),
            phases: vec![crate::proto::generated::gamepb::plantpb::PlantPhaseInfo {
                phase: p as i32,
                ..Default::default()
            }],
            ..Default::default()
        });
        LandInfo {
            id,
            unlocked,
            plant,
            ..Default::default()
        }
    }

    #[test]
    fn plantable_only_unlocked_seed_or_dead() {
        let unlocked_seed = make_land(1, true, None);
        let unlocked_ripe = make_land(2, true, Some(PlantPhase::Ripe));
        let unlocked_dead = make_land(3, true, Some(PlantPhase::Dead));
        let locked = make_land(4, false, None);
        assert!(is_plantable(&unlocked_seed));
        assert!(!is_plantable(&unlocked_ripe));
        assert!(is_plantable(&unlocked_dead));
        assert!(!is_plantable(&locked));
    }

    #[test]
    fn harvestable_only_ripe() {
        let ripe = make_land(1, true, Some(PlantPhase::Ripe));
        let dead = make_land(2, true, Some(PlantPhase::Dead));
        assert!(is_harvestable(&ripe));
        assert!(!is_harvestable(&dead));
    }

    #[test]
    fn summarize_counts() {
        let lands = vec![
            make_land(1, true, None),
            make_land(2, true, None),
            make_land(3, true, Some(PlantPhase::Ripe)),
            make_land(4, true, Some(PlantPhase::Growing)),
            make_land(5, false, None),
        ];
        let s = summarize_lands(&lands);
        assert_eq!(s.total, 5);
        assert_eq!(s.plantable, 2);
        assert_eq!(s.ripe, 1);
        assert_eq!(s.growing, 1);
    }

    #[test]
    fn collect_filters() {
        let lands = vec![
            make_land(1, true, None),
            make_land(2, true, Some(PlantPhase::Ripe)),
            make_land(3, true, Some(PlantPhase::Dead)),
            make_land(4, false, None),
        ];
        assert_eq!(collect_plantable(&lands), vec![1, 3]);
        assert_eq!(collect_harvestable(&lands), vec![2]);
        assert_eq!(collect_dead(&lands), vec![3]);
    }

    #[test]
    fn difference() {
        let a = vec![1, 2, 3, 4];
        let b = vec![2, 4];
        assert_eq!(land_ids_difference(&a, &b), vec![1, 3]);
    }
}
