use super::*;

#[gpui_kit::test]
async fn friend_destructive_action_cancel_does_not_mutate_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.selected_account = Some("7".into());
            this.preferences.language = "en-US".into();
            this.page_raw = serde_json::json!({
                "friends": [{
                    "gid": 123,
                    "nickname": "Alice",
                    "plant": {"stealNum": 1}
                }],
                "blacklist": []
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("friend-block", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.find("friend-confirm-message").visible());
        assert_eq!(window.find(("friend-confirm-cancel", 0_usize)).label(), Some("Cancel"));
        assert_eq!(window.find(("friend-confirm-confirm", 0_usize)).label(), Some("Confirm"));
        assert!(window.find("friend-confirm-content-0-blacklist").visible());
        assert!(
            !window
                .try_find("friend-confirm-content-0-bad")
                .is_some_and(|popover| popover.visible()),
            "opening one row confirmation must not open its siblings"
        );
        assert!(
            !window
                .try_find("friend-confirm-content-0-delete")
                .is_some_and(|popover| popover.visible()),
            "opening one row confirmation must not open its siblings"
        );
    })
    .expect("show a friend confirmation with cancel and confirm actions");
    wait_for_friend_confirmation_popover(cx, window).await;

    cx.update_window(window, |_, window, cx| {
        window.click(("friend-confirm-cancel", 0_usize), cx);
        assert!(!window.has_active_dialog(cx));
        assert!(!window
            .try_find("friend-confirm-message")
            .is_some_and(|message| message.visible()));
        assert_eq!(desktop.read(cx).page_raw["blacklist"], serde_json::json!([]));
        assert!(!desktop.read(cx).friends.blacklist_loading);
        assert!(desktop.read(cx).friends.delete_loading.is_empty());
    })
    .expect("canceling a destructive friend action leaves business state unchanged");
}
#[gpui_kit::test]
async fn escape_does_not_run_friend_destructive_action(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.selected_account = Some("7".into());
            this.preferences.language = "en-US".into();
            this.page_raw = serde_json::json!({
                "friends": [{
                    "gid": 123,
                    "nickname": "Alice",
                    "plant": {"stealNum": 1}
                }],
                "blacklist": []
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("friend-block", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.find("friend-confirm-message").visible());
        window.press("escape", cx);
    })
    .expect("press Escape while the friend confirmation popover is open");

    cx.update_window(window, |_, window, cx| {
        assert!(!window.has_active_dialog(cx));
        assert_eq!(desktop.read(cx).page_raw["blacklist"], serde_json::json!([]));
        assert!(!desktop.read(cx).friends.blacklist_loading);
    })
    .expect("Escape leaves the friend business state unchanged");
}
#[gpui_kit::test]
async fn blacklist_rows_keep_the_unblock_action(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "blacklist".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "friends": [{"gid": 303, "nickname": "Blocker", "level": 9, "gold": 1200}],
                "blacklist": [303]
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("friend-row-blacklist/0").visible());
        assert_eq!(window.find(("friend-unblock", 0_usize)).label(), Some("移出黑名单"));
        window.click(("friend-unblock", 0_usize), cx);
        window.render_frame(cx);
        assert!(window.find("friend-confirm-message").visible());
    })
    .expect("blacklist rows keep the remove-from-blacklist confirmation");
    wait_for_friend_confirmation_popover(cx, window).await;
    cx.update_window(window, |_, window, cx| {
        window.click(("friend-confirm-cancel", 0_usize), cx);
        assert!(!window.has_active_dialog(cx));
    })
    .expect("cancel the unblock confirmation without mutating state");
}

#[gpui_kit::test]
fn friend_interaction_items_default_to_the_first_item_and_allow_switching(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |desktop, _| {
            desktop.page = Page::Friends;
            desktop.page_section = "interactionItems".into();
            desktop.selected_account = Some("test-account".into());
            desktop.apply_friends_snapshot(serde_json::json!({
                "interactionItems": {
                    "items": [
                        {
                            "itemId": 44,
                            "name": "雨水瓶",
                            "count": 2,
                            "description": "浇灌好友农场",
                            "saleConditionSatisfiedCount": 1
                        },
                        {
                            "itemId": 45,
                            "name": "虫虫胶囊",
                            "count": 1,
                            "description": "对好友地块使用"
                        }
                    ]
                }
            }));
        });
    });
    cx.update_window(window, |_, window, cx| {
        desktop.update(cx, |this, cx| this.sync_friend_interaction_picker(window, cx));
        window.render_frame(cx);
        assert!(window.find("friend-interaction-gid").visible());
        assert!(window.find("friend-interaction-land-ids").visible());
        assert!(window.find("friend-interaction-picker").visible());
        assert!(window.find("friend-interaction-row-0").visible());
        assert_eq!(desktop.read(cx).friends.selected_interaction_item_id, Some(44));
        assert_ne!(window.find("friend-interaction-use").disabled(), Some(true));
        desktop.update(cx, |this, cx| {
            this.friends.interaction_picker.update(cx, |_, cx| {
                cx.emit(gpui_kit::component::select::SelectEvent::Confirm(Some(45)));
            });
        });
        window.render_frame(cx);
        assert!(window.find("friend-interaction-use").visible());
    })
    .expect("friend interaction defaults to the first available item and keeps it switchable");
    cx.update(|cx| {
        // Picker confirmations arrive through the deferred effect flush, like real dropdown picks.
        assert_eq!(desktop.read(cx).friends.selected_interaction_item_id, Some(45));
    });
}
#[gpui_kit::test]
fn successful_friend_operations_reorder_cards_like_vue_sorted_friends(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.page_raw = serde_json::json!({
                "friends": [
                    {"gid": 101, "nickname": "Alice", "plant": {"stealNum": 4}},
                    {"gid": 202, "nickname": "Bob", "plant": {"stealNum": 2}}
                ]
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
            assert_eq!(this.rows[0].record["gid"].as_i64(), Some(101));

            this.update_friend_operation_snapshot(101, "steal");

            assert_eq!(this.page_raw["friends"][0]["plant"]["stealNum"], 0);
            assert_eq!(this.rows[0].record["gid"].as_i64(), Some(202));
            assert_eq!(this.rows[1].record["gid"].as_i64(), Some(101));
        });
    });
}

#[gpui_kit::test]
fn bulk_steal_updates_each_friend_as_soon_as_its_request_finishes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "friends": [
                    {"gid": 101, "nickname": "Alice", "plant": {"stealNum": 4}},
                    {"gid": 202, "nickname": "Bob", "plant": {"stealNum": 2}}
                ]
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
            this.friends.steal_all_generation = 3;
            this.friends.steal_all_loading = true;
            this.friends.steal_all_current_gid = Some(101);

            assert!(this.finish_bulk_steal_target(101, 3, "7", true, cx));
            assert_eq!(this.friends.steal_all_current_gid, None);
            assert_eq!(this.page_raw["friends"][0]["plant"]["stealNum"], 0);
            assert_eq!(this.rows[0].record["gid"].as_i64(), Some(202));

            this.friends.steal_all_current_gid = Some(202);
            assert!(this.finish_bulk_steal_target(202, 3, "7", true, cx));
            assert_eq!(this.page_raw["friends"][1]["plant"]["stealNum"], 0);
            assert!(this.rows.iter().all(|row| {
                row.section != "friends"
                    || row.record.pointer("/plant/stealNum").and_then(serde_json::Value::as_i64)
                        == Some(0)
            }));
        });
    });
}

#[gpui_kit::test]
fn bulk_steal_locks_the_current_friends_row_action(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, _| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.selected_account = Some("7".into());
            this.page_raw = serde_json::json!({
                "friends": [{"gid": 101, "nickname": "Alice", "plant": {"stealNum": 1}}]
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;
            this.friends.steal_all_loading = true;
            this.friends.steal_all_current_gid = Some(101);
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("friend-steal", 0_usize), cx);
        assert!(!desktop.read(cx).friends.operation_loading.contains_key(&(101, "steal")));
        assert_eq!(desktop.read(cx).friends.steal_all_current_gid, Some(101));
    })
    .expect("do not start a duplicate row operation while bulk steal handles this friend");
}
