//! 多格作物的未来布局预留。
//!
//! 1:1 翻译原 `core/src/services/farm/layout-reservation.ts`（bot `1a5c23e`，2026-10-12，#89）。
//!
//! 用途：高优先级的多格背包种子暂时凑不齐完整布局时，选一组「未来布局」，
//! 只预留其中当前已经空出的土地，避免这些空地被后续低优先种子占掉。
//!
//! 选择规则（#89）：
//! 1. 已预留格数最多的候选布局优先，避免「后空出的地被别的作物种掉」；
//! 2. 格数相同时优先沿用已登记的目标布局，使轮次之间不来回切换（只会向更满的布局收敛）；
//! 3. 仍相同时取锚点最小者，保证结果稳定。
//!
//! 已登记布局的状态放 [`crate::services::farm::planting::PlantingEngine`]（每账号一份）：
//! bot 用模块级 Map，靠「每账号独立 worker 进程」隔离；rust 多账号同进程，按引擎实例隔离。

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::services::farm::land_analysis::{build_planting_layouts, PlantingLayout};

/// 预留结果：目标布局 + 其中当前已空出、应被预留的土地。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutureLayoutReservation {
    pub layout: PlantingLayout,
    pub reserved_land_ids: Vec<i64>,
}

/// 选择参数（对齐 TS `SelectFutureLayoutOptions`）。
#[derive(Debug, Clone, Default)]
pub struct SelectFutureLayoutOptions {
    /// 上一轮为该种子登记的目标布局；在「预留格数不吃亏」的前提下优先沿用。
    pub registered: Option<PlantingLayout>,
    /// 本轮已被其他种子占用的整组目标布局土地；与之重叠的候选布局不可选。
    pub exclude_land_ids: Vec<i64>,
}

/// 布局指纹：landIds 升序拼接，用于判断两组布局是否相同（对齐 TS `layoutKey`）。
fn layout_key(layout: Option<&PlantingLayout>) -> String {
    let Some(layout) = layout else { return String::new() };
    let mut ids: Vec<i64> = layout.land_ids.iter().copied().collect();
    ids.sort_unstable();
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

/// 为暂时凑不齐的多格作物选择一组未来布局，并只预留其中当前已空出的土地。
///
/// - `current_empty_land_ids`：本轮该种子可用的空地（已按土地类型过滤）
/// - `all_eligible_land_ids`：全部合格土地（未解锁的除外）
/// - `plant_size`：作物占地边长
///
/// 返回 `None` 表示无需/无法预留：单格作物、空地已能连成完整布局（直接种）、
/// 或没有任何「部分空出」且不与已占用布局重叠的候选。
#[must_use]
pub fn select_future_layout_reservation(
    current_empty_land_ids: &[i64],
    all_eligible_land_ids: &[i64],
    plant_size: usize,
    options: &SelectFutureLayoutOptions,
) -> Option<FutureLayoutReservation> {
    let size = plant_size.max(1);
    if size <= 1 {
        return None;
    }

    let empty_ids: HashSet<i64> =
        current_empty_land_ids.iter().copied().filter(|&id| id > 0).collect();
    if empty_ids.is_empty() {
        return None;
    }
    // 空地已能连成完整布局：直接种，无需预留
    let empty_vec: Vec<i64> = empty_ids.iter().copied().collect();
    if !build_planting_layouts(&empty_vec, size).is_empty() {
        return None;
    }

    let excluded: HashSet<i64> =
        options.exclude_land_ids.iter().copied().filter(|&id| id > 0).collect();
    let registered_key = layout_key(options.registered.as_ref());

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
    // 只保留「部分空出」的布局：全空说明能直接种；全不空无法帮助收敛。
    // 与其他种子已登记目标布局重叠的候选直接排除（#89：整组目标布局互不重叠）。
    .filter(|c| {
        !c.reserved_land_ids.is_empty()
            && c.reserved_land_ids.len() < c.layout.land_ids.len()
            && !c.layout.land_ids.iter().any(|id| excluded.contains(id))
    })
    .collect();

    candidates.sort_by(|a, b| {
        // 1. 已预留格数多者优先（只会向更满的布局收敛）
        let by_reserved = b.reserved_land_ids.len().cmp(&a.reserved_land_ids.len());
        if by_reserved != Ordering::Equal {
            return by_reserved;
        }
        // 2. 格数相同时优先沿用已登记布局（轮次间不回切）
        if !registered_key.is_empty() {
            let a_reg = layout_key(Some(&a.layout)) == registered_key;
            let b_reg = layout_key(Some(&b.layout)) == registered_key;
            if a_reg != b_reg {
                // 命中登记的排前面
                return if a_reg { Ordering::Less } else { Ordering::Greater };
            }
        }
        // 3. 仍相同取锚点最小，保证结果稳定
        a.layout.anchor_land_id.cmp(&b.layout.anchor_land_id)
    });
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut ids: Vec<i64>) -> Vec<i64> {
        ids.sort_unstable();
        ids
    }

    /// bot 场景：empty=[1,2,3]、all=1..15、size=2 → 选 anchor 5 的布局 [5,6,1,2]，
    /// 预留其中已空出的 [1,2]。
    #[test]
    fn reserves_earliest_stable_layout() {
        let all: Vec<i64> = (1..=15).collect();
        let r = select_future_layout_reservation(&[1, 2, 3], &all, 2, &Default::default())
            .expect("reservation expected");
        assert_eq!(r.layout.anchor_land_id, 5);
        assert_eq!(r.layout.land_ids, vec![5, 6, 1, 2]);
        assert_eq!(r.reserved_land_ids, vec![1, 2]);
    }

    /// 场景：empty=[1,2,5] → 同一布局，预留全部已空出的部分 [5,1,2]。
    #[test]
    fn keeps_accumulating_empty_lands() {
        let all: Vec<i64> = (1..=15).collect();
        let r = select_future_layout_reservation(&[1, 2, 5], &all, 2, &Default::default())
            .expect("reservation");
        assert_eq!(r.layout.anchor_land_id, 5);
        assert_eq!(r.reserved_land_ids, vec![5, 1, 2]);
    }

    /// #89：还没有登记时优先选「已预留格数更多」的布局。
    /// empty=[1,3,4,7]、all=1..15：anchor 7 布局 [3,4,7,8] 可预留 3 块，多于 anchor 5 的 1 块。
    #[test]
    fn prefers_the_fuller_layout_when_nothing_is_registered_yet() {
        let all: Vec<i64> = (1..=15).collect();
        let r = select_future_layout_reservation(&[1, 3, 4, 7], &all, 2, &Default::default())
            .expect("reservation");
        assert_eq!(r.layout.anchor_land_id, 7);
        assert_eq!(sorted(r.layout.land_ids), vec![3, 4, 7, 8]);
        assert_eq!(sorted(r.reserved_land_ids), vec![3, 4, 7]);
    }

    /// bot issue #89 回归：上一轮只有 [3] 空时选了某布局；下一轮 4 号地也空了，
    /// 必须换到能同时容纳 [3,4] 的更满布局，而不是丢弃后空出的 4 号地。
    #[test]
    fn issue_89_later_emptied_land_joins_the_reservation() {
        let all_24: Vec<i64> = (1..=24).collect();
        let first =
            select_future_layout_reservation(&[3], &all_24, 2, &Default::default()).unwrap();
        assert_eq!(first.reserved_land_ids, vec![3]);

        let second = select_future_layout_reservation(
            &[3, 4],
            &all_24,
            2,
            &SelectFutureLayoutOptions {
                registered: Some(first.layout.clone()),
                exclude_land_ids: Vec::new(),
            },
        )
        .expect("second reservation");
        assert_eq!(second.layout.anchor_land_id, 7);
        assert_eq!(sorted(second.layout.land_ids), vec![3, 4, 7, 8]);
        assert_eq!(sorted(second.reserved_land_ids), vec![3, 4]);
    }

    /// 格数相同时必须沿用已登记布局，不能回切到更小锚点的布局。
    #[test]
    fn keeps_the_registered_layout_when_an_alternative_is_not_any_fuller() {
        let all_24: Vec<i64> = (1..=24).collect();
        // 空地只有 [3] 时，锚点 6 与锚点 7 两组布局都只能预留 [3]，无登记时取锚点更小的 6。
        let unregistered =
            select_future_layout_reservation(&[3], &all_24, 2, &Default::default()).unwrap();
        assert_eq!(unregistered.layout.anchor_land_id, 6);

        // 已登记的是锚点更大的那一组：必须沿用。
        let registered = PlantingLayout { anchor_land_id: 7, land_ids: vec![7, 8, 3, 4] };
        let sticky = select_future_layout_reservation(
            &[3],
            &all_24,
            2,
            &SelectFutureLayoutOptions {
                registered: Some(registered),
                exclude_land_ids: Vec::new(),
            },
        )
        .expect("sticky reservation");
        assert_eq!(sticky.layout.anchor_land_id, 7);
        assert_eq!(sticky.reserved_land_ids, vec![3]);
    }

    /// 登记后绝不回切到「更不满」的布局。
    #[test]
    fn never_switches_to_a_less_full_layout_than_the_registered_one() {
        let all_24: Vec<i64> = (1..=24).collect();
        let registered = select_future_layout_reservation(&[3, 4], &all_24, 2, &Default::default())
            .expect("registered");
        let next = select_future_layout_reservation(
            &[3, 4, 9],
            &all_24,
            2,
            &SelectFutureLayoutOptions {
                registered: Some(registered.layout.clone()),
                exclude_land_ids: Vec::new(),
            },
        )
        .expect("next");
        assert!(next.reserved_land_ids.len() >= registered.reserved_land_ids.len());
    }

    /// 与其他种子已占用的整组目标布局重叠的候选必须跳过；全部被占时不预留。
    #[test]
    fn skips_candidate_layouts_that_overlap_lands_claimed_by_another_seed() {
        let all_24: Vec<i64> = (1..=24).collect();
        let base = select_future_layout_reservation(&[3, 4], &all_24, 2, &Default::default())
            .expect("base");
        let claimed = base.layout.land_ids.clone();
        let other = select_future_layout_reservation(
            &[3, 4],
            &all_24,
            2,
            &SelectFutureLayoutOptions { registered: None, exclude_land_ids: claimed.clone() },
        );
        if let Some(other) = other {
            assert!(!other.layout.land_ids.iter().any(|id| claimed.contains(id)));
        }
        assert_eq!(
            select_future_layout_reservation(
                &[3, 4],
                &all_24,
                2,
                &SelectFutureLayoutOptions { registered: None, exclude_land_ids: all_24.clone() },
            ),
            None
        );
    }

    /// 场景：empty=[1,2,5,6] 已能连成完整布局 → 直接种，不预留。
    #[test]
    fn does_not_reserve_when_layout_already_plantable() {
        let all: Vec<i64> = (1..=15).collect();
        assert_eq!(
            select_future_layout_reservation(&[1, 2, 5, 6], &all, 2, &Default::default()),
            None
        );
    }

    /// 单格作物不预留；4 块地里凑不出的布局也不预留。
    #[test]
    fn does_not_reserve_single_land_or_impossible_layouts() {
        let all: Vec<i64> = (1..=15).collect();
        assert_eq!(
            select_future_layout_reservation(&[1, 2, 3], &all, 1, &Default::default()),
            None
        );
        assert_eq!(
            select_future_layout_reservation(&[1, 2, 3], &[1, 2, 3, 4], 2, &Default::default()),
            None
        );
    }

    /// 空地列表为空 → 不预留。
    #[test]
    fn does_not_reserve_without_empty_lands() {
        let all: Vec<i64> = (1..=15).collect();
        assert_eq!(select_future_layout_reservation(&[], &all, 2, &Default::default()), None);
    }
}
