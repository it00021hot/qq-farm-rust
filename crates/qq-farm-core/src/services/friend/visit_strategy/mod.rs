//! 拜访好友策略 —— 1:1 翻译原 `core/src/services/friend/visit-strategy.ts`。
//!
//! 核心：避免重复帮同一块地（recent help 去重）+ 错误分类 + 帮助/偷菜/捣乱。
//!
//! 含：RecentHelp 状态机、安静时段、好友/植物黑名单、空访跳过、stealers 过滤。

mod blacklist;
mod cache;
mod help;
mod panel_dto;
mod quiet_hours;
mod steal;

pub use blacklist::*;
pub use cache::*;
pub use help::*;
pub use panel_dto::*;
pub use quiet_hours::*;
pub use steal::*;

pub use crate::constants::{HELP_CACHE_MAX, HELP_IN_FLIGHT_TTL_MS, HELP_RESULT_TTL_MS};

/// 当前时间（毫秒，Unix epoch）—— 测试时可注入
pub type ClockMs = u64;

#[must_use]
pub fn now_ms() -> ClockMs {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use crate::proto::generated::gamepb::plantpb::LandInfo;

    use super::*;

    #[test]
    fn make_key_format() {
        assert_eq!(RecentHelpCache::make_key(100, 1), "100:1");
        assert_eq!(RecentHelpCache::make_key(-1, 1), "-1:1");
    }

    #[test]
    fn game_friend_dto_has_name_avatar_plant() {
        let f = crate::proto::generated::gamepb::friendpb::GameFriend {
            gid: 9,
            name: "张三".into(),
            avatar_url: "http://avatar".into(),
            level: 10,
            gold: 100,
            plant: Some(crate::proto::generated::gamepb::friendpb::Plant {
                steal_plant_num: 2,
                dry_num: 1,
                weed_num: 0,
                insect_num: 3,
                ..Default::default()
            }),
            ..Default::default()
        };
        let v = serde_json::to_value(game_friend_to_summary(f)).unwrap();
        assert_eq!(v["gid"], 9);
        assert_eq!(v["name"], "张三");
        assert_eq!(v["avatarUrl"], "http://avatar");
        assert_eq!(v["plant"]["stealNum"], 2);
        assert_eq!(v["plant"]["dryNum"], 1);
    }

    #[test]
    fn snapshot_key_basic() {
        let lands = vec![
            LandSnapshot {
                id: 1,
                plant_id: 10,
                phase: 2,
                dry_num: 0,
                weed_owners: vec![],
                insect_owners: vec![],
            },
            LandSnapshot {
                id: 2,
                plant_id: 10,
                phase: 3,
                dry_num: 1,
                weed_owners: vec![100],
                insect_owners: vec![],
            },
        ];
        let key = RecentHelpCache::make_snapshot_key(&lands);
        assert_eq!(key, "1:10:2:0::|2:10:3:1:100:");
    }

    #[test]
    fn filter_empty_cache_returns_all() {
        let cache = RecentHelpCache::new();
        let lands = vec![1, 2, 3];
        let snap = "x";
        let result = cache.filter(100, &lands, snap, 1000);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn filter_removes_already_helped_with_same_snapshot() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1, 2], HelpState::Confirmed, 30_000, "snap1", 1000);
        let result = cache.filter(100, &[1, 2, 3], "snap1", 1100);
        assert_eq!(result, vec![3]);
    }

    #[test]
    fn filter_keeps_when_snapshot_changed() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1], HelpState::Confirmed, 30_000, "snap_old", 1000);
        let result = cache.filter(100, &[1], "snap_new", 1100);
        assert_eq!(result, vec![1]);
        assert!(cache.get(100, 1).is_none());
    }

    #[test]
    fn filter_expired_entries_pass_through() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1], HelpState::Confirmed, 100, "snap", 1000);
        let result = cache.filter(100, &[1], "snap", 1500);
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn filter_dedupes_input() {
        let cache = RecentHelpCache::new();
        let result = cache.filter(100, &[1, 1, 2, 2, 3], "snap", 1000);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn filter_skips_non_positive_ids() {
        let cache = RecentHelpCache::new();
        let result = cache.filter(100, &[0, -1, 1, 2], "snap", 1000);
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn mark_and_release() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1, 2], HelpState::InFlight, 15_000, "snap", 1000);
        assert_eq!(cache.len(), 2);
        cache.release(100, &[1]);
        assert_eq!(cache.len(), 1);
        assert!(cache.get(100, 1).is_none());
        assert!(cache.get(100, 2).is_some());
    }

    #[test]
    fn prune_removes_expired() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1], HelpState::Confirmed, 100, "snap", 1000);
        cache.mark(200, &[2], HelpState::Confirmed, 200, "snap", 1000);
        cache.prune(1100);
        assert!(cache.get(100, 1).is_none());
        assert!(cache.get(200, 2).is_some());
    }

    #[test]
    fn prune_lru_caps_size() {
        let cache = RecentHelpCache::new();
        for i in 0..(HELP_CACHE_MAX + 10) {
            cache.mark(100, &[i as i64], HelpState::Confirmed, 30_000, "snap", 0);
        }
        cache.prune(0);
        assert_eq!(cache.len(), HELP_CACHE_MAX);
    }

    #[test]
    fn different_host_gids_isolated() {
        let cache = RecentHelpCache::new();
        cache.mark(100, &[1], HelpState::Confirmed, 30_000, "snap", 1000);
        let result = cache.filter(200, &[1], "snap", 1100);
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn enter_farm_banned_error_detected() {
        assert!(is_enter_farm_banned_error("gate error: code=1002003 禁止进入"));
        assert!(!is_enter_farm_banned_error("some other error"));
        assert!(!is_enter_farm_banned_error(""));
    }

    #[test]
    fn parse_rpc_error_code_extracts() {
        assert_eq!(parse_rpc_error_code("error: code=1002003 msg"), 1002003);
        assert_eq!(parse_rpc_error_code("error: code=42"), 42);
        assert_eq!(parse_rpc_error_code("no code here"), 0);
        assert_eq!(parse_rpc_error_code("code=99999999 at position"), 99999999);
    }

    #[test]
    fn transient_network_error_detected() {
        assert!(is_transient_network_error("连接未打开"));
        assert!(is_transient_network_error("请求超时: foo"));
        assert!(is_transient_network_error("request timeout: foo"));
        assert!(is_transient_network_error("连接关闭 (code=1006)"));
        assert!(is_transient_network_error("worker exited"));
        assert!(!is_transient_network_error("业务错误"));
        assert!(!is_transient_network_error(""));
    }

    #[test]
    fn parse_time_to_minutes_basic() {
        assert_eq!(parse_time_to_minutes("00:00"), Some(0));
        assert_eq!(parse_time_to_minutes("12:30"), Some(12 * 60 + 30));
        assert_eq!(parse_time_to_minutes("23:59"), Some(23 * 60 + 59));
        assert_eq!(parse_time_to_minutes("24:00"), None);
        assert_eq!(parse_time_to_minutes("12:60"), None);
        assert_eq!(parse_time_to_minutes("12"), None);
        assert_eq!(parse_time_to_minutes(""), None);
    }

    #[test]
    fn in_friend_quiet_hours_disabled_by_default() {
        set_friend_quiet_hours(GLOBAL_QUIET_HOURS_ACCOUNT, None);
        assert!(!in_friend_quiet_hours(Some((10, 0))));
    }

    #[test]
    fn in_friend_quiet_hours_within_window() {
        set_friend_quiet_hours(
            GLOBAL_QUIET_HOURS_ACCOUNT,
            Some(FriendQuietHours {
                enabled: true,
                start: "22:00".to_string(),
                end: "08:00".to_string(),
            }),
        );
        assert!(in_friend_quiet_hours(Some((23, 0))));
        assert!(in_friend_quiet_hours(Some((7, 30))));
        assert!(!in_friend_quiet_hours(Some((10, 0))));
    }

    #[test]
    fn in_friend_quiet_hours_same_window_means_all_day() {
        set_friend_quiet_hours(
            GLOBAL_QUIET_HOURS_ACCOUNT,
            Some(FriendQuietHours {
                enabled: true,
                start: "00:00".to_string(),
                end: "00:00".to_string(),
            }),
        );
        assert!(in_friend_quiet_hours(Some((10, 0))));
    }

    #[test]
    fn blacklist_add_and_remove() {
        add_friend_to_blacklist("", 100, "alice", "test");
        assert!(is_in_blacklist(100));
        assert_eq!(blacklist_size(), 1);
        add_friend_to_blacklist("", 100, "alice", "test");
        assert_eq!(blacklist_size(), 1);
        assert!(remove_from_blacklist(100));
        assert!(!is_in_blacklist(100));
    }

    #[test]
    fn blacklist_add_zero_returns_false() {
        assert!(!add_friend_to_blacklist("", 0, "zero", ""));
    }

    #[test]
    fn invalid_friend_access_error_basic() {
        assert!(!is_invalid_friend_access_error(""));
        assert!(!is_invalid_friend_access_error("code=1002003"));
        assert!(!is_invalid_friend_access_error("连接未打开"));
        assert!(is_invalid_friend_access_error("code=42 invalid friend"));
    }

    #[test]
    fn handle_friend_enter_error_classifies() {
        let k = handle_friend_enter_error("", 200, "bob", "code=1002003");
        assert_eq!(k, FriendEnterErrorKind::Blacklist);
        assert!(is_in_blacklist(200));
        let _ = remove_from_blacklist(200);
        let k2 = handle_friend_enter_error("", 300, "carol", "code=42 invalid friend");
        assert_eq!(k2, FriendEnterErrorKind::InvalidRemoved);
        let k3 = handle_friend_enter_error("", 400, "dave", "连接未打开");
        assert_eq!(k3, FriendEnterErrorKind::Error);
    }

    #[test]
    fn empty_farming_outcome_defaults() {
        let o = empty_farming_outcome(FarmingEffect::Noop);
        assert_eq!(o.effect, FarmingEffect::Noop);
        assert_eq!(o.land_count, 0);
        assert!(o.land_ids.is_empty());
    }

    #[test]
    fn merge_farming_outcomes_aggregates() {
        let outcomes = vec![
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: 2,
                land_count: 1,
                land_ids: vec![1],
                operation_limits: vec![],
                code: 0,
            },
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: 3,
                land_count: 1,
                land_ids: vec![2],
                operation_limits: vec![],
                code: 0,
            },
            FarmingOutcome {
                effect: FarmingEffect::Uncertain,
                operation_count: 0,
                land_count: 0,
                land_ids: vec![],
                operation_limits: vec![],
                code: 0,
            },
        ];
        let merged = merge_farming_outcomes(&outcomes);
        assert_eq!(merged.effect, FarmingEffect::Confirmed);
        assert_eq!(merged.operation_count, 5);
        assert_eq!(merged.land_count, 2);
        assert_eq!(merged.land_ids, vec![1, 2]);
    }

    #[test]
    fn merge_farming_outcomes_only_uncertain() {
        let outcomes = vec![FarmingOutcome {
            effect: FarmingEffect::Uncertain,
            operation_count: 0,
            land_count: 0,
            land_ids: vec![],
            operation_limits: vec![],
            code: 0,
        }];
        let merged = merge_farming_outcomes(&outcomes);
        assert_eq!(merged.effect, FarmingEffect::Uncertain);
    }

    #[test]
    fn merge_farming_outcomes_dedup_land_ids() {
        let outcomes = vec![
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: 1,
                land_count: 1,
                land_ids: vec![1, 2],
                operation_limits: vec![],
                code: 0,
            },
            FarmingOutcome {
                effect: FarmingEffect::Confirmed,
                operation_count: 1,
                land_count: 1,
                land_ids: vec![2, 3],
                operation_limits: vec![],
                code: 0,
            },
        ];
        let merged = merge_farming_outcomes(&outcomes);
        assert_eq!(merged.land_ids, vec![1, 2, 3]);
    }

    #[test]
    fn plant_blacklist_per_account() {
        let a1 = "vs_plant_bl_acc1";
        let a2 = "vs_plant_bl_acc2";
        let a3 = "vs_plant_bl_acc3";
        let _ = crate::models::store::account_config::remove_account_config(a1);
        let _ = crate::models::store::account_config::remove_account_config(a2);
        let _ = crate::models::store::account_config::remove_account_config(a3);
        set_plant_blacklist(a1, vec![100, 200]);
        set_plant_blacklist(a2, vec![300]);
        assert_eq!(get_plant_blacklist(a1), vec![100, 200]);
        assert_eq!(get_plant_blacklist(a2), vec![300]);
        assert_eq!(
            get_plant_blacklist(a3),
            crate::models::store::normalize::default_account_config().plant_blacklist
        );
        let _ = crate::models::store::account_config::remove_account_config(a1);
        let _ = crate::models::store::account_config::remove_account_config(a2);
    }

    #[test]
    fn account_friend_blacklist_per_account() {
        let a1 = "vs_friend_bl_acc1";
        let a2 = "vs_friend_bl_acc2";
        let a3 = "vs_friend_bl_acc3";
        let _ = crate::models::store::account_config::remove_account_config(a1);
        let _ = crate::models::store::account_config::remove_account_config(a2);
        let _ = crate::models::store::account_config::remove_account_config(a3);
        set_account_friend_blacklist(a1, vec![11, 22]);
        set_account_friend_blacklist(a2, vec![33]);
        assert_eq!(get_account_friend_blacklist(a1), vec![11, 22]);
        assert_eq!(get_account_friend_blacklist(a2), vec![33]);
        assert_eq!(get_account_friend_blacklist(a3), Vec::<i64>::new());
        let _ = crate::models::store::account_config::remove_account_config(a1);
        let _ = crate::models::store::account_config::remove_account_config(a2);
    }

    #[test]
    fn friends_list_cache_ttl_basic() {
        let c = FriendsListCache::new();
        assert_eq!(c.get_ttl_ms(0), 60_000);
        assert_eq!(c.get_ttl_ms(120), 120_000);
        assert_eq!(c.get_ttl_ms(1), 10_000);
    }

    #[test]
    fn is_activity_plant_unknown_returns_false() {
        use crate::proto::generated::gamepb::plantpb::PlantInfo;
        let land = LandInfo {
            id: 1,
            plant: Some(PlantInfo {
                id: 9999999,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(!is_activity_plant(&land));
    }

    #[test]
    fn mark_activity_plant_makes_it_active() {
        use crate::proto::generated::gamepb::plantpb::PlantInfo;
        mark_activity_plant(8888);
        let land = LandInfo {
            id: 1,
            plant: Some(PlantInfo {
                id: 8888,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(is_activity_plant(&land));
    }

    #[test]
    fn plant_phase_from_proto_mature_is_ripe() {
        assert_eq!(PlantPhase::from_i32(6), PlantPhase::Ripe);
        assert_eq!(PlantPhase::from_i32(7), PlantPhase::Dead);
        assert_eq!(PlantPhase::from_i32(3), PlantPhase::Growing);
        assert_eq!(PlantPhase::from_i32(1), PlantPhase::Seed);
    }

    #[test]
    fn parse_max_steal_per_player_varint_and_default() {
        assert_eq!(parse_max_steal_per_player(&[]), 2);
        assert_eq!(parse_max_steal_per_player(&[2]), 2);
        assert_eq!(parse_max_steal_per_player(&[3]), 3);
    }

    #[test]
    fn can_i_still_steal_respects_stealers_and_cap() {
        use crate::proto::generated::gamepb::plantpb::{PlantInfo, StealPlayer};
        use prost::Message;
        let mut plant = PlantInfo {
            stealable: true,
            steal_num: vec![2].into(),
            ..Default::default()
        };
        assert!(can_i_still_steal_plant(&plant, 100));

        let encoded = StealPlayer {
            gid: 100,
            num: 2,
        }
        .encode_to_vec();
        plant.stealers = encoded.into();
        assert!(!can_i_still_steal_plant(&plant, 100));
        assert!(can_i_still_steal_plant(&plant, 200));
    }
}
