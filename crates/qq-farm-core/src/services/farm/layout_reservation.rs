//! 多格作物的未来布局预留。
//!
//! 1:1 翻译原 `core/src/services/farm/layout-reservation.ts`（bot `96fdb39`，2026-09-11）。
//!
//! 用途：高优先级的多格背包种子暂时凑不齐完整布局时，选一组「未来布局」，
//! 只预留其中当前已经空出的土地，避免这些空地被后续低优先种子占掉；
//! 始终选锚点最小且已部分空出的布局，使后续轮次只会向更早布局收敛，不来回切换。

use crate::services::farm::land_analysis::{build_planting_layouts, PlantingLayout};

/// 预留结果：目标布局 + 其中当前已空出、应被预留的土地。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutureLayoutReservation {
    pub layout: PlantingLayout,
    pub reserved_land_ids: Vec<i64>,
}

/// 为暂时凑不齐的多格作物选择一组未来布局，并只预留其中当前已空出的土地。
///
/// - `current_empty_land_ids`：本轮该种子可用的空地（已按土地类型过滤）
/// - `all_eligible_land_ids`：全部合格土地（未解锁的除外）
/// - `plant_size`：作物占地边长
///
/// 返回 `None` 表示无需/无法预留：单格作物、空地已能连成完整布局（直接种）、
/// 或没有任何「部分空出」的候选布局。
#[must_use]
pub fn select_future_layout_reservation(
    current_empty_land_ids: &[i64],
    all_eligible_land_ids: &[i64],
    plant_size: usize,
) -> Option<FutureLayoutReservation> {
    let size = plant_size.max(1);
    if size <= 1 {
        return None;
    }

    let empty_ids: std::collections::HashSet<i64> =
        current_empty_land_ids.iter().copied().filter(|&id| id > 0).collect();
    if empty_ids.is_empty() {
        return None;
    }
    // 空地已能连成完整布局：直接种，无需预留
    let empty_vec: Vec<i64> = empty_ids.iter().copied().collect();
    if !build_planting_layouts(&empty_vec, size).is_empty() {
        return None;
    }

    let mut candidates: Vec<FutureLayoutReservation> = build_planting_layouts(
        &all_eligible_land_ids.iter().copied().filter(|&id| id > 0).collect::<Vec<i64>>(),
        size,
    )
    .into_iter()
    .map(|layout| {
        let reserved_land_ids: Vec<i64> =
            layout.land_ids.iter().copied().filter(|id| empty_ids.contains(id)).collect();
        FutureLayoutReservation { layout, reserved_land_ids }
    })
    // 只保留「部分空出」的布局：全空说明能直接种；全不空无法帮助收敛
    .filter(|c| {
        !c.reserved_land_ids.is_empty() && c.reserved_land_ids.len() < c.layout.land_ids.len()
    })
    .collect();

    // 锚点最小 => 收敛到最早的布局，避免布局来回切换
    candidates.sort_by_key(|c| c.layout.anchor_land_id);
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// bot `farm-multiland-reservation.test.js` 场景 3：empty=[1,2,3]、all=1..15、size=2
    /// → 选 anchor 5 的布局 [5,6,1,2]，预留其中已空出的 [1,2]。
    #[test]
    fn reserves_earliest_stable_layout() {
        let all: Vec<i64> = (1..=15).collect();
        let r =
            select_future_layout_reservation(&[1, 2, 3], &all, 2).expect("reservation expected");
        assert_eq!(r.layout.anchor_land_id, 5);
        assert_eq!(r.layout.land_ids, vec![5, 6, 1, 2]);
        assert_eq!(r.reserved_land_ids, vec![1, 2]);
    }

    /// 场景 4：empty=[1,2,5] → 同一布局，预留全部已空出的部分 [5,1,2]。
    #[test]
    fn keeps_accumulating_empty_lands() {
        let all: Vec<i64> = (1..=15).collect();
        let r = select_future_layout_reservation(&[1, 2, 5], &all, 2).expect("reservation");
        assert_eq!(r.layout.anchor_land_id, 5);
        assert_eq!(r.reserved_land_ids, vec![5, 1, 2]);
    }

    /// 场景 5（防振荡）：empty=[1,3,4,7] 仍选 anchor 5 布局（更满的更晚布局不选）。
    #[test]
    fn prefers_earliest_partial_layout() {
        let all: Vec<i64> = (1..=15).collect();
        let r = select_future_layout_reservation(&[1, 3, 4, 7], &all, 2).expect("reservation");
        assert_eq!(r.layout.anchor_land_id, 5);
        assert_eq!(r.reserved_land_ids, vec![1]);
    }

    /// 场景 6：empty=[1,2,5,6] 已能连成完整布局 → 直接种，不预留。
    #[test]
    fn does_not_reserve_when_layout_already_plantable() {
        let all: Vec<i64> = (1..=15).collect();
        assert_eq!(select_future_layout_reservation(&[1, 2, 5, 6], &all, 2), None);
    }

    /// 场景 7：单格作物不预留；4 块地里凑不出的布局也不预留。
    #[test]
    fn does_not_reserve_single_land_or_impossible_layouts() {
        let all: Vec<i64> = (1..=15).collect();
        assert_eq!(select_future_layout_reservation(&[1, 2, 3], &all, 1), None);
        assert_eq!(select_future_layout_reservation(&[1, 2, 3], &[1, 2, 3, 4], 2), None);
    }

    /// 空地列表为空 → 不预留。
    #[test]
    fn does_not_reserve_without_empty_lands() {
        let all: Vec<i64> = (1..=15).collect();
        assert_eq!(select_future_layout_reservation(&[], &all, 2), None);
    }
}
