use super::*;

#[gpui_kit::test]
fn friend_plant_realtime_event_updates_summary_and_keeps_other_friend_sections(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (_window, desktop, _restore_data_dir) = open_test_desktop(cx);
    cx.update(|cx| {
        desktop.update(cx, |this, cx| {
            this.page = Page::Friends;
            this.page_section = "friends".into();
            this.selected_account = Some("7".into());
            this.preferences.language = "en-US".into();
            this.page_raw = serde_json::json!({
                "friends": [
                    {
                        "gid": 42,
                        "nickname": "好友甲",
                        "plant": {"stealNum": 1, "dryNum": 1, "weedNum": 0, "insectNum": 0}
                    },
                    {
                        "gid": 99,
                        "nickname": "好友乙",
                        "plant": {"stealNum": 2, "dryNum": 0, "weedNum": 0, "insectNum": 0}
                    }
                ],
                "records": [{"gid": 42, "nickname": "访客记录"}],
                "interactionItems": {"items": [{"itemId": 9, "name": "雨水瓶"}]}
            });
            this.rows = crate::pages::page_data(Page::Friends, this.page_raw.clone()).rows;

            let event = qq_farm_app::events::PanelRealtimeEvent {
                event_type: "friend_plant".into(),
                payload: serde_json::json!({
                    "friendGid": "42",
                    "plant": {"stealNum": "3", "dryNum": 2, "weedNum": 1, "insectNum": 1}
                }),
                account_id: Some("7".into()),
            };
            this.handle_friend_realtime_event(&event, cx);

            assert_eq!(
                this.page_raw.pointer("/friends/0/plant"),
                Some(&serde_json::json!({
                    "stealNum": 3, "dryNum": 2, "weedNum": 1, "insectNum": 1
                }))
            );
            assert_eq!(
                this.page_raw.pointer("/records/0/nickname"),
                Some(&serde_json::json!("访客记录"))
            );
            assert_eq!(
                this.page_raw.pointer("/interactionItems/items/0/itemId"),
                Some(&serde_json::json!(9))
            );
            assert_eq!(
                this.rows.iter().find(|row| row.section == "friends").unwrap().state,
                "Steal 3 Water 2 Weed 1 Remove bugs 1"
            );
            let sorted_gids = this
                .rows
                .iter()
                .filter(|row| row.section == "friends")
                .map(|row| row.record["gid"].as_i64().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(sorted_gids, [42, 99]);
        });
    });
}
