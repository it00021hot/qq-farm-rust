//! 统一好友单次访问 —— 一次 Enter 把 帮助 + 偷菜 + 捣乱 做完。
//!
//! 对齐 bot `core/src/services/friend/visit-strategy.ts` 的 `visitFriend`
//! （HEAD=8dae528，visit-strategy.ts:743-898）。
//!
//! 三件事都不需要做时连 Enter 都不发（visit-strategy.ts:770-777）；
//! 经验满之后唯一还值得帮忙的对象是挂着护主犬的好友（同气连枝礼包），
//! 进场前用当天缓存判定，进场后再用 Enter 回包兜底。

use std::sync::atomic::AtomicBool;

use crate::services::friend::api::FriendApi;
use crate::services::friend::pet_cache::{get_friend_dog_state, FriendDogState};

use super::blacklist::{get_plant_blacklist, handle_friend_enter_error, FriendEnterErrorKind};
use super::help::{
    perform_bad_actions, perform_help_actions, LandSnapshot, RecentHelpCache, TotalActions,
    VisitResult,
};
use super::panel_dto::FriendSummary;
use super::steal::{analyze_friend_lands, perform_steal_actions};

/// 一次进好友农场，把 帮助（除草/除虫/浇水）+ 偷菜 + 捣乱（放草/放虫）一次做完。
///
/// - `allow_steal` / `allow_help` / `allow_bad`：本轮计划里这位好友要做哪几件事
///   （来自 [`super::visit_plan::build_friend_visit_plan`]）；
/// - `ignore_exp_limit`：调用方显式忽略经验上限（面板手动触发等）；
/// - `help_auto_disabled`：帮忙经验满标记（帮忙经验探测用）；
/// - `can_get_exp_by_candidates`：10005/10006/10007 是否还有经验可拿。
///
/// 偷菜动作复用 [`perform_steal_actions`]、帮忙复用 [`perform_help_actions`]、
/// 捣乱复用 [`perform_bad_actions`]，与仅偷/仅帮路径共享同一套逻辑。
#[allow(clippy::too_many_arguments)]
pub async fn visit_friend_combined(
    api: &FriendApi,
    recent_help: &RecentHelpCache,
    friend: &FriendSummary,
    total_actions: &mut TotalActions,
    my_gid: i64,
    account_id: &str,
    allow_steal: bool,
    allow_help: bool,
    allow_bad: bool,
    ignore_exp_limit: bool,
    help_auto_disabled: &AtomicBool,
    can_get_exp_by_candidates: bool,
) -> VisitResult {
    use crate::services::automation::is_automation_on_for;

    let friend_gid = friend.gid;
    let friend_name = friend.name.clone();

    let steal_enabled = allow_steal && is_automation_on_for(account_id, "friend_steal");
    let bad_enabled = allow_bad && is_automation_on_for(account_id, "friend_bad");
    let stop_when_exp_limit =
        is_automation_on_for(account_id, "friend_help_exp_limit") && !ignore_exp_limit;
    if !stop_when_exp_limit {
        help_auto_disabled.store(false, std::sync::atomic::Ordering::Release);
    }
    let protect_dog_bypass_enabled =
        is_automation_on_for(account_id, "friend_help_protect_dog_ignore_exp_limit");
    let exp_limit_reached = stop_when_exp_limit
        && help_auto_disabled.load(std::sync::atomic::Ordering::Acquire);
    // 经验满之后唯一还值得帮忙的对象是挂着护主犬的好友；护主犬只能从 Enter 回包
    // 读到，所以这里只查当天缓存，不逐个进农场试探（bot visit-strategy.ts:760-768）。
    let help_blocked_by_exp_limit = exp_limit_reached
        && (!protect_dog_bypass_enabled
            || get_friend_dog_state(account_id, friend_gid) != FriendDogState::Protect);
    let help_enabled =
        allow_help && is_automation_on_for(account_id, "friend_help") && !help_blocked_by_exp_limit;

    if !steal_enabled && !bad_enabled && !help_enabled {
        // 这一轮对这位好友无事可做：一个请求都不发（bot visit-strategy.ts:770-777）
        return VisitResult { acted: false, entered: false, stolen: 0 };
    }

    let enter_reply = match api.enter_farm(friend_gid).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e}");
            let kind = handle_friend_enter_error(account_id, friend_gid, &friend_name, &msg);
            if kind != FriendEnterErrorKind::Error {
                return VisitResult { acted: false, entered: false, stolen: 0 };
            }
            crate::services::panel_log::log_warn(
                account_id,
                "好友",
                format!("进入 {friend_name} 农场失败: {msg}"),
                crate::constants::PanelEvent::EnterFarm,
                Some(serde_json::json!({
                    "module": "friend",
                    "result": "error",
                    "friendName": friend_name,
                    "friendGid": friend_gid,
                })),
            );
            return VisitResult { acted: false, entered: false, stolen: 0 };
        }
    };

    let lands = enter_reply.lands.clone();
    if lands.is_empty() {
        let _ = api.leave_farm(friend_gid).await;
        return VisitResult { acted: false, entered: true, stolen: 0 };
    }

    let plant_blacklist = get_plant_blacklist(account_id);
    let mut status = analyze_friend_lands(&lands, my_gid, &plant_blacklist, false, account_id);
    let snapshot_key = RecentHelpCache::make_snapshot_key(
        &lands.iter().map(LandSnapshot::from_land).collect::<Vec<_>>(),
    );

    let mut actions: Vec<String> = Vec::new();
    let mut stolen = 0usize;

    // 1. 帮助操作（除草/除虫/浇水，bot visit-strategy.ts:808-832）
    if help_enabled {
        // Enter 回包确认护主犬 → 本好友无视经验上限（bot visit-strategy.ts:809-810），
        // 覆盖「经验在一轮中途满掉」的情况（缓存过滤挡不住的兜底闸门）。
        let effective_stop = stop_when_exp_limit
            && !(protect_dog_bypass_enabled
                && enter_reply.brief_dog_info.as_ref().map(|d| d.dog_id)
                    == Some(crate::services::friend::pet_cache::PROTECT_DOG_ID));
        perform_help_actions(
            api,
            recent_help,
            account_id,
            friend_gid,
            &status,
            &snapshot_key,
            effective_stop,
            Some(help_auto_disabled),
            can_get_exp_by_candidates,
            total_actions,
            &mut actions,
        )
        .await;
    }

    // 2. 偷菜操作（bot visit-strategy.ts:834-869）
    if steal_enabled && !status.stealable.is_empty() {
        stolen = perform_steal_actions(
            api,
            recent_help,
            account_id,
            friend_gid,
            &mut status,
            total_actions,
            &mut actions,
        )
        .await;
    }

    // 3. 捣乱操作（放虫/放草，bot visit-strategy.ts:871-888）
    if bad_enabled {
        perform_bad_actions(api, friend_gid, &status, total_actions, &mut actions).await;
    }

    if !actions.is_empty() {
        crate::services::panel_log::log(
            account_id,
            "好友",
            format!("{friend_name}: {}", actions.join("/")),
            crate::constants::PanelEvent::VisitFriend,
            Some(serde_json::json!({
                "module": "friend",
                "result": "ok",
                "friendName": friend_name,
                "friendGid": friend_gid,
                "actions": actions,
            })),
        );
    }

    let _ = api.leave_farm(friend_gid).await;
    VisitResult { acted: !actions.is_empty(), entered: true, stolen }
}
