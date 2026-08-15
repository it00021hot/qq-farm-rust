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
    /// 从 `PlantPhaseInfo` 列表解析当前阶段（对齐 TS `getCurrentPhase`）
    ///
    /// 从后往前找第一个 `begin_time > 0 && begin_time <= nowSec` 的阶段；
    /// 全部在未来则回退到第一个阶段。
    #[must_use]
    pub fn from_phases(phases: &[PlantPhaseInfo]) -> Option<Self> {
        if phases.is_empty() {
            return None;
        }
        let now_sec = crate::utils::time::get_server_time_secs();
        for p in phases.iter().rev() {
            let begin = crate::utils::time::to_time_secs(p.begin_time);
            if begin > 0 && begin <= now_sec {
                return Some(Self::from_i32(p.phase));
            }
        }
        Some(Self::from_i32(phases[0].phase))
    }

    /// 直接从 i32 解析（对齐 TS `PlantPhase` 枚举）
    ///
    /// - 0 UNKNOWN / 1 SEED → `Seed`
    /// - 2 GERMINATION → `Sprout`
    /// - 3 SMALL_LEAVES / 4 LARGE_LEAVES / 5 BLOOMING → `Growing`
    /// - 6 MATURE → `Ripe`
    /// - 7 DEAD → `Dead`
    #[must_use]
    pub fn from_i32(phase: i32) -> Self {
        match phase {
            2 => Self::Sprout,
            3 | 4 | 5 => Self::Growing,
            6 => Self::Ripe,
            7 => Self::Dead,
            // 0 UNKNOWN / 1 SEED / 其它未知 → Seed
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
    /// 普通土地（等级 1）
    Normal,
    /// 红土地（等级 2）
    Red,
    /// 黑土地（等级 3）
    Black,
    /// 金土地（等级 4）
    Gold,
    /// 紫金土地（等级 ≥5）
    PurpleGold,
}

/// 按等级返回土地类型（1:1 对齐原 TS `getLandTypeByLevel`）
///
/// - `>=5` → 紫金 `purple-gold`
/// - `==4` → 金 `gold`
/// - `==3` → 黑 `black`
/// - `==2` → 红 `red`
/// - 其它 → 普通 `normal`
#[must_use]
pub fn land_type_by_level(level: i64) -> LandType {
    match level {
        2 => LandType::Red,
        3 => LandType::Black,
        4 => LandType::Gold,
        n if n >= 5 => LandType::PurpleGold,
        _ => LandType::Normal,
    }
}

/// 全部施肥土地类型（用于"全选"判断，1:1 对齐 TS 的 5 类）
pub const ALL_FERTILIZER_LAND_TYPES: &[LandType] = &[
    LandType::PurpleGold,
    LandType::Gold,
    LandType::Black,
    LandType::Red,
    LandType::Normal,
];

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

/// 按勾选类型过滤一组土地 id（对齐 TS `filterLandIdsByTypes`）
#[must_use]
pub fn filter_ids_by_land_types(ids: &[i64], lands: &[LandInfo], types: &[LandType]) -> Vec<i64> {
    if types.is_empty() {
        return vec![];
    }
    let type_set: HashSet<LandType> = types.iter().copied().collect();
    if type_set.len() == ALL_FERTILIZER_LAND_TYPES.len()
        && ALL_FERTILIZER_LAND_TYPES.iter().all(|t| type_set.contains(t))
    {
        return ids.to_vec();
    }
    let map = build_land_map(lands);
    ids.iter()
        .copied()
        .filter(|id| {
            map.get(id)
                .map(|l| type_set.contains(&land_type_by_level(l.level)))
                .unwrap_or(false)
        })
        .collect()
}

/// 对齐 TS `getOrganicFertilizerTargetsFromLands`：所有还能施有机肥的地
#[must_use]
pub fn get_organic_fertilizer_targets_from_lands(lands: &[LandInfo]) -> Vec<i64> {
    let mut targets = Vec::new();
    for land in lands {
        if !land.unlocked {
            continue;
        }
        let Some(plant) = land.plant.as_ref() else {
            continue;
        };
        if plant.phases.is_empty() {
            continue;
        }
        if matches!(current_phase(land), PlantPhase::Dead) {
            continue;
        }
        // 服务端有该字段时，<=0 说明不能再施有机肥；未下发（None）则视为可施（对齐 bot hasOwnProperty）
        if plant.left_inorc_fert_times.is_some_and(|n| n <= 0) {
            continue;
        }
        targets.push(land.id);
    }
    targets
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

/// 即将成熟土地（对齐 TS `getFastMatureLands`：看成熟阶段 begin_time）
#[must_use]
pub fn get_fast_mature_lands(lands: &[LandInfo], threshold_secs: i64) -> Vec<i64> {
    let now_sec = crate::utils::time::get_server_time_secs();
    let threshold = threshold_secs.max(0);
    let mut out = Vec::new();
    for land in lands {
        if !land.unlocked {
            continue;
        }
        let Some(plant) = land.plant.as_ref() else {
            continue;
        };
        if plant.phases.is_empty() {
            continue;
        }
        match current_phase(land) {
            PlantPhase::Dead | PlantPhase::Ripe => continue,
            _ => {}
        }
        let Some(mature) = plant.phases.iter().find(|p| p.phase == 6) else {
            continue;
        };
        let mature_begin = crate::utils::time::to_time_secs(mature.begin_time);
        if mature_begin <= 0 {
            continue;
        }
        let time_to_mature = mature_begin - now_sec;
        if time_to_mature > threshold || time_to_mature < 0 {
            continue;
        }
        if plant.left_inorc_fert_times.is_some_and(|n| n <= 0) {
            continue;
        }
        out.push(land.id);
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

/// 判断土地是否被占用（作为 slave 被 master 持有；粗判，不含「master 是否有植物」）
#[must_use]
pub fn is_occupied_slave_land(land: &LandInfo) -> bool {
    land.master_land_id != 0 && land.master_land_id != land.id
}

/// 对齐 TS `isOccupiedSlaveLand(land, landsMap)`：
/// 仅当关联 master 存在且 master 有植物数据时，才把从地视为被占用而跳过。
#[must_use]
pub fn is_occupied_slave_land_with_map(land: &LandInfo, lands_map: &LandMap) -> bool {
    display_land_context(land, lands_map).1
}

/// 从土地取从属地块 id
#[must_use]
pub fn get_slave_land_ids(land: Option<&LandInfo>) -> Vec<i64> {
    land.map(|l| l.slave_land_ids.clone()).unwrap_or_default()
}

/// 土地分析结果（对齐 TS `analyzeLands`）
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LandAnalysis {
    pub harvestable: Vec<i64>,
    pub need_water: Vec<i64>,
    pub need_weed: Vec<i64>,
    pub need_bug: Vec<i64>,
    pub growing: Vec<i64>,
    pub empty: Vec<i64>,
    pub dead: Vec<i64>,
    pub unlockable: Vec<i64>,
    pub upgradable: Vec<i64>,
    pub harvestable_info: Vec<HarvestableInfo>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HarvestableInfo {
    pub land_id: i64,
    pub plant_id: i64,
    pub name: String,
    pub exp: i64,
}

/// 分析全部土地状态
#[must_use]
pub fn analyze_lands(lands: &[LandInfo], own_gid: i64) -> LandAnalysis {
    let mut result = LandAnalysis::default();
    let now_sec = crate::utils::time::get_server_time_secs();
    let lands_map = build_land_map(lands);

    for land in lands {
        let id = land.id;
        if !land.unlocked {
            if land.could_unlock {
                result.unlockable.push(id);
            }
            continue;
        }
        if land.could_upgrade {
            result.upgradable.push(id);
        }
        if is_occupied_slave_land_with_map(land, &lands_map) {
            continue;
        }

        let Some(plant) = land.plant.as_ref() else {
            result.empty.push(id);
            continue;
        };
        if plant.phases.is_empty() {
            result.empty.push(id);
            continue;
        }

        let Some(phase) = PlantPhase::from_phases(&plant.phases) else {
            result.empty.push(id);
            continue;
        };
        match phase {
            PlantPhase::Dead => {
                result.dead.push(id);
                continue;
            }
            PlantPhase::Ripe => {
                result.harvestable.push(id);
                let gc = crate::config::game_config::global();
                result.harvestable_info.push(HarvestableInfo {
                    land_id: id,
                    plant_id: plant.id,
                    name: {
                        let n = gc.get_plant_name(plant.id);
                        if n.is_empty() {
                            plant.name.clone()
                        } else {
                            n
                        }
                    },
                    exp: gc.get_plant_exp(plant.id),
                });
                continue;
            }
            PlantPhase::Seed if plant.phases.is_empty() => {
                result.empty.push(id);
                continue;
            }
            _ => {}
        }

        let current_phase_info = current_phase_info(&plant.phases);
        let dry_num = plant.dry_num;
        let dry_time = current_phase_info
            .map(|p| crate::utils::time::to_time_secs(p.dry_time))
            .unwrap_or(0);
        if dry_num > 0 || (dry_time > 0 && dry_time <= now_sec) {
            result.need_water.push(id);
        }

        let weeds_time = current_phase_info
            .map(|p| crate::utils::time::to_time_secs(p.weeds_time))
            .unwrap_or(0);
        let mut has_weeds = weeds_time > 0 && weeds_time <= now_sec;
        if !has_weeds && !plant.weed_owners.is_empty() {
            if own_gid != 0 {
                has_weeds = !plant.weed_owners.iter().all(|&g| g == own_gid);
            } else {
                has_weeds = true;
            }
        }
        if has_weeds {
            result.need_weed.push(id);
        }

        let insect_time = current_phase_info
            .map(|p| crate::utils::time::to_time_secs(p.insect_time))
            .unwrap_or(0);
        let mut has_bugs = insect_time > 0 && insect_time <= now_sec;
        if !has_bugs && !plant.insect_owners.is_empty() {
            if own_gid != 0 {
                has_bugs = !plant.insect_owners.iter().all(|&g| g == own_gid);
            } else {
                has_bugs = true;
            }
        }
        if has_bugs {
            result.need_bug.push(id);
        }

        result.growing.push(id);
    }
    result
}

fn current_phase_info(phases: &[PlantPhaseInfo]) -> Option<&PlantPhaseInfo> {
    if phases.is_empty() {
        return None;
    }
    let now_sec = crate::utils::time::get_server_time_secs();
    for p in phases.iter().rev() {
        let begin = crate::utils::time::to_time_secs(p.begin_time);
        if begin > 0 && begin <= now_sec {
            return Some(p);
        }
    }
    phases.first()
}

/// 多种植占地布局
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PlantingLayout {
    pub anchor_land_id: i64,
    pub land_ids: Vec<i64>,
}

/// 按植物占地尺寸生成合法布局
#[must_use]
pub fn build_planting_layouts(available_land_ids: &[i64], plant_size: usize) -> Vec<PlantingLayout> {
    let size = plant_size.max(1);
    let mut ordered: Vec<i64> = Vec::new();
    let mut seen_ids = HashSet::new();
    for &id in available_land_ids {
        if id > 0 && seen_ids.insert(id) {
            ordered.push(id);
        }
    }
    if size == 1 {
        return ordered
            .into_iter()
            .map(|id| PlantingLayout {
                anchor_land_id: id,
                land_ids: vec![id],
            })
            .collect();
    }

    let gc = crate::config::game_config::global();
    let available: HashSet<i64> = ordered.iter().copied().collect();
    let mut layouts = Vec::new();
    let mut seen = HashSet::new();

    for &anchor_land_id in &ordered {
        let Some(anchor) = gc.get_land_config_by_id(anchor_land_id) else {
            continue;
        };
        let mut footprint = Vec::new();
        let mut complete = true;
        'outer: for y_offset in 0..size as i64 {
            for x_offset in 0..size as i64 {
                let Some(land) =
                    gc.get_land_config_by_coordinate(anchor.grid_x + x_offset, anchor.grid_y + y_offset)
                else {
                    complete = false;
                    break 'outer;
                };
                if !available.contains(&land.id) {
                    complete = false;
                    break 'outer;
                }
                footprint.push(land.id);
            }
        }
        if !complete {
            continue;
        }
        let mut key_ids = footprint.clone();
        key_ids.sort_unstable();
        let key = key_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        if !seen.insert(key) {
            continue;
        }
        layouts.push(PlantingLayout {
            anchor_land_id,
            land_ids: footprint,
        });
    }
    layouts
}

/// 选出互不重叠的布局（对齐 TS `selectNonOverlappingLayouts`）
#[must_use]
pub fn select_non_overlapping_layouts(
    layouts: &[PlantingLayout],
    max_count: usize,
) -> Vec<PlantingLayout> {
    if max_count == 0 || layouts.is_empty() {
        return vec![];
    }
    let mut best: Vec<PlantingLayout> = Vec::new();
    fn visit(
        index: usize,
        source: &[PlantingLayout],
        selected: &mut Vec<PlantingLayout>,
        occupied: &mut HashSet<i64>,
        limit: usize,
        best: &mut Vec<PlantingLayout>,
    ) {
        if selected.len() > best.len() {
            *best = selected.clone();
        }
        if selected.len() >= limit || index >= source.len() {
            return;
        }
        if selected.len() + source.len() - index <= best.len() {
            return;
        }
        let layout = &source[index];
        if layout.land_ids.iter().all(|id| !occupied.contains(id)) {
            for id in &layout.land_ids {
                occupied.insert(*id);
            }
            selected.push(layout.clone());
            visit(index + 1, source, selected, occupied, limit, best);
            selected.pop();
            for id in &layout.land_ids {
                occupied.remove(id);
            }
        }
        visit(index + 1, source, selected, occupied, limit, best);
    }
    visit(
        0,
        layouts,
        &mut Vec::new(),
        &mut HashSet::new(),
        max_count,
        &mut best,
    );
    best
}

/// 解析某锚点实际占用的土地
#[must_use]
pub fn resolve_occupied_land_ids(
    anchor_land_id: i64,
    lands: &[LandInfo],
) -> (i64, Vec<i64>) {
    let lands_map = build_land_map(lands);
    let slave_to_master = build_slave_to_master_map(lands);
    let anchor = lands_map.get(&anchor_land_id);
    let declared_master = anchor.map(|l| l.master_land_id).unwrap_or(0);
    let master_land_id = if declared_master != 0 {
        declared_master
    } else {
        slave_to_master
            .get(&anchor_land_id)
            .copied()
            .unwrap_or(anchor_land_id)
    };
    let master = lands_map.get(&master_land_id).or(anchor);
    let mut occupied = HashSet::new();
    if master_land_id != 0 {
        occupied.insert(master_land_id);
    }
    for id in get_slave_land_ids(master) {
        occupied.insert(id);
    }
    for land in lands {
        if land.id != 0 && land.master_land_id == master_land_id {
            occupied.insert(land.id);
        }
        if land.id == master_land_id {
            for id in get_slave_land_ids(Some(land)) {
                occupied.insert(id);
            }
        }
    }
    if occupied.is_empty() && anchor_land_id != 0 {
        occupied.insert(anchor_land_id);
    }
    (
        if master_land_id != 0 {
            master_land_id
        } else {
            anchor_land_id
        },
        occupied.into_iter().collect(),
    )
}

/// 收获后土地分类
#[derive(Debug, Clone, Default)]
pub struct HarvestedClassify {
    pub removable: Vec<i64>,
    pub growing: Vec<i64>,
    pub unknown: Vec<i64>,
}

/// 土地生命周期
#[must_use]
pub fn get_land_lifecycle_state(land: Option<&LandInfo>) -> &'static str {
    let Some(land) = land else {
        return "unknown";
    };
    let Some(plant) = land.plant.as_ref() else {
        return "empty";
    };
    if plant.phases.is_empty() {
        return "empty";
    }
    match PlantPhase::from_phases(&plant.phases) {
        Some(PlantPhase::Dead) => "dead",
        // 对齐 bot：SEED..MATURE 视为 growing（多季作物收获后仍为 SEED 阶段，需保留而非铲除）
        Some(PlantPhase::Seed) => "growing",
        Some(_) => "growing",
        None => "empty",
    }
}

/// 按 map 分类刚收获的土地
#[must_use]
pub fn classify_harvested_lands_by_map(
    land_ids: &[i64],
    lands_map: &LandMap,
) -> HarvestedClassify {
    let mut out = HarvestedClassify::default();
    for &id in land_ids {
        match get_land_lifecycle_state(lands_map.get(&id)) {
            "dead" | "empty" => out.removable.push(id),
            "growing" => out.growing.push(id),
            _ => out.unknown.push(id),
        }
    }
    out
}

use crate::constants::{PHASE_DEAD, PHASE_MATURE, PHASE_NAMES, PHASE_UNKNOWN};

/// 面板土地汇总（对齐 TS `summarizeLandDetails`）
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandDetailSummary {
    pub harvestable: usize,
    pub growing: usize,
    pub empty: usize,
    pub dead: usize,
    pub need_water: usize,
    pub need_weed: usize,
    pub need_bug: usize,
}

/// 面板土地 DTO 模式
#[derive(Debug, Clone, Copy)]
pub enum LandDetailKind {
    /// 自己的农场：成熟=harvestable
    Own,
    /// 好友农场：成熟=stealable/harvested
    Friend,
}

fn has_plant_data(land: &LandInfo) -> bool {
    land.plant
        .as_ref()
        .is_some_and(|p| !p.phases.is_empty())
}

fn get_linked_master_land<'a>(land: &'a LandInfo, lands_map: &'a LandMap) -> Option<&'a LandInfo> {
    if land.master_land_id == 0 || land.master_land_id == land.id {
        return None;
    }
    let master = lands_map.get(&land.master_land_id)?;
    if !master.slave_land_ids.is_empty() && !master.slave_land_ids.contains(&land.id) {
        return None;
    }
    Some(master)
}

fn display_land_context<'a>(
    land: &'a LandInfo,
    lands_map: &'a LandMap,
) -> (&'a LandInfo, bool, i64, Vec<i64>) {
    if let Some(master) = get_linked_master_land(land, lands_map) {
        if has_plant_data(master) {
            let mut occupied = vec![master.id];
            occupied.extend(get_slave_land_ids(Some(master)));
            occupied.retain(|id| *id != 0);
            occupied.sort_unstable();
            occupied.dedup();
            let occupied = if occupied.is_empty() {
                vec![master.id]
            } else {
                occupied
            };
            return (master, true, master.id, occupied);
        }
    }
    let mut occupied = vec![land.id];
    occupied.extend(get_slave_land_ids(Some(land)));
    occupied.retain(|id| *id != 0);
    occupied.sort_unstable();
    occupied.dedup();
    (land, false, land.id, occupied)
}

fn summarize_land_details(lands: &[serde_json::Value]) -> LandDetailSummary {
    let mut summary = LandDetailSummary::default();
    for land in lands {
        if !land.get("unlocked").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        let status = land.get("status").and_then(|v| v.as_str()).unwrap_or("");
        match status {
            "harvestable" => summary.harvestable += 1,
            "dead" => summary.dead += 1,
            "empty" => summary.empty += 1,
            "growing" | "stealable" | "harvested" => summary.growing += 1,
            _ => {}
        }
        if land.get("needWater").and_then(|v| v.as_bool()).unwrap_or(false) {
            summary.need_water += 1;
        }
        if land.get("needWeed").and_then(|v| v.as_bool()).unwrap_or(false) {
            summary.need_weed += 1;
        }
        if land.get("needBug").and_then(|v| v.as_bool()).unwrap_or(false) {
            summary.need_bug += 1;
        }
    }
    summary
}

/// 构造面板土地 DTO（对齐 TS `getLandsDetail` / `getFriendLandsDetail`）
#[must_use]
pub fn build_lands_panel_dto(lands: &[LandInfo], kind: LandDetailKind) -> Vec<serde_json::Value> {
    let lands_map = build_land_map(lands);
    let now_sec = crate::utils::time::get_server_time_secs();
    let gc = crate::config::game_config::global();
    let mut out = Vec::with_capacity(lands.len());
    for land in lands {
        let (source, occupied_by_master, master_land_id, occupied_land_ids) =
            display_land_context(land, &lands_map);
        if !land.unlocked {
            let phase_name = match kind {
                LandDetailKind::Friend => "未解锁",
                LandDetailKind::Own => "",
            };
            let mut obj = serde_json::json!({
                "id": land.id,
                "unlocked": false,
                "status": "locked",
                "plantName": "",
                "seedImage": "",
                "phaseName": phase_name,
                "level": land.level,
                "occupiedByMaster": false,
                "masterLandId": 0,
                "occupiedLandIds": [],
                "plantSize": 1,
                "harvestable": false,
            });
            if matches!(kind, LandDetailKind::Own) {
                if let Some(o) = obj.as_object_mut() {
                    o.insert("maxLevel".into(), serde_json::json!(land.max_level));
                    o.insert("landsLevel".into(), serde_json::json!(land.lands_level));
                    o.insert("landSize".into(), serde_json::json!(land.land_size));
                    o.insert("couldUnlock".into(), serde_json::json!(land.could_unlock));
                    o.insert("couldUpgrade".into(), serde_json::json!(land.could_upgrade));
                    o.insert("currentSeason".into(), serde_json::json!(0));
                    o.insert("totalSeason".into(), serde_json::json!(0));
                }
            } else if let Some(o) = obj.as_object_mut() {
                o.insert("needWater".into(), serde_json::json!(false));
                o.insert("needWeed".into(), serde_json::json!(false));
                o.insert("needBug".into(), serde_json::json!(false));
            }
            out.push(obj);
            continue;
        }
        let plant = source.plant.as_ref();
        if plant.is_none() || plant.is_some_and(|p| p.phases.is_empty()) {
            let mut obj = serde_json::json!({
                "id": land.id,
                "unlocked": true,
                "status": "empty",
                "plantName": "",
                "seedImage": "",
                "phaseName": "空地",
                "level": land.level,
                "occupiedByMaster": occupied_by_master,
                "masterLandId": master_land_id,
                "occupiedLandIds": occupied_land_ids,
                "plantSize": 1,
                "harvestable": false,
            });
            if matches!(kind, LandDetailKind::Own) {
                if let Some(o) = obj.as_object_mut() {
                    o.insert("maxLevel".into(), serde_json::json!(land.max_level));
                    o.insert("landsLevel".into(), serde_json::json!(land.lands_level));
                    o.insert("landSize".into(), serde_json::json!(land.land_size));
                    o.insert("couldUnlock".into(), serde_json::json!(land.could_unlock));
                    o.insert("couldUpgrade".into(), serde_json::json!(land.could_upgrade));
                    o.insert("currentSeason".into(), serde_json::json!(0));
                    o.insert("totalSeason".into(), serde_json::json!(0));
                }
            }
            out.push(obj);
            continue;
        }
        let plant = plant.unwrap();
        let Some(current_phase) = current_phase_info(&plant.phases) else {
            let mut obj = serde_json::json!({
                "id": land.id,
                "unlocked": true,
                "status": "empty",
                "plantName": "",
                "seedImage": "",
                "phaseName": "",
                "level": land.level,
                "occupiedByMaster": occupied_by_master,
                "masterLandId": master_land_id,
                "occupiedLandIds": occupied_land_ids,
                "plantSize": 1,
                "harvestable": false,
            });
            if matches!(kind, LandDetailKind::Own) {
                if let Some(o) = obj.as_object_mut() {
                    o.insert("maxLevel".into(), serde_json::json!(land.max_level));
                    o.insert("landsLevel".into(), serde_json::json!(land.lands_level));
                    o.insert("landSize".into(), serde_json::json!(land.land_size));
                    o.insert("couldUnlock".into(), serde_json::json!(land.could_unlock));
                    o.insert("couldUpgrade".into(), serde_json::json!(land.could_upgrade));
                    o.insert("currentSeason".into(), serde_json::json!(0));
                    o.insert("totalSeason".into(), serde_json::json!(0));
                }
            }
            out.push(obj);
            continue;
        };
        let phase_val = current_phase.phase;
        let plant_id = plant.id;
        let mut plant_name = gc.get_plant_name(plant_id);
        if plant_name.is_empty() {
            plant_name = if plant.name.is_empty() {
                "未知".to_string()
            } else {
                plant.name.clone()
            };
        }
        let plant_cfg = gc.get_plant_by_id(plant_id);
        let seed_id = plant_cfg.as_ref().and_then(|p| p.seed_id).unwrap_or(0);
        let seed_image = if seed_id > 0 {
            gc.get_seed_image_by_seed_id(seed_id).unwrap_or_default()
        } else {
            String::new()
        };
        let plant_size = plant_cfg
            .as_ref()
            .and_then(|p| p.size)
            .unwrap_or(1)
            .max(1);
        let total_season = plant_cfg
            .as_ref()
            .and_then(|p| p.seasons)
            .unwrap_or(1)
            .max(1);
        let current_season_raw = plant.season;
        let current_season = if current_season_raw > 0 {
            current_season_raw.min(total_season)
        } else {
            1
        };
        let phase_name = PHASE_NAMES
            .get(phase_val as usize)
            .copied()
            .unwrap_or("");
        let mature_begin = plant
            .phases
            .iter()
            .find(|p| p.phase == PHASE_MATURE)
            .map(|p| crate::utils::time::to_time_secs(p.begin_time))
            .unwrap_or(0);
        let mature_in_sec = if mature_begin > now_sec {
            mature_begin - now_sec
        } else {
            0
        };
        let total_grow_time = gc.get_plant_grow_time(plant_id);
        let mut land_status = "growing".to_string();
        match kind {
            LandDetailKind::Own => {
                if phase_val == PHASE_MATURE {
                    land_status = "harvestable".into();
                } else if phase_val == PHASE_DEAD {
                    land_status = "dead".into();
                } else if phase_val == PHASE_UNKNOWN || plant.phases.is_empty() {
                    land_status = "empty".into();
                }
            }
            LandDetailKind::Friend => {
                if phase_val == PHASE_MATURE {
                    land_status = if plant.stealable {
                        "stealable".into()
                    } else {
                        "harvested".into()
                    };
                } else if phase_val == PHASE_DEAD {
                    land_status = "dead".into();
                }
            }
        }
        let dry_time = crate::utils::time::to_time_secs(current_phase.dry_time);
        let weeds_time = crate::utils::time::to_time_secs(current_phase.weeds_time);
        let insect_time = crate::utils::time::to_time_secs(current_phase.insect_time);
        let need_water = match kind {
            LandDetailKind::Own => {
                plant.dry_num > 0 || (dry_time > 0 && dry_time <= now_sec)
            }
            LandDetailKind::Friend => plant.dry_num > 0,
        };
        let need_weed = match kind {
            LandDetailKind::Own => {
                !plant.weed_owners.is_empty() || (weeds_time > 0 && weeds_time <= now_sec)
            }
            LandDetailKind::Friend => !plant.weed_owners.is_empty(),
        };
        let need_bug = match kind {
            LandDetailKind::Own => {
                !plant.insect_owners.is_empty() || (insect_time > 0 && insect_time <= now_sec)
            }
            LandDetailKind::Friend => !plant.insect_owners.is_empty(),
        };
        let mut obj = serde_json::json!({
            "id": land.id,
            "unlocked": true,
            "status": land_status,
            "plantName": plant_name,
            "seedId": seed_id,
            "seedImage": seed_image,
            "phaseName": phase_name,
            "currentSeason": current_season,
            "totalSeason": total_season,
            "matureInSec": mature_in_sec,
            "totalGrowTime": total_grow_time,
            "needWater": need_water,
            "needWeed": need_weed,
            "needBug": need_bug,
            "level": land.level,
            "occupiedByMaster": occupied_by_master,
            "masterLandId": master_land_id,
            "occupiedLandIds": occupied_land_ids,
            "plantSize": plant_size,
            "harvestable": land_status == "harvestable",
        });
        if matches!(kind, LandDetailKind::Own) {
            if let Some(o) = obj.as_object_mut() {
                o.insert("stealable".into(), serde_json::json!(plant.stealable));
                o.insert("maxLevel".into(), serde_json::json!(land.max_level));
                o.insert("landsLevel".into(), serde_json::json!(land.lands_level));
                o.insert("landSize".into(), serde_json::json!(land.land_size));
                o.insert("couldUnlock".into(), serde_json::json!(land.could_unlock));
                o.insert("couldUpgrade".into(), serde_json::json!(land.could_upgrade));
            }
        }
        out.push(obj);
    }
    out
}

/// 自己农场土地详情 + 汇总
#[must_use]
pub fn own_lands_detail(lands: &[LandInfo]) -> (Vec<serde_json::Value>, LandDetailSummary) {
    let dto = build_lands_panel_dto(lands, LandDetailKind::Own);
    let summary = summarize_land_details(&dto);
    (dto, summary)
}

/// 好友农场土地详情 DTO
#[must_use]
pub fn friend_lands_detail(lands: &[LandInfo]) -> Vec<serde_json::Value> {
    build_lands_panel_dto(lands, LandDetailKind::Friend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::plantpb::PlantInfo;

    /// 语义阶段 → 游戏 PlantPhase 数值（对齐 config.ts：SEED=1, GERMINATION=2,
    /// SMALL_LEAVES=3..BLOOMING=5, MATURE=6, DEAD=7）
    fn game_phase(p: PlantPhase) -> i32 {
        match p {
            PlantPhase::Seed => 1,
            PlantPhase::Sprout => 2,
            PlantPhase::Growing => 3,
            PlantPhase::Ripe => 6,
            PlantPhase::Dead => 7,
        }
    }

    fn make_land(id: i64, unlocked: bool, phase: Option<PlantPhase>) -> LandInfo {
        let plant = phase.map(|p| PlantInfo {
            id: 1,
            name: String::new(),
            phases: vec![crate::proto::generated::gamepb::plantpb::PlantPhaseInfo {
                phase: game_phase(p),
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
        assert_eq!(land_type_by_level(2), LandType::Red);
        assert_eq!(land_type_by_level(3), LandType::Black);
        assert_eq!(land_type_by_level(4), LandType::Gold);
        assert_eq!(land_type_by_level(5), LandType::PurpleGold);
        assert_eq!(land_type_by_level(6), LandType::PurpleGold);
        assert_eq!(land_type_by_level(7), LandType::PurpleGold);
        assert_eq!(land_type_by_level(100), LandType::PurpleGold);
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
        for i in 1..=5 {
            let mut land = make_land(i, true, None);
            land.level = i;
            lands.push(land);
        }
        // level 2 = Red（对齐 TS getLandTypeByLevel）
        let red_ids = filter_land_ids_by_types(&lands, &[LandType::Red]);
        assert_eq!(red_ids, vec![2]);
        // level >= 5 = PurpleGold
        let purple_ids = filter_land_ids_by_types(&lands, &[LandType::PurpleGold]);
        assert_eq!(purple_ids, vec![5]);
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
    fn occupied_slave_with_map_requires_master_plant() {
        let mut master_empty = make_land(1, true, None);
        master_empty.slave_land_ids = vec![2];
        let mut slave = make_land(2, true, Some(PlantPhase::Ripe));
        slave.master_land_id = 1;
        let map_empty = build_land_map(&[master_empty, slave.clone()]);
        assert!(!is_occupied_slave_land_with_map(&slave, &map_empty));

        let mut master_planted = make_land(1, true, Some(PlantPhase::Growing));
        master_planted.slave_land_ids = vec![2];
        let map_planted = build_land_map(&[master_planted, slave.clone()]);
        assert!(is_occupied_slave_land_with_map(&slave, &map_planted));
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

    #[test]
    fn analyze_lands_classifies_empty_ripe_dead() {
        let lands = vec![
            make_land(1, true, None),
            make_land(2, true, Some(PlantPhase::Ripe)),
            make_land(3, true, Some(PlantPhase::Dead)),
            make_land(4, false, None),
        ];
        let a = analyze_lands(&lands, 0);
        assert_eq!(a.empty, vec![1]);
        assert_eq!(a.harvestable, vec![2]);
        assert_eq!(a.dead, vec![3]);
        assert!(a.unlockable.is_empty());
    }

    #[test]
    fn build_planting_layouts_size_one() {
        let layouts = build_planting_layouts(&[3, 1, 1, 2], 1);
        assert_eq!(layouts.len(), 3);
        assert_eq!(layouts[0].anchor_land_id, 3);
        assert_eq!(layouts[0].land_ids, vec![3]);
    }

    #[test]
    fn select_non_overlapping_picks_disjoint() {
        let layouts = vec![
            PlantingLayout {
                anchor_land_id: 1,
                land_ids: vec![1, 2],
            },
            PlantingLayout {
                anchor_land_id: 3,
                land_ids: vec![3, 4],
            },
            PlantingLayout {
                anchor_land_id: 2,
                land_ids: vec![2, 3],
            },
        ];
        let picked = select_non_overlapping_layouts(&layouts, 2);
        assert_eq!(picked.len(), 2);
        let used: HashSet<i64> = picked.iter().flat_map(|l| l.land_ids.iter().copied()).collect();
        assert_eq!(used.len(), 4);
    }

    #[test]
    fn resolve_occupied_uses_master_and_slaves() {
        let mut master = make_land(1, true, Some(PlantPhase::Growing));
        master.slave_land_ids = vec![2, 3];
        let mut slave = make_land(2, true, None);
        slave.master_land_id = 1;
        let (mid, occupied) = resolve_occupied_land_ids(2, &[master, slave]);
        assert_eq!(mid, 1);
        assert!(occupied.contains(&1));
        assert!(occupied.contains(&2));
        assert!(occupied.contains(&3));
    }

    #[test]
    fn land_lifecycle_seed_is_growing() {
        // 对齐 bot：SEED 阶段视为 growing（多季收获后仍为 SEED，不可当 empty 铲除）
        let land = make_land(1, true, Some(PlantPhase::Seed));
        assert_eq!(get_land_lifecycle_state(Some(&land)), "growing");
        assert_eq!(get_land_lifecycle_state(Some(&make_land(2, true, None))), "empty");
        assert_eq!(
            get_land_lifecycle_state(Some(&make_land(3, true, Some(PlantPhase::Dead)))),
            "dead"
        );
    }

    #[test]
    fn organic_targets_skip_only_when_left_inorc_present_and_zero() {
        let mut allow = make_land(1, true, Some(PlantPhase::Growing));
        if let Some(p) = allow.plant.as_mut() {
            p.left_inorc_fert_times = None;
        }
        let mut deny = make_land(2, true, Some(PlantPhase::Growing));
        if let Some(p) = deny.plant.as_mut() {
            p.left_inorc_fert_times = Some(0);
        }
        let mut ok = make_land(3, true, Some(PlantPhase::Growing));
        if let Some(p) = ok.plant.as_mut() {
            p.left_inorc_fert_times = Some(2);
        }
        let targets = get_organic_fertilizer_targets_from_lands(&[allow, deny, ok]);
        assert_eq!(targets, vec![1, 3]);
    }
}
