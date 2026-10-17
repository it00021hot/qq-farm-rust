use super::*;

#[gpui_kit::test]
fn friends_page_requires_an_account_before_showing_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, cx| {
            desktop.page = Page::Friends;
            desktop.page_section = "friends".into();
            desktop.rows = vec![DisplayRow {
                id: "friends/123".into(),
                section: "friends".into(),
                title: "好友甲".into(),
                detail: "Lv12".into(),
                state: "偷1".into(),
                record: serde_json::json!({"gid": 123, "nickname": "好友甲", "level": 12}),
            }];
            desktop.selected_account = None;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("friend-card-123").is_none());
        assert!(window.try_find(("page-section", 0_usize)).is_none());
        assert_eq!(window.find("empty-state-label").label(), Some("请选择账号"));
    })
    .expect("friends page hides rows and sections until an account is selected");
}

#[gpui_kit::test]
fn friend_lands_empty_copy_follows_vue_no_land_data(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, _| {
            desktop.page = Page::Friends;
            desktop.page_section = "friends".into();
            desktop.selected_account = Some("test-account".into());
            desktop.rows = vec![DisplayRow {
                id: "friends/123".into(),
                section: "friends".into(),
                title: "好友甲".into(),
                detail: "Lv12".into(),
                state: "偷1".into(),
                record: serde_json::json!({
                    "gid": 123,
                    "nickname": "好友甲",
                    "level": 12,
                    "gold": 12345,
                    "plant": { "stealNum": 1 }
                }),
            }];
            desktop.friends.detail = Some((123, serde_json::json!({ "lands": [] })));
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("friend-lands-empty").label(), Some("无土地数据"));
    })
    .expect("empty friend farm shows Vue's no-land-data copy");
}

#[gpui_kit::test]
fn friend_card_expands_land_details_inline_and_collapses_on_second_click(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, _| {
            desktop.page = Page::Friends;
            desktop.page_section = "friends".into();
            desktop.selected_account = Some("test-account".into());
            desktop.rows = vec![DisplayRow {
                id: "friends/123".into(),
                section: "friends".into(),
                title: "好友甲".into(),
                detail: "Lv12".into(),
                state: "偷1".into(),
                record: serde_json::json!({
                    "gid": 123,
                    "nickname": "好友甲",
                    "level": 12,
                    "gold": 12345,
                    "plant": { "stealNum": 1 }
                }),
            }];
            desktop.friends.detail = Some((
                123,
                serde_json::json!({
                    "career": { "harvest": 12000, "steal": 3000, "level": 12 },
                    "lands": [
                        {
                            "id": 1,
                            "occupiedLandIds": [1, 2, 5, 6],
                            "plantSize": 2,
                            "status": "growing",
                            "plantName": "草莓",
                            "matureInSec": 300,
                            "totalGrowTime": 900,
                            "level": 1,
                            "currentSeason": 1,
                            "totalSeason": 2
                        },
                        { "id": 2, "occupiedByMaster": true },
                        { "id": 3, "status": "empty" },
                        { "id": 5, "occupiedByMaster": true },
                        { "id": 6, "occupiedByMaster": true },
                        { "id": 7, "status": "empty" }
                    ]
                }),
            ));
            desktop.friends.detail_loaded_at = 100;
            desktop.friends.countdown_now_sec = 100;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("friend-card-123").visible());
        assert!(window.find("friend-avatar-friends/123").visible());
        assert_eq!(window.find("friend-avatar-friends/123").label(), Some("Avatar 好"));
        assert!(window.find("friend-detail-123").visible());
        assert!(window.find("friend-land-123-1").visible());
        assert!(window.try_find("friend-land-123-2").is_none());
        let merged = window.find("friend-land-123-1").bounds();
        let single = window.find("friend-land-123-3").bounds();
        let next_row = window.find("friend-land-123-7").bounds();
        assert!(merged.size.width.as_f32() > single.size.width.as_f32() * 1.5);
        assert!(merged.size.height.as_f32() > single.size.height.as_f32() * 1.5);
        assert!(single.origin.x > merged.origin.x);
        assert!(next_row.origin.y > single.origin.y);
        window.click("friend-card-123", cx);
        assert!(window.try_find("friend-detail-123").is_none());
    })
    .expect("friend details render inline and collapse from their card");
}
#[gpui_kit::test]
fn friend_range_shows_vue_page_bounds_and_raw_total(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "friends": [
                    {"gid": 101, "nickname": "Alice", "gold": 12345, "plant": {"stealNum": 1}},
                    {"gid": 202, "nickname": "Bob", "gold": 0, "plant": {"stealNum": 2}},
                    {"gid": 303, "nickname": "Cindy", "gold": 30}
                ],
                "blacklist": ["303"]
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("friend-card-101").visible());
        assert_eq!(window.find("friends-range").label(), Some("第 1-2 名 / 共 3 名好友"));
        assert_eq!(window.find("friend-gold-202").label(), Some("金币 0"));
    })
    .expect("friend range uses page bounds while the label total follows Vue's raw list length");
}
#[gpui_kit::test]
fn friend_controls_and_actions_follow_the_selected_locale(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.selected_account = Some("7".into());
            let mut friends = (0..26)
                .map(|index| {
                    serde_json::json!({
                        "gid": 100 + index,
                        "nickname": format!("Friend {index}"),
                        "plant": { "stealNum": 1 }
                    })
                })
                .collect::<Vec<_>>();
            friends[0]["gid"] = serde_json::json!(123);
            friends[0]["nickname"] = serde_json::json!("Alice");
            this.page_raw = serde_json::json!({"friends": friends});
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
            cx.notify();
        });
    });

    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find(("page-section", 0_usize)).label(), Some("Friends"));
        assert_eq!(window.find("refresh-friends").label(), Some("Refresh List"));
        assert_eq!(window.find("friends-page-next").label(), Some("Next"));
        assert_eq!(window.find(("friends-page-number", 0_usize)).label(), Some("Page 1"));
        assert_eq!(window.find(("friends-page-number", 1_usize)).label(), Some("Page 2"));
        assert_eq!(window.find("steal-all-friends").label(), Some("Steal all"));
        assert_eq!(window.find(("friend-steal", 0_usize)).label(), Some("One-click steal"));
        assert_eq!(
            desktop.read(cx).friends.interaction_gid.read(cx).presentation().placeholder().as_ref(),
            "Friend GID"
        );
        window.click(("friends-page-number", 1_usize), cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).friends.page, 1);
        assert!(window.find("friend-card-125").visible());
        window.click("friends-page-previous", cx);
        window.render_frame(cx);
        assert_eq!(desktop.read(cx).friends.page, 0);
    })
    .expect("friend controls and row actions localize with the selected language");
}

#[gpui_kit::test]
fn friend_refresh_controls_match_the_active_vue_tab(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.selected_account = Some("7".into());
            this.page_section = "friends".into();
            this.page_raw = serde_json::json!({"friends": [], "blacklist": []});
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("refresh-friends").visible());
        assert!(window.try_find("refresh-friend-visitors").is_none());

        desktop.update(cx, |this, _| this.page_section = "records".into());
        window.render_frame(cx);
        assert!(window.find("refresh-friend-visitors").visible());
        assert!(window.try_find("refresh-friends").is_none());

        desktop.update(cx, |this, _| this.page_section = "interactionItems".into());
        window.render_frame(cx);
        assert!(window.find("refresh-friend-interaction-items").visible());
        assert!(window.try_find("refresh-friends").is_none());

        desktop.update(cx, |this, _| this.page_section = "blacklist".into());
        window.render_frame(cx);
        assert!(window.try_find("refresh-friends").is_none());
        assert!(window.try_find("refresh-friend-visitors").is_none());
        assert!(window.try_find("refresh-friend-interaction-items").is_none());
    })
    .expect("each friend subpage exposes only its matching refresh action");
}

#[gpui_kit::test]
fn friends_blacklist_tab_shows_the_vue_count_badge(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"friends": [], "blacklist": ["303", "404"]});
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("page-section", 2_usize)).label(), Some("好友黑名单, 2"));
        assert_eq!(window.find("friends-blacklist-tab-count").label(), Some("2"));
        desktop.update(cx, |this, cx| {
            this.page_raw["blacklist"] = serde_json::json!(["303"]);
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find("friends-blacklist-tab-count").label(), Some("1"));
        desktop.update(cx, |this, cx| {
            this.page_raw["blacklist"] = serde_json::json!([]);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("friends-blacklist-tab-count").is_none());
        assert_eq!(window.find(("page-section", 2_usize)).label(), Some("好友黑名单"));
    })
    .expect("friends blacklist tab badge follows the Vue count");
}

#[gpui_kit::test]
fn friend_section_empties_follow_vue_copy(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({"friends": [], "blacklist": [], "records": []});
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });
    cx.update_window(window, |_, window, cx| {
        for (section, expected) in
            [("friends", "无数据"), ("blacklist", "暂无黑名单好友"), ("records", "暂无访客记录")]
        {
            desktop.update(cx, |this, cx| this.set_page_section(section, window, cx));
            window.render_frame(cx);
            assert_eq!(
                window.find("empty-state-label").label(),
                Some(expected),
                "empty copy for {section}"
            );
        }
        desktop.update(cx, |this, cx| this.set_language("en-US", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("empty-state-label").label(), Some("No visitor records"));
    })
    .expect("friends sections keep Vue empty copy");
}
