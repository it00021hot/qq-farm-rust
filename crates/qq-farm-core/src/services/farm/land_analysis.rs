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
#[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
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

// ===== 阶段 1C.2 扩展 =====

/// 土地类型（按等级）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LandType {
    /// 普通地（等级 1）
    Normal,
    /// 红土地
    Red,
    /// 黑土地
    Black,
    /// 金土地
    Gold,
}

/// 按等级返回土地类型（与原 TS getLandTypeByLevel 对齐）
#[must_use]
pub fn land_type_by_level(level: i64) -> LandType {
    match level {
        1..=2 => LandType::Normal,
        3..=4 => LandType::Red,
        5..=6 => LandType::Black,
        _ => LandType::Gold,
    }
}

/// 全部施肥土地类型（用于"全选"判断）
pub const ALL_FERTILIZER_LAND_TYPES: &[LandType] = &[LandType::Normal, LandType::Red, LandType::Black, LandType::Gold];

/// 规范化施肥土地类型（去重 + 顺序）
#[must_use]
pub fn normalize_fertilizer_land_types(types: &[LandType]) -> Vec<LandType> {
    let mut seen = HashSet::new();
    types.iter().copied().filter(|t| seen.insert(*t)).collect()
}

/// 按类型过滤土地 id
#[must_use]
pub fn filter_land_ids_by_types(lands: &[LandInfo], types: &[LandType]) -> Vec<i64> {
    if types.is_empty() {
        return vec![];
    }
    let type_set: HashSet<LandType> = types.iter().copied().collect();
    let is_all = type_set.len() == ALL_FERTILIZER_LAND_TYPES.len()
        && ALL_FERTILIZER_LAND_TYPES.iter().all(|t| type_set.contains(t));
    if is_all {
        // "全选" → 所有解锁土地
        return lands.iter().filter(|l| l.unlocked).map(|l| l.id).collect();
    }
    lands
        .iter()
        .filter(|l| l.unlocked && type_set.contains(&land_type_by_level(l.level)))
        .map(|l| l.id)
        .collect()
}

/// 获取有机肥目标土地（已收获 + 多季作物）
#[must_use]
pub fn get_organic_fertilizer_targets(lands: &[LandInfo], planted_ids: &[i64]) -> Vec<i64> {
    let planted_set: HashSet<i64> = planted_ids.iter().copied().collect();
    lands
        .iter()
        .filter(|l| {
            // 已种植物的土地（除 Seed 阶段）作为多季补肥目标
            if !l.unlocked {
                return false;
            }
            if !planted_set.contains(&l.id) {
                return false;
            }
            // 多季作物 = 植物最后阶段不是 Ripe
            matches!(
                current_phase(l),
                PlantPhase::Sprout | PlantPhase::Growing
            )
        })
        .map(|l| l.id)
        .collect()
}

/// 获取即将成熟土地（用于"快速成熟"施肥）
///
/// 算法：取最后一个 phase 的 `begin_time`（Unix 秒），如果 `begin_time - now < threshold_secs` 则认为即将成熟。
#[must_use]
pub fn get_fast_mature_lands(lands: &[LandInfo], threshold_secs: i64) -> Vec<i64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut out = Vec::new();
    for land in lands {
        if !land.unlocked {
            continue;
        }
        if current_phase(land) != PlantPhase::Growing {
            continue;
        }
        let plant = match land.plant.as_ref() {
            Some(p) => p,
            None => continue,
        };
        if let Some(last_phase) = plant.phases.last() {
            let begin_sec = last_phase.begin_time; // proto int64 Unix seconds
            if begin_sec > 0 && (begin_sec - now_sec) <= threshold_secs {
                out.push(land.id);
            }
        }
    }
    out
}

/// 找出可铲除的"已收获且有多余"土地
#[must_use]
pub fn resolve_removable_harvested(lands: &[LandInfo], max_to_remove: usize) -> Vec<i64> {
    let ripe: Vec<i64> = lands
        .iter()
        .filter(|l| l.unlocked && current_phase(l) == PlantPhase::Ripe)
        .map(|l| l.id)
        .collect();
    ripe.into_iter().take(max_to_remove).collect()
}

/// 解析 Slave→Master 映射（多格作物的从地块指向主地块）
#[must_use]
pub fn build_slave_to_master_map(lands: &[LandInfo]) -> HashMap<i64, i64> {
    let mut map = HashMap::new();
    for land in lands {
        if land.master_land_id != 0 && land.master_land_id != land.id {
            map.insert(land.id, land.master_land_id);
        }
    }
    map
}

/// 判断土地是否被占用（作为 slave 被 master 持有）
#[must_use]
pub fn is_occupied_slave_land(land: &LandInfo) -> bool {
    land.master_land_id != 0 && land.master_land_id != land.id
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

    // ===== 阶段 1C.2 扩展测试 =====

    #[test]
    fn land_type_by_level_works() {
        assert_eq!(land_type_by_level(1), LandType::Normal);
        assert_eq!(land_type_by_level(2), LandType::Normal);
        assert_eq!(land_type_by_level(3), LandType::Red);
        assert_eq!(land_type_by_level(4), LandType::Red);
        assert_eq!(land_type_by_level(5), LandType::Black);
        assert_eq!(land_type_by_level(6), LandType::Black);
        assert_eq!(land_type_by_level(7), LandType::Gold);
        assert_eq!(land_type_by_level(100), LandType::Gold);
    }

    #[test]
    fn normalize_dedup_preserves_order() {
        let types = vec![LandType::Red, LandType::Normal, LandType::Red, LandType::Gold];
        let n = normalize_fertilizer_land_types(&types);
        assert_eq!(n, vec![LandType::Red, LandType::Normal, LandType::Gold]);
    }

    #[test]
    fn filter_by_all_returns_all_unlocked() {
        let lands = vec![
            make_land(1, true, None),
            make_land(2, false, None),
            make_land(3, true, None),
        ];
        let all = filter_land_ids_by_types(&lands, ALL_FERTILIZER_LAND_TYPES);
        assert_eq!(all, vec![1, 3]);
    }

    #[test]
    fn filter_by_type_subset() {
        let mut lands = vec![];
        for i in 1..=4 {
            let mut land = make_land(i, true, None);
            land.level = i;
            lands.push(land);
        }
        // 只选 Red (level 3-4)
        let red_ids = filter_land_ids_by_types(&lands, &[LandType::Red]);
        assert_eq!(red_ids, vec![3, 4]);
    }

    #[test]
    fn build_slave_to_master_basic() {
        let mut master = make_land(1, true, None);
        master.slave_land_ids = vec![2, 3];
        let mut slave2 = make_land(2, true, None);
        slave2.master_land_id = 1;
        let mut slave3 = make_land(3, true, None);
        slave3.master_land_id = 1;
        let map = build_slave_to_master_map(&[master, slave2, slave3]);
        assert_eq!(map.get(&2), Some(&1));
        assert_eq!(map.get(&3), Some(&1));
    }

    #[test]
    fn is_occupied_slave() {
        let mut slave = make_land(2, true, None);
        slave.master_land_id = 1;
        assert!(is_occupied_slave_land(&slave));
        assert!(!is_occupied_slave_land(&make_land(3, true, None)));
    }

    #[test]
    fn organic_targets_growing_only() {
        let lands = vec![
            make_land(1, true, Some(PlantPhase::Seed)),
            make_land(2, true, Some(PlantPhase::Growing)),
            make_land(3, true, Some(PlantPhase::Ripe)),
            make_land(4, true, Some(PlantPhase::Sprout)),
        ];
        let planted = vec![1, 2, 3, 4];
        let targets = get_organic_fertilizer_targets(&lands, &planted);
        assert_eq!(targets, vec![2, 4]);
    }
}
