use super::super::support::open_test_desktop;
use crate::desktop::Page;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as GpuiContextExt, Focusable as _, TestAppContext};

#[gpui_kit::test]
fn pet_diary_snapshot_failure_hides_activity_sections_and_keeps_retry(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "petDiary".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"activities": []});
            this.error = Some("Pet diary request failed".into());
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("pet-diary-error").visible());
        assert_eq!(desktop.read(cx).error.as_deref(), Some("Pet diary request failed"));
        assert!(window.find("pet-diary-refresh").visible());
        assert!(window.try_find("pet-diary-feed").is_none());
        assert!(window.try_find("pet-diary-draw").is_none());
        window.click("pet-diary-refresh", cx);
        window.render_frame(cx);
        assert!(window.try_find("pet-diary-error").is_none());
        assert!(window.find("pet-diary-refresh").visible());
    })
    .expect("show the snapshot error without stale pet controls and allow retry");

    cx.update(|cx| {
        let this = desktop.read(cx);
        assert!(this.error.is_none());
        assert!(!this.loading);
        assert_eq!(this.page_section, "petDiary");
    });
}

#[gpui_kit::test]
fn pet_diary_uses_one_search_or_gid_picker_and_honors_backend_gates(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Activities;
            this.page_section = "petDiary".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "petDiary": {
                    "active": true,
                    "nurture": {
                    "initialized": true,
                        "canFeed": true,
                        "growth": 8,
                        "adultGrowth": 100,
                        "feedCount": 16,
                        "feedLimit": 16
                    },
                    "hunt": {"canDraw": false, "canPlunder": false, "count": 10, "limit": 10},
                    "charms": {
                        "canRefresh": true,
                        "freeRefreshRemaining": 0,
                        "refreshCost": {"count": 30},
                        "refreshBalance": 75,
                        "pool": [],
                        "equipped": []
                    },
                    "seeds": {"canClaim": false, "days": []},
                    "shop": [],
                    "stories": [],
                    "solarTerms": {"terms": []},
                    "treasures": [],
                    "battleCount": 20,
                    "battleLimit": 20,
                    "skipBattle": false
                },
                "petDiaryFriends": [{"gid": "42", "nickname": "Alice"}],
                "petDiaryFriendGid": "42",
                "petDiaryFriend": {
                    "treasures": [{
                        "id": 77,
                        "status": 2,
                        "item": {"id": 9, "name": "星砂"},
                        "previews": [{"challengeId": "80101", "canStart": true, "plunderableCount": 1}]
                    }],
                    "charms": []
                },
                "petDiaryRecords": []
            });
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_pet_diary_friend_selector(window, cx));
        window.render_frame(cx);
        assert!(window.find("pet-diary-refresh").visible());
        assert!(window.find("pet-diary-friend-gid").visible());
        assert!(window.try_find("pet-diary-friend-search").is_none());
        assert!(window.find("pet-diary-feed").visible());
        assert!(
            !desktop.read(cx).activities.pet_diary_expanded_sections.contains("logs"),
            "the Vue collapse defaults leave interaction logs closed"
        );
        assert!(window.try_find("pet-diary-log-interact").is_none());
        window.click("pet-diary-refresh", cx);
        assert!(!desktop.read(cx).loading);
        assert_eq!(desktop.read(cx).page, Page::Activities);
        assert_eq!(desktop.read(cx).page_section, "petDiary");

        desktop.update(cx, |this, cx| {
            this.activities.pet_diary_expanded_sections =
                ["logs".to_string()].into_iter().collect();
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("pet-diary-log-interact").visible());
        window.click("pet-diary-section-logs", cx);
        assert!(window.try_find("pet-diary-log-interact").is_none());
        desktop.update(cx, |this, cx| {
            this.activities.pet_diary_expanded_sections =
                ["nurture", "hunt", "charm", "battle", "seeds", "shop", "story", "solar"]
                    .into_iter()
                    .map(str::to_string)
                    .collect();
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find("pet-diary-feed").label(), Some("投喂元气糕（16/16）"));
        assert!(window.find("pet-diary-draw").visible());
        desktop.update(cx, |this, cx| {
            this.activities.pet_diary_friend_loading = true;
            this.activities.pet_diary_friends_loading = true;
            cx.notify();
        });
        window.render_frame(cx);
        let (lookup_generation, refresh_generation) = {
            let activities = &desktop.read(cx).activities;
            (
                activities.pet_diary_friend_request_generation,
                activities.pet_diary_friends_request_generation,
            )
        };
        window.click("pet-diary-query-friend", cx);
        window.click("pet-diary-refresh-friends", cx);
        assert_eq!(
            desktop.read(cx).activities.pet_diary_friend_request_generation,
            lookup_generation
        );
        assert_eq!(
            desktop.read(cx).activities.pet_diary_friends_request_generation,
            refresh_generation
        );
        assert!(
            !desktop.read(cx).loading,
            "friend request activity is local to the friend section"
        );
        assert!(window.find("pet-diary-feed").visible());
        desktop.update(cx, |this, cx| {
            this.activities.pet_diary_friend_loading = false;
            this.activities.pet_diary_friends_loading = false;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("pet-diary-section-nurture", cx);
        assert!(window.try_find("pet-diary-feed").is_none());
        desktop.update(cx, |this, cx| {
            this.activities.pending_actions.insert("petDiary:feed".into(), 99);
            cx.notify();
        });
        window.click("pet-diary-section-nurture", cx);
        assert!(window.find("pet-diary-feed").visible());
        assert_eq!(
            desktop.read(cx).activities.pending_actions.get("petDiary:feed"),
            Some(&99),
            "the pet diary remains interactive while an operation is pending"
        );
        desktop.update(cx, |this, cx| {
            this.activities.pending_actions.remove("petDiary:feed");
            cx.notify();
        });

        desktop.update(cx, |this, cx| {
            this.activities.pet_diary_friend_gid = "42".into();
            this.activities.pet_diary_friend_selector.update(cx, |state, cx| {
                state.set_selected_values(&["42".into()], window, cx);
            });
        });
        assert_eq!(desktop.read(cx).activities.pet_diary_friend_gid, "42");
        window.render_frame(cx);
        assert!(window.find("pet-diary-challenge-0-0").visible());
        window.render_frame(cx);

        window.click("pet-diary-refresh-charm", cx);
        window.render_frame(cx);
        let confirm = window.find("ok");
        assert!(confirm.visible(), "paid refresh confirmation was not visible: {confirm:?}");
        assert_eq!(confirm.label(), Some("消耗点券刷新"));
        assert!(window.find("cancel").visible());
        window.press("escape", cx);
        window.render_frame(cx);
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).activities.pet_diary_friend_gid, "42");
        assert_eq!(window.find("pet-diary-feed").label(), Some("Feed vitality cakes (16/16)"));
        assert_eq!(window.find("pet-diary-draw").label(), Some("Treasure hunt (10/10)"));
        assert_eq!(
            window.find("pet-diary-refresh-charm").label(),
            Some("Refresh charm pool (0 free)")
        );
        desktop.update(cx, |this, cx| {
            this.page_raw["petDiaryFriend"] = serde_json::Value::Null;
            this.page_raw["petDiaryFriendGid"] = serde_json::Value::Null;
            this.activities.pet_diary_friend_error = Some("Friend lookup failed".into());
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find("pet-diary-friend-error").label(), Some("Friend lookup failed"));
        assert!(window.try_find("pet-diary-challenge-0-0").is_none());
        assert!(desktop.read(cx).error.is_none(), "lookup errors stay local to the friend section");
    })
    .expect("exercise pet diary controls");
    cx.update_window(window, |_, window, _| assert!(window.try_find("ok").is_none()))
        .expect("Escape closes the paid refresh confirmation");
}

#[gpui_kit::test]
fn pet_diary_friend_selector_closes_with_escape_and_restores_focus(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _cx| {
            this.page = Page::Activities;
            this.page_section = "petDiary".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "petDiary": {"active": true, "hunt": {"canDraw": false}},
                "petDiaryFriends": [{"gid": "42", "nickname": "Alice"}]
            });
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_pet_diary_friend_selector(window, cx));
        window.render_frame(cx);
        let selector_id = desktop.read(cx).activities.pet_diary_friend_selector.entity_id();
        window.click(("multi-combo-box", selector_id), cx);
        window.render_frame(cx);
    })
    .expect("open the searchable pet diary friend selector");

    cx.update_window(window, |_, window, cx| {
        window.press("escape", cx);
        window.render_frame(cx);
        let selector = desktop.read(cx).activities.pet_diary_friend_selector.clone();
        assert!(window.try_find(("combobox-popup", selector.entity_id())).is_none());
        assert!(selector.read(cx).focus_handle(cx).is_focused(window));
    })
    .expect("Escape closes the selector and restores trigger focus");
}

#[gpui_kit::test]
fn pet_diary_log_rows_keep_vue_fields_and_per_tab_cache(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    let time_millis = 1_760_000_000_000_i64;
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Activities;
            this.page_section = "petDiary".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "petDiary": {
                    "active": true,
                    "nurture": {"initialized": false},
                    "hunt": {},
                    "charms": {},
                    "seeds": {"canClaim": false, "days": []},
                    "shop": [],
                    "stories": [],
                    "solarTerms": {"terms": []},
                    "treasures": [],
                    "skipBattle": false
                },
            });
            this.activities.pet_diary_log_kind = "plunder".into();
            this.activities.pet_diary_records.insert(
                "plunder".into(),
                serde_json::json!([{
                    "time": time_millis,
                    "name": "玩家A",
                    "level": 12,
                    "won": true,
                    "challenge": {"id": 1, "name": "挑战书"},
                    "injected": [{"id": 2, "name": "注入道具"}],
                    "lost": [{"id": 3, "name": "星砂", "count": 2}],
                    "fake": true
                }]),
            );
            this.activities.pet_diary_expanded_sections.insert("logs".into());
            this.rows = crate::pages::page_data(Page::Activities, this.page_raw.clone()).rows;
            cx.notify();
        });
    });

    use chrono::TimeZone as _;
    let time = chrono::Local
        .timestamp_millis_opt(time_millis)
        .single()
        .expect("valid test timestamp")
        .format("%m-%d %H:%M")
        .to_string();
    let expected = format!("{time} · 玩家A（Lv.12） · 对方获胜 · 损失 星砂 ×2 · 移花接木假宝藏");

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let row = window.find("pet-diary-plunder-row-0");
        assert_eq!(row.label(), Some(expected.as_str()));
        desktop.update(cx, |this, cx| {
            this.activities.pet_diary_log_kind = "interact".into();
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("pet-diary-plunder-row-0").is_none());
        assert!(window.try_find("pet-diary-interact-row-0").is_none());
        desktop.update(cx, |this, cx| {
            this.activities.pet_diary_records.insert(
                "interact".into(),
                serde_json::json!([{
                    "time": time_millis,
                    "type": 1,
                    "costs": [{"id": 8, "name": "元气糕", "count": 1}],
                    "rewards": [{"id": 3, "name": "星砂", "count": 3}]
                }]),
            );
            cx.notify();
        });
        window.render_frame(cx);
        let interact = window.find("pet-diary-interact-row-0");
        assert_eq!(
            interact.label(),
            Some(format!("{time} · 类型 1 · 消耗 元气糕 ×1 · 获得 星砂 ×3").as_str())
        );
    })
    .expect("log rows keep the Vue field set and per-tab cache");
}
