use super::{entries, period, sort_records, status, timestamp_ms};

#[test]
fn activity_timestamps_follow_vue_number_and_seconds_conversion() {
    assert_eq!(timestamp_ms(&serde_json::json!({"startTime": 1.5}), "startTime"), 1_500);
    assert_eq!(timestamp_ms(&serde_json::json!({"startTime": "1.5"}), "startTime"), 1_500);
    assert_eq!(timestamp_ms(&serde_json::json!({"startTime": -1.5}), "startTime"), -1_500);
    assert_eq!(
        timestamp_ms(&serde_json::json!({"startTime": 1_700_000_000_123_i64}), "startTime"),
        1_700_000_000_123
    );
    assert_eq!(timestamp_ms(&serde_json::json!({"startTime": " "}), "startTime"), 0);
}

#[test]
fn directory_falls_back_to_snapshot_games_and_formats_time_window() {
    let raw = serde_json::json!({
        "season": { "title": "千星游记", "activityId": "11", "startTime": 1_800_000_000 },
        "qixi": { "title": "七夕鹊桥", "groupId": "12" }
    });
    let records = entries(&raw, "zh-CN");
    assert_eq!(records.len(), 3);
    assert_eq!(records[0]["detailTarget"], "travel");
    assert_eq!(records[1]["detailTarget"], "qixi");
    assert_eq!(period(&records[0], "zh-CN"), "2027/01/15 未开始");
    assert_eq!(status(&records[0], 1_790_000_000_000).0, "未开始");
}

#[test]
fn stellar_fallback_uses_vue_availability_and_defaults_to_the_travel_tab() {
    let records = entries(
        &serde_json::json!({
            "solarTerms": { "terms": [{ "id": "1", "name": "立春" }] }
        }),
        "zh-CN",
    );
    let stellar = records
        .iter()
        .find(|record| {
            record.get("gameplayKey").and_then(serde_json::Value::as_str) == Some("stellar")
        })
        .expect("solar terms make the stellar gameplay available");
    assert_eq!(stellar["detailTarget"], "travel");
    assert_eq!(stellar["name"], "千星游记");
    assert_eq!(stellar["id"], "stellar");

    let records = entries(
        &serde_json::json!({
            "solarTerms": { "title": "节气标题", "terms": [] }
        }),
        "zh-CN",
    );
    assert!(records.iter().all(|record| {
        record.get("gameplayKey").and_then(serde_json::Value::as_str) != Some("stellar")
    }));
}

#[test]
fn activity_directory_fallbacks_and_time_labels_follow_the_selected_language() {
    let records = entries(
        &serde_json::json!({
            "solarTerms": { "terms": [{ "id": "1", "name": "立春" }] }
        }),
        "en-US",
    );
    assert_eq!(records[0]["name"], "Travel Pass");
    assert_eq!(records.last().unwrap()["name"], "Rainfall Poems");
    assert_eq!(
        period(&serde_json::json!({"startTime": 1_800_000_000}), "en-US"),
        "2027/01/15 Upcoming"
    );
    assert_eq!(period(&serde_json::json!({}), "en-US"), "Schedule TBD");
}

#[test]
fn activity_fallbacks_follow_vue_known_and_active_gates() {
    let records = entries(
        &serde_json::json!({
            "qixi": { "title": "标题但无活动标识" },
            "greenPlum": { "known": false, "active": true, "name": "未知青梅" },
            "charity": { "title": "标题但无名称或活动 ID" }
        }),
        "zh-CN",
    );
    assert!(records.iter().all(|record| {
        !matches!(
            record.get("gameplayKey").and_then(serde_json::Value::as_str),
            Some("qixi" | "greenPlum" | "charity")
        )
    }));

    let records = entries(
        &serde_json::json!({
            "greenPlum": { "known": true, "active": true, "name": "青梅酿造" }
        }),
        "zh-CN",
    );
    assert!(records.iter().any(|record| {
        record.get("gameplayKey").and_then(serde_json::Value::as_str) == Some("greenPlum")
    }));

    let records = entries(
        &serde_json::json!({
            "activities": [{ "gameplayKey": "future-mode", "detailTarget": "greenPlum" }],
            "greenPlum": { "known": true, "active": true, "name": "青梅酿造" }
        }),
        "zh-CN",
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| {
                record.get("gameplayKey").and_then(serde_json::Value::as_str) == Some("greenPlum")
            })
            .count(),
        0
    );
}

#[test]
fn activity_fallback_cards_keep_vue_name_and_id_precedence() {
    let records = entries(
        &serde_json::json!({
            "qixi": {
                "groupId": "qixi-group",
                "name": "Name wins",
                "title": "Title loses"
            },
            "greenPlum": {
                "known": true,
                "active": true,
                "activityId": 0,
                "brewActivityId": "brew-42",
                "title": "Vue ignores this title"
            },
            "charity": {
                "groupId": "charity-group",
                "activityId": "charity-42",
                "name": "Name wins",
                "title": "Title loses"
            }
        }),
        "zh-CN",
    );

    let qixi = records.iter().find(|record| record["gameplayKey"] == "qixi").unwrap();
    assert_eq!(qixi["id"], "qixi-group");
    assert_eq!(qixi["name"], "Name wins");

    let green_plum = records.iter().find(|record| record["gameplayKey"] == "greenPlum").unwrap();
    assert_eq!(green_plum["id"], "brew-42");
    assert_eq!(green_plum["name"], "青梅");

    let charity = records.iter().find(|record| record["gameplayKey"] == "charity").unwrap();
    assert_eq!(charity["id"], "charity-group");
    assert_eq!(charity["name"], "Name wins");

    let records = entries(&serde_json::json!({"qixi": {"groupId": "qixi-group"}}), "zh-CN");
    let qixi = records.iter().find(|record| record["gameplayKey"] == "qixi").unwrap();
    assert_eq!(qixi["name"], "鹊桥寄情");

    let records = entries(
        &serde_json::json!({
            "greenPlum": {
                "known": true,
                "active": true,
                "name": "青梅活动",
                "title": "备用标题"
            }
        }),
        "en-US",
    );
    let green_plum = records.iter().find(|record| record["gameplayKey"] == "greenPlum").unwrap();
    assert_eq!(green_plum["name"], "青梅活动");

    let weather = records.iter().find(|record| record["gameplayKey"] == "weather").unwrap();
    assert_eq!(weather["id"], "2026070300");
    assert_eq!(weather["name"], "Rainfall Poems");
}

#[test]
fn directory_sorts_by_status_then_start_time_like_vue() {
    let now = 2_000_000_000_000;
    let mut records = vec![
        serde_json::json!({"id":"upcoming-late", "startTime":2_100_000_000_000_i64}),
        serde_json::json!({"id":"active-late", "startTime":1_950_000_000_000_i64}),
        serde_json::json!({"id":"ended", "startTime":1_000_000_000_000_i64, "endTime":1_900_000_000_000_i64}),
        serde_json::json!({"id":"active-early", "startTime":1_900_000_000_000_i64}),
        serde_json::json!({"id":"upcoming-early", "startTime":2_050_000_000_000_i64}),
    ];

    sort_records(&mut records, now);

    assert_eq!(
        records.iter().map(|record| record["id"].as_str().unwrap()).collect::<Vec<_>>(),
        ["active-early", "active-late", "upcoming-early", "upcoming-late", "ended"]
    );
}

#[test]
fn activity_status_changes_at_start_and_end_boundaries() {
    let record =
        serde_json::json!({"startTime": 2_000_000_000_000_i64, "endTime": 2_000_000_010_000_i64});
    assert_eq!(status(&record, 1_999_999_999_999).0, "未开始");
    assert_eq!(status(&record, 2_000_000_000_000).0, "进行中");
    assert_eq!(status(&record, 2_000_000_010_000).0, "已结束");
}
