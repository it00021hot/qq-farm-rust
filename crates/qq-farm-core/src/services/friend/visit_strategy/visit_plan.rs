//! 好友单次访问计划（纯函数，可单测）。
//!
//! 对齐 bot `core/src/services/friend/visit-plan.ts`（HEAD=8dae528）。
//!
//! 以前好友巡查分三段跑：先「只偷菜」、再「只帮忙」、最后「只捣乱」，
//! 每段各自 Enter/Leave。既有可偷又需要帮忙的好友因此被进两次农场，
//! 200 位好友一轮下来能刷出几百个 Enter/Leave，网关直接被打满。
//!
//! 现在先算出「每位好友这一轮要做哪几件事」，再对每位好友只进一次农场
//! （[`super::combined::visit_friend_combined`]），在里面把
//! 帮助（除草/除虫/浇水）+ 偷菜 + 捣乱（放草/放虫）一次做完。
//!
//! 过滤规则（visit-plan.ts:13-18）：
//! - 捣乱额度用完 → 这一轮不捣乱；
//! - 经验已满且「经验满不帮」开着 → 不帮；
//! - 经验已满但「护主犬无视经验上限」开着 → 只帮当天缓存已确认是护主犬的好友；
//! - 好友宠物还没同步（缓存里没有结论）且经验已满 → 不进农场，交给每日宠物同步补齐；
//! - 没有任何事可做的好友一个请求都不发。

use std::collections::HashSet;

use crate::services::friend::pet_cache::FriendDogState;

/// 一轮最多对多少位「没可偷也没可帮」的好友做纯捣乱访问
/// （bot scheduler.ts:71 `MAX_BAD_ONLY_VISITS_PER_ROUND = 20`）
pub const MAX_BAD_ONLY_VISITS_PER_ROUND: usize = 20;

/// 计划输入的好友摘要（气泡数字已由调用方按 cleared/hints 修正）
#[derive(Debug, Clone)]
pub struct PlanFriend {
    pub gid: i64,
    pub name: String,
    pub level: i64,
    /// 生效的可偷数（偷菜成功后暂清零的值）
    pub steal_num: i64,
    /// 帮忙需求数（dry+weed+insect）
    pub help_num: i64,
    /// GetAll 原始可偷数（用于偷菜空访标记，rust 增强）
    pub live_steal_num: i64,
}

/// 单个访问目标
#[derive(Debug, Clone)]
pub struct VisitTarget {
    pub gid: i64,
    pub name: String,
    pub level: i64,
    pub steal_num: i64,
    pub help_num: i64,
    pub live_steal_num: i64,
    pub want_steal: bool,
    pub want_help: bool,
    pub want_bad: bool,
}

/// 一轮巡查计划
#[derive(Debug, Clone, Default)]
pub struct VisitPlan {
    /// 访问顺序：primary（偷/帮）在前，纯捣乱在队尾
    pub visits: Vec<VisitTarget>,
    pub steal_count: usize,
    pub help_count: usize,
    pub bad_only_count: usize,
    /// 经验已满而被跳过（未进农场）的好友数
    pub skipped_exp_limit: usize,
    /// 其中因为「宠物还没同步」无法确认护主犬的好友数
    pub skipped_unknown_dog: usize,
}

/// 计算一轮巡查要访问谁、每位好友做哪几件事。
///
/// - `help_allowed_for_all`：经验没满（或本次显式忽略经验上限）时对所有好友都可以帮；
/// - `protect_dog_bypass_enabled`：「护主犬无视经验上限」开关；
/// - `get_dog_state`：当天宠物缓存结论；
/// - `bad_budget`：剩余捣乱次数，`<= 0` 表示这一轮不捣乱。
#[must_use]
pub fn build_friend_visit_plan(
    friends: &[PlanFriend],
    my_gid: i64,
    blacklist: &HashSet<i64>,
    steal_enabled: bool,
    help_enabled: bool,
    bad_enabled: bool,
    help_allowed_for_all: bool,
    protect_dog_bypass_enabled: bool,
    get_dog_state: &dyn Fn(i64) -> FriendDogState,
    bad_budget: i64,
    max_bad_only_visits: usize,
) -> VisitPlan {
    let bad_allowed = bad_enabled && bad_budget > 0 && max_bad_only_visits > 0;
    let mut primary: Vec<VisitTarget> = Vec::new();
    let mut bad_only: Vec<VisitTarget> = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();
    let mut skipped_exp_limit = 0usize;
    let mut skipped_unknown_dog = 0usize;

    for friend in friends {
        let gid = friend.gid;
        if gid <= 0 || gid == my_gid || !seen.insert(gid) {
            continue;
        }
        if blacklist.contains(&gid) {
            continue;
        }

        let steal_num = friend.steal_num;
        let help_num = friend.help_num;

        let want_steal = steal_enabled && steal_num > 0;
        let mut want_help = help_enabled && help_num > 0;
        if want_help && !help_allowed_for_all {
            // 经验已满：只有「护主犬无视经验上限」开着、且当天缓存已确认是护主犬
            // 时才值得进农场（visit-plan.ts:105-115）。
            let dog_state = get_dog_state(gid);
            let bypass = protect_dog_bypass_enabled && dog_state == FriendDogState::Protect;
            if !bypass {
                want_help = false;
                skipped_exp_limit += 1;
                // 宠物没同步的好友这一轮不试探，等每日宠物同步给出结论
                if protect_dog_bypass_enabled && dog_state == FriendDogState::Unknown {
                    skipped_unknown_dog += 1;
                }
            }
        }

        let target = VisitTarget {
            gid,
            name: friend.name.clone(),
            level: friend.level,
            steal_num,
            help_num,
            live_steal_num: friend.live_steal_num,
            want_steal,
            want_help,
            want_bad: false,
        };

        if want_steal || want_help {
            primary.push(target);
            continue;
        }
        // 既没可偷也没可帮的好友才是捣乱对象：和旧逻辑一致，不在偷/帮的访问里
        // 顺手放草放虫，免得每日捣乱额度被花在错误的好友身上（visit-plan.ts:136-139）。
        if bad_allowed && steal_num == 0 && help_num == 0 {
            bad_only.push(target);
        }
    }

    // 偷得多的先走，其次是帮助需求大的，最后按等级（visit-plan.ts:143）
    primary.sort_by(|a, b| {
        b.steal_num.cmp(&a.steal_num).then(b.help_num.cmp(&a.help_num)).then(b.level.cmp(&a.level))
    });
    // 捣乱优先挑等级高的好友（visit-plan.ts:145）
    bad_only.sort_by(|a, b| b.level.cmp(&a.level));
    bad_only.truncate(max_bad_only_visits);
    for target in &mut bad_only {
        target.want_bad = true;
    }

    let steal_count = primary.iter().filter(|t| t.want_steal).count();
    let help_count = primary.iter().filter(|t| t.want_help).count();
    let bad_only_count = bad_only.len();
    primary.extend(bad_only);
    VisitPlan {
        visits: primary,
        steal_count,
        help_count,
        bad_only_count,
        skipped_exp_limit,
        skipped_unknown_dog,
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn friend(gid: i64, level: i64, steal: i64, help: i64) -> PlanFriend {
        PlanFriend {
            gid,
            name: format!("F{gid}"),
            level,
            steal_num: steal,
            help_num: help,
            live_steal_num: steal,
        }
    }

    fn plan_with(
        friends: &[PlanFriend],
        help_allowed_for_all: bool,
        bypass: bool,
    ) -> VisitPlan {
        build_friend_visit_plan(
            friends,
            1,
            &HashSet::new(),
            true,
            true,
            true,
            help_allowed_for_all,
            bypass,
            &|_| FriendDogState::Unknown,
            10,
            MAX_BAD_ONLY_VISITS_PER_ROUND,
        )
    }

    #[test]
    fn plan_sorts_steal_first_then_help_then_level() {
        let friends = vec![
            friend(10, 5, 0, 2),
            friend(11, 9, 3, 0),
            friend(12, 2, 3, 0),
            friend(13, 8, 1, 1),
        ];
        let plan = plan_with(&friends, true, false);
        let gids: Vec<i64> = plan.visits.iter().map(|t| t.gid).collect();
        // 可偷 3 的两位按等级降序在前，可偷 1 其次，纯帮忙最后
        assert_eq!(gids, vec![11, 12, 13, 10]);
        assert_eq!(plan.steal_count, 3);
        assert_eq!(plan.help_count, 2);
        assert_eq!(plan.bad_only_count, 0);
    }

    #[test]
    fn plan_bad_only_picked_by_level_and_capped() {
        let mut friends: Vec<PlanFriend> = Vec::new();
        for i in 0..30 {
            friends.push(friend(100 + i, i, 0, 0));
        }
        let plan = plan_with(&friends, true, false);
        assert_eq!(plan.bad_only_count, MAX_BAD_ONLY_VISITS_PER_ROUND);
        // 等级最高（gid=129）在前
        assert_eq!(plan.visits[0].gid, 129);
        assert!(plan.visits.iter().all(|t| t.want_bad));
    }

    #[test]
    fn plan_bad_disabled_without_budget() {
        let friends = vec![friend(10, 5, 0, 0)];
        let plan = build_friend_visit_plan(
            &friends,
            1,
            &HashSet::new(),
            true,
            true,
            true,
            true,
            false,
            &|_| FriendDogState::Unknown,
            0,
            MAX_BAD_ONLY_VISITS_PER_ROUND,
        );
        assert!(plan.visits.is_empty());
    }

    #[test]
    fn plan_exp_limit_skips_unknown_dog_friends() {
        let friends = vec![friend(10, 5, 0, 3), friend(11, 6, 0, 2)];
        let plan = plan_with(&friends, false, true);
        // 经验满 + bypass 开但缓存全是 unknown → 全部跳过
        assert!(plan.visits.is_empty());
        assert_eq!(plan.skipped_exp_limit, 2);
        assert_eq!(plan.skipped_unknown_dog, 2);
    }

    #[test]
    fn plan_exp_limit_keeps_protect_dog_friends() {
        let friends = vec![friend(10, 5, 0, 3), friend(11, 6, 0, 2), friend(12, 1, 0, 1)];
        let plan = build_friend_visit_plan(
            &friends,
            1,
            &HashSet::new(),
            true,
            true,
            false,
            false,
            true,
            &|gid| if gid == 10 { FriendDogState::Protect } else { FriendDogState::Other },
            10,
            MAX_BAD_ONLY_VISITS_PER_ROUND,
        );
        // 只有护主犬好友保留 wantHelp；other 算 skippedExpLimit 但不算 unknown
        assert_eq!(plan.help_count, 1);
        assert_eq!(plan.visits[0].gid, 10);
        assert_eq!(plan.skipped_exp_limit, 2);
        assert_eq!(plan.skipped_unknown_dog, 0);
    }

    #[test]
    fn plan_exp_limit_without_bypass_skips_all_help() {
        let friends = vec![friend(10, 5, 0, 3)];
        let plan = build_friend_visit_plan(
            &friends,
            1,
            &HashSet::new(),
            true,
            true,
            false,
            false,
            false,
            &|_| FriendDogState::Protect,
            10,
            MAX_BAD_ONLY_VISITS_PER_ROUND,
        );
        assert!(plan.visits.is_empty());
        // bypass 关闭时不算「宠物待确认」
        assert_eq!(plan.skipped_exp_limit, 1);
        assert_eq!(plan.skipped_unknown_dog, 0);
    }

    #[test]
    fn plan_blacklist_and_self_excluded() {
        let friends = vec![friend(1, 5, 3, 0), friend(2, 5, 3, 0), friend(3, 5, 3, 0)];
        let blacklist: HashSet<i64> = [2].into_iter().collect();
        let plan = build_friend_visit_plan(
            &friends,
            1,
            &blacklist,
            true,
            true,
            false,
            true,
            false,
            &|_| FriendDogState::Unknown,
            10,
            MAX_BAD_ONLY_VISITS_PER_ROUND,
        );
        let gids: Vec<i64> = plan.visits.iter().map(|t| t.gid).collect();
        assert_eq!(gids, vec![3]);
    }

    #[test]
    fn plan_dedupes_by_gid() {
        let friends = vec![friend(10, 5, 2, 0), friend(10, 5, 2, 0)];
        let plan = plan_with(&friends, true, false);
        assert_eq!(plan.visits.len(), 1);
    }

    #[test]
    fn plan_bad_not_attached_to_steal_or_help_targets() {
        let friends = vec![friend(10, 5, 2, 0)];
        let plan = plan_with(&friends, true, false);
        assert!(plan.visits[0].want_steal);
        assert!(!plan.visits[0].want_bad);
    }
}
