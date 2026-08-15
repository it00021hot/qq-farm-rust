use super::*;
use crate::constants::{ACTIVITY_SERVICE, SEASON_SERVICE, SOLAR_TERMS_SERVICE};
use crate::error::Error;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::{
    ActivityContent, ActivityData, ActivityItem, ActivityOperateReply, ExchangeShopRequest,
    QueryActivityRequest, StarSandGoods, StarSandGoodsList,
};
use crate::proto::generated::gamepb::seasonpb::{
    GetSeasonInfoReply, SeasonActivity, SeasonInfo, SeasonItem, SeasonPass, SeasonRewardNode,
};
use crate::proto::generated::gamepb::solartermspb::{GetSolarTermsReply, SolarTermInfo};
use crate::services::activity_center_state::ConstellationActivityState;
use prost::Message;

    #[test]
    fn service_constants() {
        assert_eq!(SEASON_SERVICE, "gamepb.seasonpb.SeasonService");
        assert_eq!(ACTIVITY_SERVICE, "gamepb.activitypb.ActivityService");
        assert_eq!(SOLAR_TERMS_SERVICE, "gamepb.solartermspb.SolarTermsService");
    }

    #[test]
    fn activity_type_codes() {
        assert_eq!(SHOP_ACTIVITY_TYPE, 3);
        assert_eq!(CONSTELLATION_ACTIVITY_TYPE, 13);
        assert_eq!(EXCHANGE_SHOP_OPERATE_TYPE, 1);
        assert_eq!(QUERY_SHOP_OPERATE_TYPE, 7);
        assert_eq!(LIGHT_CONSTELLATION_OPERATE_TYPE, 21);
    }

    #[test]
    fn positive_decimal_valid() {
        assert_eq!(positive_decimal("123", ActivityErrorCode::InvalidShopGoodsId, "x").unwrap(), 123);
    }

    #[test]
    fn positive_decimal_rejects_zero() {
        assert!(positive_decimal("0", ActivityErrorCode::InvalidShopGoodsId, "x").is_err());
    }

    #[test]
    fn positive_decimal_rejects_negative() {
        assert!(positive_decimal("-5", ActivityErrorCode::InvalidShopGoodsId, "x").is_err());
    }

    #[test]
    fn positive_decimal_rejects_empty() {
        assert!(positive_decimal("", ActivityErrorCode::InvalidShopGoodsId, "x").is_err());
    }

    #[test]
    fn positive_decimal_rejects_non_digit() {
        assert!(positive_decimal("12a", ActivityErrorCode::InvalidShopGoodsId, "x").is_err());
    }

    #[test]
    fn json_positive_decimal_accepts_string_or_number() {
        assert_eq!(
            json_positive_decimal(
                &serde_json::json!("41221001"),
                ActivityErrorCode::InvalidQingmeiUid,
                "uid"
            )
            .unwrap(),
            41_221_001
        );
        assert_eq!(
            json_positive_decimal(
                &serde_json::json!(3),
                ActivityErrorCode::InvalidQingmeiCount,
                "count"
            )
            .unwrap(),
            3
        );
        assert!(json_positive_decimal(
            &serde_json::json!(null),
            ActivityErrorCode::InvalidQingmeiUid,
            "uid"
        )
        .is_err());
    }

    #[test]
    fn qingmei_rules_from_extra_json_object() {
        let extra = br#"{"title":"rules","paragraphs":["first"]}"#;
        let rules = text_content(extra);
        assert_eq!(rules["title"], "rules");
        assert_eq!(rules["paragraphs"][0], "first");
    }

    #[test]
    fn error_codes_have_str() {
        assert_eq!(ActivityErrorCode::ShopUnavailable.as_str(), "SHOP_UNAVAILABLE");
        assert_eq!(ActivityErrorCode::InsufficientStarSand.as_str(), "INSUFFICIENT_STAR_SAND");
        assert_eq!(ActivityErrorCode::InvalidShopGoodsId.as_str(), "INVALID_SHOP_GOODS_ID");
        assert_eq!(ActivityErrorCode::InvalidExchangeCount.as_str(), "INVALID_EXCHANGE_COUNT");
    }

    #[test]
    fn activity_error_display() {
        let e = ActivityError {
            code: ActivityErrorCode::ShopUnavailable,
            message: "test".to_string(),
        };
        assert!(e.to_string().contains("SHOP_UNAVAILABLE"));
        assert!(e.to_string().contains("test"));
    }

    #[test]
    fn bytes_to_text_basic() {
        assert_eq!(bytes_to_text(b"hello"), "hello");
        assert_eq!(bytes_to_text(&[]), "");
    }

    #[test]
    fn bytes_to_text_invalid_utf8_replacement() {
        // 0xFF 0xFE 是非 UTF-8
        let s = bytes_to_text(&[0xFF, 0xFE, b'h', b'i']);
        // lossy 替换：可能是 \u{FFFD}\u{FFFD}hi
        assert!(s.ends_with("hi"));
    }

    #[test]
    fn item_dto_from_core_item() {
        let i = CoreItem {
            id: 1002,
            count: 50,
            ..Default::default()
        };
        let dto = item_dto(&i);
        assert_eq!(dto.id, 1002);
        assert_eq!(dto.count, 50);
    }

    #[test]
    fn normalize_season_basic() {
        let mut reply = GetSeasonInfoReply::default();
        let season = SeasonInfo {
            season_id: 1,
            name: bytes::Bytes::from_static(b"Season 1"),
            status: 1,
            field_4: 0,
            begin_time: 1000,
            end_time: 2000,
            server_time: 1500,
            activities: vec![SeasonActivity {
                activity_id: 10,
                r#type: SHOP_ACTIVITY_TYPE,
                name: bytes::Bytes::from_static(b"Shop"),
                begin_time: 1000,
                end_time: 2000,
            }],
            pass: Some(SeasonPass {
                activity_id: 10,
                current_level: 5,
                current_progress: 100,
                progress_target: 1000,
                node_count: 30,
                claimed_through_level: 3,
                ..Default::default()
            }),
        };
        reply.season_info = Some(season);
        let dto = normalize_season(&reply).unwrap();
        assert_eq!(dto.id, 1);
        assert_eq!(dto.title, "Season 1");
        assert_eq!(dto.server_time, 1500);
        assert_eq!(dto.activities.len(), 1);
        assert!(dto.shop_activity.is_some());
        assert!(dto.constellation_activity.is_none());
        assert!(dto.pass.is_some());
    }

    #[test]
    fn normalize_season_missing() {
        let reply = GetSeasonInfoReply::default();
        assert!(normalize_season(&reply).is_none());
    }

    #[test]
    fn find_season_activity_by_type() {
        let mut reply = GetSeasonInfoReply::default();
        reply.season_info = Some(SeasonInfo {
            activities: vec![
                SeasonActivity { activity_id: 1, r#type: 3, name: bytes::Bytes::new(), begin_time: 0, end_time: 0 },
                SeasonActivity { activity_id: 2, r#type: 13, name: bytes::Bytes::new(), begin_time: 0, end_time: 0 },
            ],
            ..Default::default()
        });
        let shop = find_season_activity(&reply, SHOP_ACTIVITY_TYPE).unwrap();
        assert_eq!(shop.activity_id, 1);
        let constellation = find_season_activity(&reply, CONSTELLATION_ACTIVITY_TYPE).unwrap();
        assert_eq!(constellation.activity_id, 2);
        let missing = find_season_activity(&reply, 99);
        assert!(missing.is_none());
    }

    #[test]
    fn normalize_solar_terms_basic() {
        let mut reply = GetSolarTermsReply::default();
        reply.server_time = 1500;
        reply.terms = vec![SolarTermInfo {
            term_id: 100,
            status: 2,
            begin_time: 1000,
            end_time: 2000,
            name: bytes::Bytes::from("立春".as_bytes()),
            rewards: vec![],
        }];
        let dto = normalize_solar_terms(&reply);
        assert_eq!(dto.terms.len(), 1);
        assert_eq!(dto.terms[0].name, "立春");
        assert_eq!(dto.current_term_id, Some(100));
    }

    #[test]
    fn normalize_solar_terms_no_current() {
        let reply = GetSolarTermsReply::default();
        let dto = normalize_solar_terms(&reply);
        assert!(dto.terms.is_empty());
        assert_eq!(dto.current_term_id, None);
    }

    #[test]
    fn constellation_day_basic() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        // 2024-01-02 00:00:00 UTC = 1704153600
        let start = 1_704_067_200_i64;
        let server = 1_704_153_600_i64;
        // UTC+8 后：start = 2024-01-01 08:00:00 (day 19883)
        //           server = 2024-01-02 08:00:00 (day 19884)
        // 差 = 19884 - 19883 = 1，再 +1 = 2（第 2 天）
        let day = constellation_day_from_beijing_midnight(start, server).unwrap();
        assert_eq!(day, 2);
    }

    #[test]
    fn constellation_day_same_day() {
        let start = 1_704_067_200_i64;
        let server = 1_704_067_200_i64;
        let day = constellation_day_from_beijing_midnight(start, server).unwrap();
        assert_eq!(day, 1);
    }

    #[test]
    fn constellation_day_zero_inputs() {
        assert!(constellation_day_from_beijing_midnight(0, 1000).is_none());
        assert!(constellation_day_from_beijing_midnight(1000, 0).is_none());
    }

    #[test]
    fn constellation_day_before_start() {
        // server 在 start 之前（北京时间）
        // 2023-12-31 16:00:00 UTC = 1704038400 (北京时间 2024-01-01 00:00:00)
        // start = 2024-01-01 08:00:00 UTC
        // server = 1704038400 在 start 之前（北京时间相差 8 小时）
        // 但用"日历天"算：start_day = 2024-01-01 (day 19883)
        //                  server_day = 2024-01-01 (day 19883)
        // 差 = 0 + 1 = 1
        // 实际上 "before start" 是不会发生因为 server 在 start 当天或之后才有效
        let start = 1_704_067_200_i64;
        let server = 1_704_038_400_i64; // 2023-12-31 16:00:00 UTC = 2024-01-01 00:00:00 BJ
        let day = constellation_day_from_beijing_midnight(start, server).unwrap();
        // 实际是 1（因为日历天差 0 + 1 = 1）
        assert_eq!(day, 1);
    }

    #[test]
    fn star_sand_goods_dto_with_balance() {
        let goods = StarSandGoods {
            goods_id: 100,
            cost: Some(ActivityItem { item_id: 1, count: 10 }),
            item: Some(ActivityItem { item_id: 2, count: 1 }),
            status: 0,
            owned: false,
            sort_order: 1,
            name: bytes::Bytes::from_static(b"Star"),
            category: bytes::Bytes::from_static(b"tools"),
            ..Default::default()
        };
        let mut balances = std::collections::HashMap::new();
        balances.insert(1, 50);
        let dto = star_sand_goods_dto(&goods, 999, Some(&balances));
        assert_eq!(dto.id, 100);
        assert_eq!(dto.activity_id, 999);
        assert_eq!(dto.cost.id, 1);
        assert_eq!(dto.cost.count, 10);
        assert!(dto.exchangeable);
        assert!(!dto.sold_out);
        assert_eq!(dto.max_exchange_count, 5);
        assert!(dto.max_exchange_count_known);
    }

    #[test]
    fn star_sand_goods_dto_no_balance() {
        let goods = StarSandGoods {
            goods_id: 100,
            cost: Some(ActivityItem { item_id: 1, count: 10 }),
            ..Default::default()
        };
        let dto = star_sand_goods_dto(&goods, 999, None);
        assert!(!dto.max_exchange_count_known);
        assert_eq!(dto.max_exchange_count, 0);
    }

    #[test]
    fn star_sand_goods_dto_invalid_cost() {
        let goods = StarSandGoods {
            goods_id: 100,
            cost: Some(ActivityItem { item_id: 0, count: 0 }),
            ..Default::default()
        };
        let dto = star_sand_goods_dto(&goods, 999, None);
        assert!(!dto.exchangeable);
    }

    #[test]
    fn activity_dto_basic() {
        let a = SeasonActivity {
            activity_id: 42,
            r#type: 3,
            name: bytes::Bytes::from("商店".as_bytes()),
            begin_time: 1000,
            end_time: 2000,
        };
        let dto = activity_dto(&a);
        assert_eq!(dto.id, 42);
        assert_eq!(dto.r#type, 3);
        assert_eq!(dto.name, "商店");
    }

    #[test]
    fn pass_dto_basic() {
        let p = SeasonPass {
            activity_id: 10,
            current_level: 5,
            current_progress: 100,
            progress_target: 1000,
            node_count: 30,
            claimed_through_level: 3,
            nodes: vec![SeasonRewardNode {
                node_id: 5,
                is_key_level: true,
                rewards: vec![SeasonItem {
                    item_id: 1,
                    count: 10,
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let dto = pass_dto(&p);
        assert_eq!(dto.activity_id, 10);
        assert_eq!(dto.current_level, 5);
        assert_eq!(dto.level, 5);
        assert_eq!(dto.claimed_through_level, 3);
        assert_eq!(dto.nodes.len(), 1);
        assert!(dto.nodes[0].claimable);
        assert_eq!(
            dto.nodes[0].rewards[0].image,
            "/game-config/seed_images_named/1.png"
        );
        let v = serde_json::to_value(&dto).unwrap();
        assert!(v.get("nodes").and_then(|n| n.as_array()).is_some());
        assert_eq!(v["nodes"][0]["rewards"][0]["image"], "/game-config/seed_images_named/1.png");
    }

    #[test]
    fn season_dto_default() {
        let dto = SeasonDto::default();
        assert_eq!(dto.id, 0);
        assert!(dto.activities.is_empty());
        assert!(dto.pass.is_none());
    }

    #[test]
    fn solar_term_dto_default() {
        let dto = SolarTermDto::default();
        assert_eq!(dto.id, 0);
    }

    #[test]
    fn star_sand_shop_dto_default() {
        let dto = StarSandShopDto::default();
        assert!(!dto.balance_known);
        assert_eq!(dto.affordable_count, 0);
    }

    #[test]
    fn light_constellation_result_lighted() {
        let r = serde_json::json!({
            "outcome": "lighted",
            "rewards": [],
            "constellation": null,
        });
        assert_eq!(r["outcome"], "lighted");
    }

    #[test]
    fn light_constellation_result_nothing() {
        let r = serde_json::json!({
            "outcome": "nothingToClaim",
            "noClaimable": true,
            "message": "已领",
        });
        assert_eq!(r["outcome"], "nothingToClaim");
        assert_eq!(r["message"], "已领");
    }

    #[test]
    fn activity_error_into_core_error() {
        let e = ActivityError {
            code: ActivityErrorCode::ShopUnavailable,
            message: "x".to_string(),
        };
        let core: Error = e.into();
        assert!(core.to_string().contains("SHOP_UNAVAILABLE"));
    }

    #[test]
    fn constellation_dto_default() {
        let dto = ConstellationDto::default();
        assert_eq!(dto.activity_id, 0);
        assert_eq!(dto.current_day, None);
    }

    #[test]
    fn constellation_catalog_has_groups() {
        let groups = constellation_catalog_groups();
        assert!(groups.as_array().map(|a| !a.is_empty()).unwrap_or(false));
        assert!(constellation_catalog_version() >= 1);
    }

    #[test]
    fn constellation_schedule_marks_future_locked() {
        let act = SeasonActivityDto {
            id: 2_026_072_701,
            r#type: 13,
            name: "千星同明".to_string(),
            begin_time: 1_753_574_400,
            start_time: 1_753_574_400,
            end_time: 1_756_166_400,
        };
        let confirmed = ConstellationActivityState::default();
        let dto = constellation_dto(&act, 1_753_574_400 + 86_400 * 2, None, &confirmed);
        assert_eq!(dto.catalog_status, "supported");
        assert_eq!(dto.current_day, Some(3));
        let current = dto.groups.iter().find(|g| g.order == 3).expect("day 3");
        assert_eq!(current.visual_state, "claimableUnknown");
        let future = dto.groups.iter().find(|g| g.order == 10).expect("day 10");
        assert_eq!(future.visual_state, "locked");
        let past = dto.groups.iter().find(|g| g.order == 1).expect("day 1");
        assert_eq!(past.visual_state, "unknown");
    }

    #[test]
    fn qingmei_constants() {
        assert_eq!(QINGMEI_DAILY_ACTIVITY_ID, 2026081201);
        assert_eq!(QINGMEI_BREW_ACTIVITY_ID, 2026081202);
        assert_eq!(QINGMEI_ITEM_ID, 41221);
    }

    #[test]
    fn qingmei_already_claimed_message_detects_code_and_text() {
        assert!(is_qingmei_already_claimed_message(
            "gateway error: x.y code=1034014 already"
        ));
        assert!(is_qingmei_already_claimed_message(
            "今日青梅种子已经领取，无需重复领取"
        ));
        assert!(!is_qingmei_already_claimed_message("timeout"));
    }

    #[test]
    fn force_qingmei_seed_claimed_patches_snapshot() {
        let mut snap = Some(serde_json::json!({
            "qingMei": {
                "dailySeed": { "claimed": false, "grantId": "3" },
                "actions": { "claimSeed": { "enabled": true, "available": true } }
            }
        }));
        force_qingmei_seed_claimed_in_snapshot(&mut snap);
        let qm = &snap.unwrap()["qingMei"];
        assert_eq!(qm["dailySeed"]["claimed"], true);
        assert_eq!(qm["actions"]["claimSeed"]["enabled"], false);
    }

    #[test]
    fn qingmei_seed_claimed_persist_roundtrip() {
        let acc = format!("test-qm-{}", std::process::id());
        let today = beijing_date_key();
        assert!(load_qingmei_seed_claimed_date(&acc).is_none());
        persist_qingmei_seed_claimed_date(&acc, &today).expect("persist");
        assert_eq!(load_qingmei_seed_claimed_date(&acc).as_deref(), Some(today.as_str()));
        let _ = std::fs::remove_file(qingmei_seed_claimed_path(&acc));
    }

    #[test]
    fn encode_query_activity_request() {
        let req = QueryActivityRequest {
            activity_id: 100,
            operate_type: QUERY_SHOP_OPERATE_TYPE,
        };
        let bytes = req.encode_to_vec();
        let back = QueryActivityRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.activity_id, 100);
        assert_eq!(back.operate_type, 7);
    }

    #[test]
    fn encode_exchange_shop_request() {
        let req = ExchangeShopRequest {
            activity_id: 100,
            operate_type: EXCHANGE_SHOP_OPERATE_TYPE,
            exchange_shop_operate: Some(
                crate::proto::generated::gamepb::activitypb::ExchangeShopOperateParams {
                    goods_id: 50,
                    count: 3,
                },
            ),
        };
        let bytes = req.encode_to_vec();
        let back = ExchangeShopRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.exchange_shop_operate.as_ref().unwrap().goods_id, 50);
    }

    #[test]
    fn activity_data_with_empty_catalog() {
        // proto 必须能解空 catalog
        let data = ActivityData {
            activity: Some(ActivityContent {
                activity_id: 1,
                group_id: 0,
                r#type: 0,
                name: "test".to_string(),
                extra: bytes::Bytes::new(),
                begin_time: 0,
                end_time: 0,
                sort_order: 0,
                field_20: 0,
                field_23: 0,
            }),
            catalog: Some(StarSandGoodsList { goods: vec![] }),
            ..Default::default()
        };
    let reply = ActivityOperateReply {
        activity_id: 1,
        operate_type: 7,
        data: Some(data),
        ..Default::default()
    };
    let raw = extract_goods(&reply);
    assert!(raw.is_empty());
}
