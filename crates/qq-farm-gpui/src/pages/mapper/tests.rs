use super::page_data;
use crate::pages::Page;

#[test]
fn friends_page_keeps_friends_interactions_blacklist_and_history_separate() {
    let data = page_data(
        Page::Friends,
        serde_json::json!({
            "friends": [
                {"gid": 11, "nick": "Alice"},
                {"gid": 22, "nickname": "blocked"}
            ],
            "blacklist": [22],
            "records": [{"gid": 33, "nick": "Bob"}],
            "interactionItems": {"items": [{"itemId": 44, "name": "雨水瓶", "count": 2}]},
        }),
    );

    assert_eq!(data.rows.len(), 4);
    assert!(data.rows[0].id.starts_with("friends/"));
    assert_eq!(data.rows[0].detail, "好友");
    assert_eq!(data.rows[0].title, "Alice");
    assert!(data.rows[1].id.starts_with("blacklist/"));
    assert_eq!(data.rows[1].title, "blocked");
    assert!(data.rows[1].detail.starts_with("黑名单"));
    assert!(data.rows[2].id.starts_with("records/"));
    assert_eq!(data.rows[2].detail, "");
    assert_eq!(data.rows[3].section, "interactionItems");
}

#[test]
fn visitor_rows_keep_vue_identity_and_action_fields_without_preformatted_time() {
    let data = page_data(
        Page::Friends,
        serde_json::json!({
            "friends": [],
            "blacklist": [],
            "records": [{
                "visitorGid": "33",
                "nick": "Bob",
                "level": "12",
                "avatarUrl": "https://example.invalid/avatar.png",
                "actionType": "2",
                "actionDetail": "帮忙除草",
                "serverTimeMs": "1790000000000"
            }]
        }),
    );

    let visitor = data.rows.iter().find(|row| row.section == "records").unwrap();
    assert_eq!(visitor.title, "Bob");
    assert_eq!(visitor.detail, "帮忙除草");
    assert_eq!(visitor.state, "互动");
    assert_eq!(visitor.record["avatarUrl"], "https://example.invalid/avatar.png");
    assert_eq!(visitor.record["serverTimeMs"], "1790000000000");
}

#[test]
fn each_business_page_retains_its_section_identity() {
    let personal = page_data(
        Page::Personal,
        serde_json::json!({
            "lands": {"lands": [{"id": 1, "status": "empty", "unlocked": true}]},
            "bag": {"items": [{"id": 8, "name": "肥料", "count": 2, "sellable": true}]},
            "dailyGifts": [{"id": 3, "name": "每日奖励"}],
            "pets": {"dogs": [{"id": 5, "name": "小狗"}]},
            "illustrated": {"plants": [{"id": 10, "name": "作物图鉴"}]},
            "interactionItems": [{"id": 11, "name": "互动道具"}],
        }),
    );
    for section in ["lands", "bag", "dailyGifts", "pets", "illustrated", "interactionItems"] {
        assert!(
            personal.rows.iter().any(|row| row.section == section),
            "missing personal section {section}"
        );
    }

    let analytics = page_data(
        Page::Analytics,
        serde_json::json!({
            "rankings": [{"seedId": 9, "name": "高收益种子"}],
            "plantBlacklist": [9],
        }),
    );
    assert!(analytics.rows.iter().any(|row| row.section == "rankings"));
    assert!(analytics.rows.iter().any(|row| row.section == "plantBlacklist"));

    let mall = page_data(
        Page::Mall,
        serde_json::json!({
            "slotType": 4, "diamond": 25, "catalog": {"goods": [{"id": 7, "name": "种子包"}]},
        }),
    );
    assert!(mall.rows.iter().any(|row| row.section == "svip" && row.title == "种子包"));
    assert!(mall.rows.iter().any(|row| row.title == "钻石余额"));
}

#[test]
fn settings_snapshot_is_rendered_only_by_grouped_forms() {
    let data = page_data(
        Page::Settings,
        serde_json::json!({
            "systemConfig": {
                "serverUrl": "wss://example.invalid/ws",
                "platform": "qq",
                "deviceInfo": {"deviceId": "test-device"}
            },
            "devicePresets": [{"id": "windows_pc", "name": "Windows PC"}],
            "automation": {"farm": true}
        }),
    );

    assert!(data.rows.is_empty());
}

#[test]
fn standalone_mystery_shop_maps_its_snapshot_and_summary() {
    let data = page_data(
        Page::MysteryShop,
        serde_json::json!({
            "active": true,
            "slotType": -1,
            "npc": {
                "id": 99,
                "stock": 2,
                "reward": {"name": "神秘种子", "count": 1},
                "price": {"name": "金币", "count": 8, "balance": 10}
            }
        }),
    );

    assert_eq!(data.summary, "神秘商店限时商品");
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0].record["id"], 99);
}

#[test]
fn mystery_shop_is_mapped_into_the_mall_tab() {
    let data = page_data(
        Page::Mall,
        serde_json::json!({
            "slotType": -1,
            "active": true,
            "npc": {
                "id": 42,
                "active": true,
                "stock": 1,
                "reward": {"name": "神秘种子", "count": 1},
                "price": {"count": 10, "balance": 20, "name": "金豆"}
            }
        }),
    );

    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0].section, "mystery");
    assert_eq!(data.rows[0].record["id"], 42);
}
