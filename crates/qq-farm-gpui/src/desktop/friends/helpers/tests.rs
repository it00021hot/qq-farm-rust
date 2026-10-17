use super::{
    friend_pet_sync_progress, friend_status_localized, has_friend_action,
    stealable_friends_matching,
};

#[test]
fn friend_status_localizes_available_actions() {
    let friend = serde_json::json!({"plant": {"stealNum": 2, "dryNum": 1, "weedNum": 1}});
    assert_eq!(friend_status_localized(&friend, "zh-CN"), "偷2 水1 草1");
    assert_eq!(friend_status_localized(&friend, "en-US"), "Steal 2 Water 1 Weed 1");
    assert_eq!(friend_status_localized(&serde_json::json!({}), "en-US"), "No action");
}

#[test]
fn friend_status_matches_vue_truthiness_for_raw_action_counts() {
    let friend = serde_json::json!({"plant": {
        "stealNum": "0",
        "dryNum": 0,
        "weedNum": -1,
        "insectNum": null
    }});
    assert_eq!(friend_status_localized(&friend, "zh-CN"), "偷0 草-1");
    assert_eq!(friend_status_localized(&friend, "en-US"), "Steal 0 Weed -1");
    assert!(has_friend_action(&friend));

    let no_actions = serde_json::json!({"plant": {
        "stealNum": 0,
        "dryNum": false,
        "weedNum": "",
        "insectNum": null
    }});
    assert_eq!(friend_status_localized(&no_actions, "zh-CN"), "无操作");
    assert_eq!(friend_status_localized(&no_actions, "en-US"), "No action");
    assert!(!has_friend_action(&no_actions));
}

#[test]
fn pet_sync_progress_excludes_blacklisted_numeric_string_gids() {
    let progress = friend_pet_sync_progress(&serde_json::json!({
        "blacklist": ["42"],
        "friends": [
            {"gid":"42", "petState":"unknown"},
            {"gid":43, "petState":"protect"},
            {"gid":"44", "petState":"unknown"}
        ]
    }));

    assert_eq!(progress, (1, 2));
}

#[test]
fn steal_all_excludes_blacklist_and_keeps_vue_action_priority() {
    let friends = stealable_friends_matching(&serde_json::json!({
        "blacklist": [30],
        "friends": [
            {"gid": 20, "level": 20, "plant": {"stealNum": 2}},
            {"gid": 10, "level": 20, "plant": {"stealNum": 2, "dryNum": 1}},
            {"gid": 30, "level": 99, "plant": {"stealNum": 8}},
            {"gid": 15, "level": 21, "plant": {"stealNum": 2}},
            {"gid": 40, "level": 30, "plant": {"stealNum": 0}},
            {"gid": "5", "level": "30", "plant": {"stealNum": "2"}}
        ]
    }));

    let gids =
        friends.iter().map(|friend| super::friend_number_at(friend, "/gid")).collect::<Vec<_>>();
    assert_eq!(gids, vec![10, 5, 15, 20]);
}

#[test]
fn friend_plant_summary_ignores_locked_lands_and_counts_vue_flags() {
    let summary = super::friend_plant_summary_from_lands(&[
        serde_json::json!({"unlocked": true, "status": "stealable", "needWater": true}),
        serde_json::json!({"unlocked": true, "status": "growing", "needWeed": true, "needBug": true}),
        serde_json::json!({"unlocked": false, "status": "stealable", "needWater": true}),
    ]);
    assert_eq!(
        summary,
        serde_json::json!({
            "stealNum": 1,
            "dryNum": 1,
            "weedNum": 1,
            "insectNum": 1,
        })
    );
}

#[test]
fn optimistic_friend_actions_clear_only_the_operation_fields() {
    let mut friend = serde_json::json!({
        "gid": "12",
        "plant": {"stealNum": 2, "dryNum": 1, "weedNum": 3, "insectNum": 1}
    });
    super::apply_friend_operation_snapshot(&mut friend, 12, "steal");
    assert_eq!(
        friend["plant"],
        serde_json::json!({
            "stealNum": 0, "dryNum": 1, "weedNum": 3, "insectNum": 1
        })
    );
    super::apply_friend_operation_snapshot(&mut friend, 12, "help");
    assert_eq!(
        friend["plant"],
        serde_json::json!({
            "stealNum": 0, "dryNum": 0, "weedNum": 0, "insectNum": 0
        })
    );
}
