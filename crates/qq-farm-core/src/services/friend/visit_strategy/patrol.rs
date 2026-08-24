//! 好友偷菜巡逻：气泡优先 + 零气泡自巡（对齐 Go `friend_patrol.go`）。
//!
//! 微信 GetAll 的 `steal_plant_num` 好友一多就会漏推；自巡补漏报。

use std::collections::HashSet;

/// `ceil(n/4)`，n>0 时至少 1。
#[must_use]
pub fn get_patrol_batch_size(friend_count: usize) -> usize {
    if friend_count == 0 {
        return 0;
    }
    let n = (friend_count + 3) / 4;
    n.max(1)
}

/// 从未访问候选里取最多 `budget` 个；全访问过则清空 `visited` 再从头取。
#[must_use]
pub fn select_unvisited_patrol(
    candidates: &[i64],
    budget: usize,
    visited: &mut HashSet<i64>,
) -> Vec<i64> {
    if candidates.is_empty() || budget == 0 {
        return Vec::new();
    }
    let mut unmarked: Vec<i64> =
        candidates.iter().copied().filter(|gid| *gid > 0 && !visited.contains(gid)).collect();
    if unmarked.is_empty() {
        visited.clear();
        unmarked = candidates.iter().copied().filter(|gid| *gid > 0).collect();
    }
    if unmarked.len() > budget {
        unmarked.truncate(budget);
    }
    unmarked
}

/// 气泡（steal>0）按数量降序。对齐 bot friend/scheduler.ts:309-321：
/// 只访问有可偷气泡的好友，不做零气泡探测——零气泡"进场即走"模式 bot 从不产生，
/// 属于服务端行为识别的高危指纹。
#[must_use]
pub fn build_steal_patrol_targets(eligible: &[(i64, i64)], _visited: &mut HashSet<i64>) -> Vec<i64> {
    let mut bubble: Vec<(i64, i64)> = Vec::new();
    for &(gid, steal) in eligible {
        if gid <= 0 {
            continue;
        }
        if steal > 0 {
            bubble.push((gid, steal));
        }
    }
    bubble.sort_by(|a, b| b.1.cmp(&a.1));
    bubble.into_iter().map(|(gid, _)| gid).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrol_batch_size_ceil_n_over_4() {
        assert_eq!(get_patrol_batch_size(0), 0);
        assert_eq!(get_patrol_batch_size(1), 1);
        assert_eq!(get_patrol_batch_size(4), 1);
        assert_eq!(get_patrol_batch_size(5), 2);
        assert_eq!(get_patrol_batch_size(9), 3);
    }

    #[test]
    fn select_unvisited_resets_when_exhausted() {
        let mut visited = HashSet::from([1, 2, 3]);
        let got = select_unvisited_patrol(&[1, 2, 3, 4], 2, &mut visited);
        assert_eq!(got, vec![4]);

        let mut visited = HashSet::from([1, 2, 3, 4]);
        let got = select_unvisited_patrol(&[1, 2, 3, 4], 2, &mut visited);
        assert_eq!(got, vec![1, 2]);
        assert!(visited.is_empty());
    }

    #[test]
    fn steal_targets_bubble_only() {
        let eligible = vec![(10, 2), (11, 5), (12, 0), (13, 0), (14, 0), (15, 0)];
        let mut visited = HashSet::new();
        let targets = build_steal_patrol_targets(&eligible, &mut visited);
        // 对齐 bot：只有气泡好友（按数量降序），零气泡不探测
        assert_eq!(targets, vec![11, 10]);
    }
}
