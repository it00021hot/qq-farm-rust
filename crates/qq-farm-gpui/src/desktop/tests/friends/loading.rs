use super::*;

#[gpui_kit::test]
fn leaving_friends_invalidates_pending_row_and_bulk_operations(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, cx| {
            desktop.page = Page::Friends;
            desktop.page_section = "friends".into();
            desktop.friends.operation_loading.insert((123, "steal"), 1);
            desktop.friends.steal_all_loading = true;
            desktop.friends.blacklist_loading = true;
            desktop.friends.feedback =
                Some(("旧账号的结果".into(), crate::desktop::FriendFeedbackLevel::Info));

            desktop.set_page(Page::Dashboard, cx);

            assert!(desktop.friends.operation_loading.is_empty());
            assert!(!desktop.friends.steal_all_loading);
            assert!(!desktop.friends.blacklist_loading);
            assert!(desktop.friends.feedback.is_none());
            assert_eq!(desktop.page, Page::Dashboard);
        });
    });
}
#[gpui_kit::test]
fn opening_lazy_friend_tabs_refreshes_only_the_selected_section(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, _| {
            desktop.page = Page::Friends;
            desktop.page_section = "friends".into();
        });
    });

    cx.update_window(window, |_, window, cx| {
        let before = desktop.read(cx).request_revision;
        desktop.update(cx, |desktop, cx| {
            desktop.set_page_section("records", window, cx);
        });
        assert_eq!(desktop.read(cx).page_section, "records");
        assert!(desktop.read(cx).request_revision > before);
    })
    .expect("opening visitor history starts a tab-scoped refresh");
}

#[gpui_kit::test]
fn interaction_item_refresh_does_not_issue_a_full_friend_page_refresh(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, cx| {
            desktop.page = Page::Friends;
            desktop.page_section = "interactionItems".into();
            desktop.selected_account = Some("7".into());
            let request_revision = desktop.request_revision;
            let generation = desktop.friends.interaction_items_refresh_generation;
            // A successful use starts a fresh read even if a manual refresh is pending.
            desktop.friends.interaction_items_refresh_loading = true;

            desktop.refresh_friend_interaction_items(cx);

            assert_eq!(desktop.request_revision, request_revision);
            assert_eq!(desktop.friends.interaction_items_refresh_generation, generation + 1);
            assert!(!desktop.loading);
            assert!(!desktop.friends.interaction_items_refresh_loading);
        });
    });
}

#[gpui_kit::test]
fn visitor_service_failure_clears_old_rows_and_shows_a_retryable_error(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, _| {
            desktop.page = Page::Friends;
            desktop.page_section = "records".into();
            desktop.selected_account = Some("7".into());
            desktop.page_raw = serde_json::json!({
                "friends": [{"gid": 1}],
                "records": [],
                "recordsError": "访客服务不可用"
            });
            desktop.rows = crate::pages::page_data(Page::Friends, desktop.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("friend-section-error").visible());
        assert_eq!(window.find("friend-section-error").label(), Some("加载失败: 访客服务不可用"));
        assert!(
            window.try_find("empty-state-label").is_none(),
            "the section banner replaces the visitor empty state"
        );
    })
    .expect("show the visitor tab's own failure state while keeping the friend page available");
}
#[gpui_kit::test]
fn opening_friend_detail_syncs_actionable_land_counts_into_the_list(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, _| {
            desktop.page = Page::Friends;
            desktop.page_raw = serde_json::json!({
                "friends": [
                    {"gid": 123, "nickname": "好友甲", "plant": {"stealNum": 7}},
                    {"gid": 124, "nickname": "好友乙", "plant": {"stealNum": 2}}
                ]
            });
            desktop.rows = crate::pages::page_data(Page::Friends, desktop.page_raw.clone()).rows;
            assert_eq!(desktop.rows[0].record["gid"].as_i64(), Some(123));
            desktop.sync_friend_plant_from_lands(
                123,
                &serde_json::json!({
                    "lands": [
                        {"unlocked": true, "status": "stealable", "needWater": true},
                        {"unlocked": true, "status": "growing", "needWeed": true, "needBug": true},
                        {"unlocked": false, "status": "stealable", "needWater": true}
                    ]
                }),
            );

            assert_eq!(
                desktop.page_raw["friends"][0]["plant"],
                serde_json::json!({
                    "stealNum": 1,
                    "dryNum": 1,
                    "weedNum": 1,
                    "insectNum": 1
                })
            );
            assert_eq!(desktop.rows[0].record["gid"].as_i64(), Some(124));
            assert_eq!(desktop.rows[1].record["gid"].as_i64(), Some(123));
            assert_eq!(desktop.rows[1].state, "偷1 水1 草1 虫1");
        });
    });
}

#[gpui_kit::test]
fn page_load_failure_shows_the_shared_error_banner_instead_of_the_empty_state(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, _| {
            desktop.page = Page::Friends;
            desktop.page_section = "friends".into();
            desktop.selected_account = Some("7".into());
            desktop.page_raw = serde_json::json!({"friends": []});
            desktop.rows = crate::pages::page_data(Page::Friends, desktop.page_raw.clone()).rows;
            desktop.error = Some("加载页面失败：网络请求超时".into());
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("page-error").visible());
        assert_eq!(window.find("page-error").label(), Some("加载页面失败：网络请求超时"));
        assert!(
            window.try_find("empty-state-label").is_none(),
            "the failure banner replaces the generic empty state"
        );
        desktop.update(cx, |desktop, _| desktop.error = None);
        window.render_frame(cx);
        assert!(window.try_find("page-error").is_none());
        assert_eq!(window.find("empty-state-label").label(), Some("无数据"));
    })
    .expect("a failed page load shows the shared banner instead of the empty state");
}

#[gpui_kit::test]
fn friend_detail_failure_shows_a_retryable_error_band_on_its_row(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, _| {
            desktop.page = Page::Friends;
            desktop.page_section = "friends".into();
            desktop.selected_account = Some("7".into());
            desktop.page_raw = serde_json::json!({
                "friends": [
                    {"gid": 123, "nickname": "好友甲"},
                    {"gid": 124, "nickname": "好友乙"}
                ]
            });
            desktop.rows = crate::pages::page_data(Page::Friends, desktop.page_raw.clone()).rows;
            desktop.friends.detail_error = Some((123, "地块数据加载失败".into()));
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("friend-detail-error-123").visible());
        assert_eq!(window.find("friend-detail-error-123").label(), Some("地块数据加载失败"));
        assert_eq!(window.find("friend-detail-retry-123").label(), Some("重试"));
        assert!(window.try_find("friend-detail-error-124").is_none());
    })
    .expect("a failed friend-detail load keeps a retryable error band on its own row");
}
