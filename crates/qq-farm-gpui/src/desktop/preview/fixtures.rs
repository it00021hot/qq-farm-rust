const SEED_IMAGE_ROOT: &str = "/game-config/seed_images_named/seed_images";

pub(super) fn account_snapshot() -> serde_json::Value {
    serde_json::json!({"accounts": [
        {"id":"1", "name":"主账号", "nick":"农场主人", "platform":"qq", "qq":"12345678",
            "running":true, "level":42, "lastOnlineAt":1791208800_i64, "createdAt":1790000000_i64},
        {"id":"2", "name":"日常农场", "platform":"qq", "running":false, "createdAt":1790000000_i64},
        {"id":"3", "name":"长名称账号 · 微信授权", "platform":"wx", "running":false,
            "wxAuthorized":true, "lastOnlineAt":1791194400_i64, "createdAt":1790000000_i64}
    ]})
}

pub(super) fn accounts_empty_snapshot() -> serde_json::Value {
    serde_json::json!({"accounts": []})
}

/// Accounts table with 23 rows so the shared pager renders several pages.
pub(super) fn accounts_many_snapshot() -> serde_json::Value {
    let accounts: Vec<_> = (1..=23)
        .map(|index| {
            let wx = index % 3 == 0;
            let mut account = serde_json::json!({
                "id": index.to_string(),
                "name": format!("农场账号{index:02}"),
                "platform": if wx { "wx" } else { "qq" },
                "running": index % 4 == 1,
                "createdAt": 1_790_000_000_i64,
            });
            if wx {
                account["wxAuthorized"] = serde_json::json!(true);
            } else {
                account["qq"] = serde_json::json!(format!("10000{index:03}"));
            }
            if index % 2 == 0 {
                account["lastOnlineAt"] = serde_json::json!(1_791_200_000_i64 - index * 3_600);
            }
            account
        })
        .collect();
    serde_json::json!({ "accounts": accounts })
}

pub(super) fn farm_snapshot() -> serde_json::Value {
    let lands: Vec<_> = (1..=12)
        .map(|id| {
            let empty = id % 4 == 0;
            let mut land = serde_json::json!({
                "id":id, "status":if empty {"empty"} else {"growing"}, "unlocked":true,
            });
            if !empty {
                land["plantName"] = serde_json::json!("向日葵");
                land["seedImage"] = serde_json::json!(format!("{SEED_IMAGE_ROOT}/1020145.webp"));
                land["matureInSec"] = serde_json::json!(1200);
                land["totalGrowTime"] = serde_json::json!(3600);
                land["level"] = serde_json::json!(2);
                land["currentSeason"] = serde_json::json!(1);
                land["totalSeason"] = serde_json::json!(2);
                land["needWater"] = serde_json::json!(id == 3);
            }
            land
        })
        .collect();
    serde_json::json!({"connected":true, "lands": {
        "lands":lands, "summary":{"harvestable":0, "growing":9, "empty":3, "dead":0},
        "career":{"harvest":12345, "steal":1234}
    }})
}

pub(super) fn bag_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "bag": {
            "items": [
                {
                    "id": 1020001, "uid": 9001, "itemType": 17, "name": "草莓", "count": 8,
                    "level": 3, "price": 12, "priceId": 1, "priceUnit": " 金币",
                    "sellable": true, "sellStatus": "sellable",
                    "mutantEffects": ["多倍", "巨大"],
                    "image": format!("{SEED_IMAGE_ROOT}/1020001.webp")
                },
                {
                    "id": 1020145, "uid": 9002, "itemType": 17, "name": "向日葵", "count": 5,
                    "level": 2, "price": 8, "priceId": 1, "priceUnit": " 金币",
                    "sellable": false, "sellStatus": "conditional",
                    "sellCondition": "收获满 5 次后可出售",
                    "image": format!("{SEED_IMAGE_ROOT}/1020145.webp")
                },
                {
                    "id": 1020033, "uid": 9003, "itemType": 5, "name": "蓝莓种子", "count": 20,
                    "level": 1, "price": 30, "priceId": 1005, "priceUnit": " 金豆",
                    "image": format!("{SEED_IMAGE_ROOT}/1020033.webp")
                },
                {
                    "id": 3001, "uid": 9004, "itemType": 11, "name": "同气连枝礼包", "count": 1
                }
            ],
            "originalItems": [
                {"id": 1020001, "count": 8, "uid": 9001},
                {"id": 1020145, "count": 5, "uid": 9002}
            ]
        }
    })
}

/// Bag where two fruits are sellable, so the batch-selection state has two
/// selectable cards.
pub(super) fn bag_selection_snapshot() -> serde_json::Value {
    let mut snapshot = bag_snapshot();
    snapshot["bag"]["items"][1]["sellable"] = serde_json::json!(true);
    snapshot["bag"]["items"][1]["sellStatus"] = serde_json::json!("sellable");
    snapshot
}

pub(super) fn daily_gifts_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "dailyGifts": {
            "gifts": [
                {"key": "task_claim", "label": "任务奖励", "doneToday": true, "enabled": true,
                    "completedCount": 3, "totalCount": 3, "lastAt": 1791208800000_i64},
                {"key": "email_rewards", "label": "邮件奖励", "doneToday": false, "enabled": true,
                    "completedCount": 1, "totalCount": 2},
                {"key": "mall_free_gifts", "label": "商城免费礼包", "doneToday": false,
                    "enabled": false},
                {"key": "daily_share", "label": "每日分享", "doneToday": true, "enabled": true,
                    "lastAt": 1791208800000_i64},
                {"key": "vip_daily_gift", "label": "QQ 会员每日礼包", "doneToday": false,
                    "enabled": false, "hasGift": false},
                {"key": "month_card_gift", "label": "月卡每日礼包", "doneToday": false,
                    "enabled": true, "hasCard": true, "completedCount": 1, "totalCount": 1}
            ],
            "growth": {
                "completedCount": 1,
                "totalCount": 3,
                "doneToday": false,
                "tasks": [
                    {"id": 1, "desc": "收获 5 次成熟作物", "isCompleted": false,
                        "progress": 2, "totalProgress": 5},
                    {"id": 2, "desc": "与好友互动 3 次", "isCompleted": true,
                        "progress": 3, "totalProgress": 3},
                    {"id": 3, "desc": "登录一次农场", "isCompleted": false}
                ]
            }
        }
    })
}

pub(super) fn illustrated_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "illustrated": {
            "crop": {
                "level": 4,
                "progress": 46,
                "nextLevelProgress": 80,
                "items": [
                    {"seedId": 1020001, "name": "草莓", "rewardCategory": 2, "progress": 46,
                        "unlocked": true, "image": format!("{SEED_IMAGE_ROOT}/1020001.webp"),
                        "reward": {"name": "星砂", "count": 2}},
                    {"seedId": 1020145, "name": "向日葵", "rewardCategory": 3, "progress": 20,
                        "unlocked": true, "image": format!("{SEED_IMAGE_ROOT}/1020145.webp")},
                    {"seedId": 1020033, "name": "蓝莓", "rewardCategory": 4, "progress": 0,
                        "unlocked": false, "image": format!("{SEED_IMAGE_ROOT}/1020033.webp")},
                    {"seedId": 100001, "name": "小麦", "rewardCategory": 1, "progress": 0,
                        "unlocked": false, "image": format!("{SEED_IMAGE_ROOT}/100001.webp")}
                ]
            },
            "mutant": {
                "level": 2,
                "progress": 30,
                "nextLevelProgress": 40,
                "currentBuffs": [
                    {"id": 1, "name": "变异几率", "value": 15, "valueType": "probability"},
                    {"id": 2, "name": "变异收获", "value": 3, "valueType": "count"}
                ],
                "items": [
                    {"seedId": 20001, "name": "金色草莓", "rewardCategory": 4, "progress": 30,
                        "unlocked": true, "image": format!("{SEED_IMAGE_ROOT}/1020001.webp")}
                ]
            }
        }
    })
}

pub(super) fn interaction_items_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "interactionItems": {
            "items": [
                {"itemId": 44, "name": "雨水瓶", "count": 2, "saleConditionSatisfiedCount": 1,
                    "description": "为指定地块降雨，作物立即成长一段进度"},
                {"itemId": 55, "name": "彩虹", "count": 3,
                    "description": "在好友农场生成彩虹，帮助好友作物生长"},
                {"itemId": 61, "name": "同气连枝", "count": 1,
                    "description": "与好友互相照顾，双方的作物同时受益"}
            ],
            "message": ""
        }
    })
}

pub(super) fn pets_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "pets": {
            "activeDogId": 1,
            "protectDuration": 93_600,
            "maxProtectDuration": 864_000,
            "dogs": [
                {"id": 1, "name": "柴犬", "active": true, "owned": true, "rarityLabel": "稀有",
                    "skillDescription": "看家护院，减少好友偷取损失并记录守护日志"},
                {"id": 2, "name": "哈士奇", "active": false, "owned": true, "rarityLabel": "史诗",
                    "skillDescription": "提升守护上限，增加每日守护时长"},
                {"id": 3, "name": "柯基", "active": false, "owned": false, "activatable": true,
                    "rarityLabel": "传说", "skillDescription": "激活后可提升被偷数量的返还比例"},
                {"id": 4, "name": "中华田园犬", "active": false, "owned": false,
                    "activatable": false, "rarityLabel": "普通",
                    "skillDescription": "新手宠物，入门级守护技能"}
            ],
            "foods": [
                {"id": 90004, "name": "普通狗粮", "count": 5, "duration": 86400},
                {"id": 90005, "name": "高级狗粮", "count": 0, "duration": 172800}
            ]
        },
        "petGiftStatus": {"pending": 2}
    })
}

pub(super) fn friends_blacklist_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "friends": [
            {"gid": 101, "nickname": "小麦同学", "level": 18, "gold": 24500,
                "plant": {"stealNum": 2, "dryNum": 1}},
            {"gid": 102, "nickname": "草莓园长", "level": 26, "gold": 12800,
                "plant": {"stealNum": 1, "weedNum": 2}},
            {"gid": 303, "nickname": "黑名单访客", "level": 9, "gold": 1200}
        ],
        "blacklist": [303]
    })
}

/// Blacklist tab with a single blocked friend so the row action index is 0
/// (row action ids derive from the friend's position in the friends array).
pub(super) fn friends_unblacklist_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "friends": [{"gid": 303, "nickname": "黑名单访客", "level": 9, "gold": 1200}],
        "blacklist": [303]
    })
}

pub(super) fn friends_visitors_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "friends": [],
        "records": [
            {"visitorGid": 99, "nick": "偷菜小能手", "level": 18, "actionType": 1,
                "actionLabel": "偷取", "actionDetail": "偷取了 2 份草莓",
                "serverTimeMs": 1_791_208_800_000_i64},
            {"visitorGid": 100, "nick": "热心邻居", "level": 22, "actionType": 2,
                "actionLabel": "帮忙", "actionDetail": "帮忙浇水 3 块地",
                "serverTimeMs": 1_791_200_000_000_i64},
            {"visitorGid": 101, "nick": "调皮的小麦", "level": 12, "actionType": 3,
                "actionLabel": "捣乱", "actionDetail": "在田里放了杂草",
                "serverTimeMs": 1_791_190_000_000_i64}
        ]
    })
}

pub(super) fn friends_interaction_snapshot() -> serde_json::Value {
    serde_json::json!({
        "connected": true,
        "interactionItems": {"items": [
            {"itemId": 44, "name": "雨水瓶", "count": 2, "saleConditionSatisfiedCount": 1,
                "description": "为好友指定地块降雨，作物立即成长一段进度"},
            {"itemId": 55, "name": "彩虹", "count": 3,
                "description": "在好友农场生成彩虹，帮助好友作物生长"},
            {"itemId": 61, "name": "同气连枝", "count": 1,
                "description": "与好友互相照顾，双方作物同时受益"}
        ]}
    })
}

pub(super) fn dashboard_snapshot() -> serde_json::Value {
    serde_json::json!({
        "status": {
            "accountId": "1", "nick": "农场主人", "level": 42, "exp": 1850,
            "gold": 12880, "runStatus": 1, "online": true, "uptime": 7260,
            "levelProgress": {"current": 1850, "needed": 2400},
            "operations": {"harvest": 24, "fertilize": 3, "helpFarming": 12},
            "nextChecks": {"farmRemainSec": 420, "helpRemainSec": 90}
        },
        "logs": [
            {"ts": 1, "time": "10:24:18", "tag": "农场", "msg": "收获了成熟的草莓", "isWarn": false, "meta": {"event": "harvest_crop"}},
            {"ts": 2, "time": "10:22:03", "tag": "好友", "msg": "帮助好友除草", "isWarn": false, "meta": {"event": "friend_help"}},
            {"ts": 3, "time": "10:18:46", "tag": "系统", "msg": "连接已恢复", "isWarn": true, "meta": {"event": "ws_reconnected"}}
        ],
        "bag": {"items": [{"id": 1001, "name": "草莓种子", "count": 32}], "systemItems": []},
        "diamond": 86
    })
}

/// Dashboard with more than the 300-row render cap so the "show earlier
/// logs" affordance appears at the bottom of the log panel.
pub(super) fn dashboard_many_logs_snapshot() -> serde_json::Value {
    let mut snapshot = dashboard_snapshot();
    let tags = ["农场", "好友", "系统", "其他"];
    let events = ["harvest_crop", "friend_help", "ws_reconnected"];
    let logs: Vec<_> = (1..=305)
        .map(|index| {
            let seconds = 36_000 + index * 137;
            serde_json::json!({
                "ts": index,
                "time": format!(
                    "{:02}:{:02}:{:02}",
                    seconds / 3600 % 24,
                    seconds / 60 % 60,
                    seconds % 60
                ),
                "tag": tags[index as usize % 4],
                "msg": format!("运行事件 #{index}"),
                "isWarn": index % 50 == 0,
                "meta": {"event": events[index as usize % 3]},
            })
        })
        .collect();
    snapshot["logs"] = serde_json::json!(logs);
    snapshot
}

/// Dashboard with no log entries yet so the log panel's empty state renders.
pub(super) fn dashboard_empty_logs_snapshot() -> serde_json::Value {
    let mut snapshot = dashboard_snapshot();
    snapshot["logs"] = serde_json::json!([]);
    snapshot
}

pub(super) fn friends_snapshot() -> serde_json::Value {
    serde_json::json!({
        "friends": [
            {"gid": 101, "nickname": "小麦同学", "level": 18, "gold": 24500, "plant": {"stealNum": 2, "dryNum": 1}},
            {"gid": 102, "nickname": "草莓园长", "level": 26, "gold": 12800, "plant": {"stealNum": 1, "weedNum": 2}},
            {"gid": 103, "nickname": "星星农场", "level": 12, "gold": 8900, "plant": {"insectNum": 1}}
        ],
        "blacklist": [],
        "records": [
            {"gid": 201, "nickname": "访客甲", "action": "steal", "time": 1790000000},
            {"gid": 202, "nickname": "访客乙", "action": "help", "time": 1790000300}
        ],
        "interactionItems": []
    })
}

pub(super) fn analytics_snapshot() -> serde_json::Value {
    serde_json::json!({
        "accountLevel": 26,
        "plantBlacklist": [1020077],
        "configuration": {"plantingStrategy": "max_profit", "preferredSeed": 1020001},
        "rankings": [
            {"seedId": 1020001, "name": "草莓", "level": 18, "seasons": 2, "image": format!("{SEED_IMAGE_ROOT}/1020001.webp"), "expPerHour": 42, "profitPerHour": 380, "normalFertilizerExpPerHour": 56, "normalFertilizerProfitPerHour": 510},
            {"seedId": 1020145, "name": "向日葵", "level": 12, "seasons": 1, "image": format!("{SEED_IMAGE_ROOT}/1020145.webp"), "expPerHour": 35, "profitPerHour": 320, "normalFertilizerExpPerHour": 47, "normalFertilizerProfitPerHour": 430},
            {"seedId": 1020077, "name": "蓝莓", "level": 30, "seasons": 2, "image": format!("{SEED_IMAGE_ROOT}/1020077.webp"), "expPerHour": 50, "profitPerHour": 450, "normalFertilizerExpPerHour": 68, "normalFertilizerProfitPerHour": 590}
        ]
    })
}

/// Analytics blacklist tab with nothing blocked so its empty branch renders.
pub(super) fn analytics_blacklist_empty_snapshot() -> serde_json::Value {
    let mut snapshot = analytics_snapshot();
    snapshot["plantBlacklist"] = serde_json::json!([]);
    snapshot
}

pub(super) fn mall_snapshot() -> serde_json::Value {
    serde_json::json!({
        "diamond": 86,
        "catalog": {
            "membership": {"isSvip": true, "remainingDays": 18},
            "refreshCountdown": 420,
            "goods": [
                {"id": 31, "name": "草莓种子礼盒", "rewards": [{"id": 20001, "name": "草莓种子", "count": 20, "image": format!("{SEED_IMAGE_ROOT}/20001.webp")}], "price": {"id": 1, "count": 80, "name": "钻石"}, "originalPrice": 100, "discountText": "8 折", "purchaseStatus": "available", "purchasable": true},
                {"id": 32, "name": "每日成长礼包", "rewards": [{"id": 2001, "name": "经验道具", "count": 1}], "price": {"id": 1, "count": 0, "name": "钻石"}, "purchaseStatus": "sold_out", "purchasable": false, "isFree": true}
            ]
        }
    })
}

/// Normal mall with an empty catalog so the "no goods" branch is covered.
pub(super) fn mall_empty_snapshot() -> serde_json::Value {
    let mut snapshot = mall_snapshot();
    snapshot["catalog"]["goods"] = serde_json::json!([]);
    snapshot
}

pub(super) fn mall_svip_snapshot() -> serde_json::Value {
    serde_json::json!({
        "diamond": 86,
        "catalog": {
            "membership": {"isSvip": true, "remainingDays": 18},
            "refreshCountdown": 600,
            "goods": [
                {"id": 41, "name": "SVIP 专属礼包",
                    "rewards": [{"id": 9001, "name": "高级化肥", "count": 5}],
                    "price": {"id": 1, "count": 120, "name": "钻石"},
                    "originalPrice": 150, "discountText": "8 折",
                    "purchaseStatus": "available", "purchasable": true},
                {"id": 42, "name": "典藏种子箱",
                    "rewards": [{"id": 20001, "name": "典藏种子", "count": 10,
                        "image": format!("{SEED_IMAGE_ROOT}/20001.webp")}],
                    "price": {"id": 1, "count": 60, "name": "钻石"},
                    "purchaseStatus": "svip_required", "purchasable": false},
                {"id": 43, "name": "SVIP 每日免费礼",
                    "rewards": [{"id": 2001, "name": "经验道具", "count": 1}],
                    "price": {"id": 1, "count": 0, "name": "钻石"},
                    "purchaseStatus": "sold_out", "purchasable": false, "isFree": true},
                {"id": 44, "name": "限量外观礼盒",
                    "rewards": [{"id": 9002, "name": "限定头像框", "count": 1}],
                    "price": {"id": 1, "count": 88, "name": "钻石"},
                    "purchaseStatus": "owned", "purchasable": false}
            ]
        }
    })
}

/// Normal mall for a non-SVIP account: the membership chip flips to the
/// muted "非 SVIP 会员" state.
pub(super) fn mall_non_svip_snapshot() -> serde_json::Value {
    let mut snapshot = mall_snapshot();
    snapshot["catalog"]["membership"] = serde_json::json!({"isSvip": false, "remainingDays": 0});
    snapshot
}

/// SVIP tab for a non-member: SVIP-gated goods stay locked while the other
/// goods keep their own per-good status.
pub(super) fn mall_svip_non_member_snapshot() -> serde_json::Value {
    let mut snapshot = mall_svip_snapshot();
    snapshot["catalog"]["membership"] = serde_json::json!({"isSvip": false, "remainingDays": 0});
    snapshot
}

pub(super) fn mystery_shop_snapshot() -> serde_json::Value {
    serde_json::json!({
        "active": true,
        "activeTime": 1_790_000_000_000_i64,
        "expireTime": 1_790_003_600_000_i64,
        "npc": {
            "id": 99, "stock": 8, "discountPercent": 25, "originalPrice": 40,
            "reward": {"id": 20001, "name": "稀有草莓种子", "count": 8, "image": format!("{SEED_IMAGE_ROOT}/20001.webp")},
            "price": {"id": 1, "name": "金币", "count": 30, "balance": 12880}
        }
    })
}

/// Mystery shop with no active merchant so the empty branch is covered.
pub(super) fn mystery_shop_inactive_snapshot() -> serde_json::Value {
    serde_json::json!({
        "active": false,
        "activeTime": 1_790_000_000_000_i64,
        "expireTime": 1_790_003_600_000_i64,
        "npc": serde_json::Value::Null
    })
}

/// Empty personal-page branches: no bag items, no gifts or growth tasks, no
/// pet info, no friends, and an empty game-config catalog.
pub(super) fn bag_empty_snapshot() -> serde_json::Value {
    let mut snapshot = bag_snapshot();
    snapshot["bag"]["items"] = serde_json::json!([]);
    snapshot
}

pub(super) fn daily_gifts_empty_snapshot() -> serde_json::Value {
    let mut snapshot = daily_gifts_snapshot();
    snapshot["dailyGifts"]["gifts"] = serde_json::json!([]);
    snapshot["dailyGifts"]["growth"]["tasks"] = serde_json::json!([]);
    snapshot
}

pub(super) fn pets_empty_snapshot() -> serde_json::Value {
    let mut snapshot = pets_snapshot();
    snapshot["pets"] = serde_json::Value::Null;
    snapshot["petGiftStatus"] = serde_json::json!({"pending": 0});
    snapshot
}

pub(super) fn friends_empty_snapshot() -> serde_json::Value {
    let mut snapshot = friends_snapshot();
    snapshot["friends"] = serde_json::json!([]);
    snapshot["blacklist"] = serde_json::json!([]);
    snapshot["records"] = serde_json::json!([]);
    snapshot
}

pub(super) fn game_config_empty_snapshot() -> serde_json::Value {
    let mut snapshot = game_config_snapshot();
    snapshot["seeds"] = serde_json::json!([]);
    snapshot["fruits"] = serde_json::json!([]);
    snapshot["items"] = serde_json::json!([]);
    snapshot
}

/// Same settings snapshot with the DingTalk reminder channel selected so the
/// provider-specific fields are covered by the preview matrix.
pub(super) fn settings_dingtalk_snapshot() -> serde_json::Value {
    let mut snapshot = settings_snapshot();
    snapshot["offlineReminder"] = serde_json::json!({
        "provider": "ding_talk",
        "endpoint": "https://oapi.dingtalk.com/robot/send?access_token=demo",
        "token": "",
        "secret": "SECdemo"
    });
    snapshot
}

/// Same settings snapshot with a bound QQ bot so the bound reminder state
/// (status tag, nickname, invite/unbind actions) is covered by previews.
pub(super) fn settings_qq_bot_bound_snapshot() -> serde_json::Value {
    let mut snapshot = settings_snapshot();
    snapshot["qqBotBinding"] = serde_json::json!({
        "bound": true,
        "binding": {"nickname": "农场通知助手", "userOpenid": "openid-123"},
        "botInviteUrl": "https://q.qq.com/bot/demo"
    });
    snapshot
}

pub(super) fn settings_snapshot() -> serde_json::Value {
    serde_json::json!({
        "plantingStrategy": "max_profit",
        "preferredSeedId": 1020001,
        "bagSeedFallbackStrategy": "max_exp",
        "bagSeedPriority": [1020001, 1020145],
        "bagSeedMultiLandReservationEnabled": true,
        "intervals": {"farmMin": 30, "farmMax": 60, "helpMin": 90, "helpMax": 180, "stealMin": 60, "stealMax": 120},
        "automation": {
            "farm": true, "land_upgrade": true, "task": true, "sell": false, "friend": true,
            "fertilizer": "smart", "fertilizer_land_types": ["ordinary", "fertile"],
            "fertilizer_smart_seconds": 180, "fertilizer_multi_season": true,
            "friend_auto_accept": true, "autoAcceptFriendMinLevel": 12
        },
        "offlineReminder": {"provider": "qq_bot", "qqBotBinding": {"nickname": "农场通知助手"}},
        "systemConfig": {
            "serverUrl": "wss://gateway.example.com/ws",
            "platform": "qq",
            "os": "Windows",
            "timeZone": "Asia/Shanghai",
            "deviceInfo": {
                "clientVersion": "1.14.2.15",
                "sysSoftware": "Windows 10 x64",
                "deviceId": "device-abc123",
                "memory": "8192",
                "network": "wifi",
                "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"
            }
        }
    })
}

pub(super) fn game_config_snapshot() -> serde_json::Value {
    serde_json::json!({
        "seeds": [
            {"id": 1020001, "seedId": 20001, "name": "草莓", "requiredLevel": 1, "seasons": 1, "growTime": 28800, "harvestCount": 200, "exp": 784, "price": 80, "image": format!("{SEED_IMAGE_ROOT}/1020001.webp"), "rarity": "稀有"},
            {"id": 1020145, "seedId": 20145, "name": "向日葵", "requiredLevel": 1, "seasons": 1, "growTime": 14400, "harvestCount": 200, "exp": 448, "price": 55, "image": format!("{SEED_IMAGE_ROOT}/1020145.webp"), "rarity": "普通"},
            {"id": 1020077, "seedId": 20077, "name": "蓝莓", "requiredLevel": 1, "seasons": 2, "growTime": 28800, "harvestCount": 200, "exp": 1254, "price": 120, "image": format!("{SEED_IMAGE_ROOT}/1020077.webp"), "rarity": "史诗"}
        ],
        "fruits": [
            {"id": 20001, "name": "草莓", "plantName": "草莓", "plantId": 20001, "rarity": 3,
                "price": 12, "priceId": 1, "image": format!("{SEED_IMAGE_ROOT}/1020001.webp")},
            {"id": 20077, "name": "蓝莓", "plantName": "蓝莓", "rarity": 4, "price": 30,
                "priceId": 1005, "image": format!("{SEED_IMAGE_ROOT}/1020077.webp")}
        ],
        "itemTypes": [
            {"value": 7, "label": "化肥"},
            {"value": 9, "label": "礼包"}
        ],
        "items": [
            {"id": 30001, "name": "普通化肥", "type": 7, "rarity": 2, "price": 5, "priceId": 1},
            {"id": 30002, "name": "同气连枝礼包", "type": 9, "rarity": 1, "price": 0, "priceId": 1}
        ]
    })
}

pub(super) fn activity_snapshot() -> serde_json::Value {
    serde_json::json!({
        "activities": [
            {
                "id": "2026100100", "name": "千星游记", "startTime": 1790812800_i64,
                "endTime": 1793404800_i64, "gameplayKey": "stellar", "detailTarget": "travel",
                "gameplayTargets": ["travel", "constellation", "shop", "solar"]
            },
            {
                "id": "2026100200", "name": "秋日祈愿", "startTime": 1790812800_i64,
                "endTime": 1793404800_i64, "gameplayKey": "autumnWish", "detailTarget": "wish"
            },
            {
                "id": "2026110100", "name": "即将开放的活动", "startTime": 1793491200_i64,
                "endTime": 1796083200_i64
            },
            {
                "id": "2026090100", "name": "已结束的活动", "startTime": 1788220800_i64,
                "endTime": 1789862400_i64
            }
        ],
        "season": {
            "title": "千星游记",
            "pass": {
                "level": "5", "progress": 25, "progressMax": 50,
                "nodes": [
                    {"id":"4", "level":"4", "claimed":true, "rewards":[{"name":"星砂", "count":2}]},
                    {"id":"5", "level":"5", "current":true, "claimable":true, "rewards":[{"name":"种子", "count":1}]}
                ]
            }
        },
        "actions": {"claimPass":{"enabled":true}}
    })
}

pub(super) fn pet_diary_snapshot() -> serde_json::Value {
    serde_json::json!({
        "petDiary": {
            "title": "萌宠成长日记",
            "active": true,
            "startTime": 1790812800000_i64,
            "endTime": 1793404800000_i64,
            "balances": [
                {"id": "luckyStar", "name": "幸运星", "count": 128},
                {"id": "coupon", "name": "", "count": null},
                {"id": "cake", "name": "元气糕", "count": 6}
            ],
            "nurture": {
                "initialized": true, "adult": false, "growth": 46, "adultGrowth": 100,
                "feedCount": 3, "feedLimit": 16, "canFeed": true,
                "feedCosts": [{"id": 3001, "name": "元气糕", "count": 1, "image": "/activity-assets/pet-diary/img_exchange_item1.webp"}]
            },
            "hunt": {
                "count": 2, "limit": 10, "total": "5", "luckyStarTotal": "18",
                "costs": [{"id": 3002, "name": "骨头", "count": 2}],
                "canDraw": true, "canPlunder": true
            },
            "charms": {
                "canRefresh": true, "canChoose": false,
                "freeRefreshRemaining": 1, "refreshNote": "每日免费刷新 1 次",
                "paidRefreshCount": 0, "refreshCost": {"count": 30}, "refreshBalance": "75",
                "pool": [
                    {"id": 11, "name": "金钟罩", "shortDescription": "被夺时保护部分宝藏", "image": "/activity-assets/pet-diary/img_exchange_item2.webp"},
                    {"id": 12, "name": "吸星大法", "shortDescription": "夺宝时提升胜率"}
                ],
                "equipped": [{"id": 9, "name": "凌波微步", "remaining": [2, 3]}]
            },
            "treasures": [
                {"id": 71, "status": 2, "item": {"id": 4001, "name": "红包", "count": 1, "image": "/activity-assets/pet-diary/img_exchange_item3.webp"},
                    "protectedCount": "3", "originalCount": "5", "plunderCount": 1, "maxPlunderCount": 2, "endTime": 1791300000000_i64},
                {"id": 72, "status": 3, "item": {"id": 4002, "name": "星砂", "count": 2},
                    "protectedCount": "2", "originalCount": "2", "plunderCount": 0, "maxPlunderCount": 2}
            ],
            "compensationCount": "1",
            "seeds": {"canClaim": true, "days": [
                {"day": 1, "claimed": true, "rewards": [{"id": 1, "name": "种子", "count": 1, "image": format!("{SEED_IMAGE_ROOT}/1020145.webp")}]},
                {"day": 2, "claimable": true, "rewards": [{"id": 2, "name": "种子", "count": 1, "image": format!("{SEED_IMAGE_ROOT}/1020145.webp")}]},
                {"day": 3, "rewards": []}
            ]},
            "shop": [
                {"id": "201", "name": "元气糕", "image": "/activity-assets/pet-diary/img_exchange_item1.webp", "exchangeable": true,
                    "costs": [{"name": "幸运星", "count": 5}], "limit": "3", "purchased": "1", "remaining": "2"},
                {"id": "202", "name": "星砂袋", "exchangeable": false,
                    "costs": [{"name": "幸运星", "count": 20}], "limit": "0", "purchased": "0", "remaining": null}
            ],
            "stories": [
                {"order": 1, "unlocked": true, "claimed": true, "photo": "/activity-assets/pet-diary/img_s3PhotoWall_photo0.webp"},
                {"order": 2, "unlocked": true, "claimed": false, "photo": "/activity-assets/pet-diary/img_s3PhotoWall_photo1.webp"},
                {"order": 3}
            ],
            "solarTerms": {"terms": [
                {"id": 9, "name": "白露", "canClaim": true, "rewards": [{"name": "幸运星", "count": 3}]},
                {"id": 10, "name": "秋分", "claimed": true, "rewards": [{"name": "元气糕", "count": 2}]}
            ]},
            "battleCount": 3, "battleLimit": 20, "skipBattle": false
        },
        "petDiaryRecords": [],
        "petDiaryFriends": [{"gid": "42", "nickname": "Alice"}]
    })
}

pub(super) fn charity_snapshot() -> serde_json::Value {
    serde_json::json!({
        "charity": {
            "groupId": "charity-demo",
            "name": "公益小红花",
            "active": true,
            "love": {"name": "爱心", "image": format!("{SEED_IMAGE_ROOT}/1020145.webp")},
            "loveBalance": "36",
            "donatedLove": "128",
            "settlement": {
                "requiredLove": "200",
                "eligible": false,
                "reward": {"name": "公益礼盒", "image": format!("{SEED_IMAGE_ROOT}/1020145.webp")}
            },
            "globalProgress": {
                "donated": "1250",
                "target": "5000",
                "rewardTarget": "3000",
                "reward": {"name": "全服奖励", "image": format!("{SEED_IMAGE_ROOT}/1020145.webp")}
            },
            "seedReward": {
                "claimed": false,
                "reward": {"name": "小红花种子", "count": 5, "image": format!("{SEED_IMAGE_ROOT}/1020145.webp")}
            },
            "dailyGift": {"claimed": true, "reward": {"name": "每日礼包", "count": 1}},
            "progressRewards": [
                {"target": "100", "reached": true, "claimed": true, "reward": {"name": "星砂", "count": 2}},
                {"target": "300", "reached": true, "claimed": false, "claimable": true, "claimSupported": true, "reward": {"name": "化肥", "count": 3}},
                {"target": "800", "reached": false, "reward": {"name": "种子", "count": 5}}
            ],
            "rules": {
                "title": "公益规则",
                "paragraphs": ["捐赠爱心可获得奖励", "全服达标解锁共同目标奖励"],
                "lines": ["活动结束后按结算资格发放"]
            },
            "actions": {
                "claimSeeds": {"enabled": true},
                "donateLove": {"enabled": true},
                "claimDailyGift": {"enabled": false}
            }
        }
    })
}

pub(super) fn qixi_snapshot() -> serde_json::Value {
    let image = format!("{SEED_IMAGE_ROOT}/1020145.webp");
    serde_json::json!({
        "qixi": {
            "title": "鹊桥寄情",
            "active": true,
            "startTime": 1790812800000_i64,
            "endTime": 1793404800000_i64,
            "feather": {"name": "鹊羽", "image": image},
            "sachet": {"name": "鹊羽香囊", "image": image},
            "receivedSachet": {"image": image},
            "balances": {"known": true, "feather": "80", "sachet": "6", "receivedSachet": "3"},
            "gift": {"sentCount": "12"},
            "bridge": {
                "currentStage": 3,
                "claimable": true,
                "stages": [
                    {"id": 1, "stage": 1, "completed": true, "claimed": true,
                        "cost": {"name": "鹊羽", "count": 20},
                        "rewards": [{"name": "钻石", "count": 5}]},
                    {"id": 2, "stage": 2, "completed": true,
                        "cost": {"name": "鹊羽", "count": 30},
                        "rewards": [{"name": "香囊", "count": 1, "image": image}]},
                    {"id": 3, "stage": 3, "current": true, "claimable": true,
                        "cost": {"name": "鹊羽", "count": 40, "image": image},
                        "rewards": [{"name": "星砂", "count": 3}]}
                ]
            },
            "actions": {"bridge": {"enabled": true}, "gift": {"enabled": true}},
            "rules": {
                "title": "活动规则",
                "paragraphs": ["筑建鹊桥领取阶段奖励", "把鹊羽香囊赠给好友"],
                "lines": ["赠送数量不超过当前香囊余额"]
            }
        },
        "activityFriends": [
            {"gid": 101, "nickname": "小麦", "level": 12},
            {"gid": 102, "nickname": "阿远", "level": 8}
        ]
    })
}

pub(super) fn autumn_snapshot() -> serde_json::Value {
    let image = format!("{SEED_IMAGE_ROOT}/1020145.webp");
    serde_json::json!({
        "autumn": {
            "wish": {
                "title": "秋祈良愿",
                "active": true,
                "startTime": 1790812800000_i64,
                "endTime": 1793404800000_i64,
                "rules": ["每日可祈愿领取奖励", "连续祈愿可获得额外奖励"],
                "day": 3,
                "remaining": 2,
                "canDraw": true,
                "canClaim": true,
                "choices": [
                    {"id": 1, "name": "祈愿丰收"},
                    {"id": 2, "name": "祈愿好运"}
                ],
                "pending": {
                    "chooseId": 1, "textId": 1, "day": 2, "text": "收获满满",
                    "rewards": [{"name": "秋日果实", "count": 2, "image": image}]
                },
                "rewardDays": [
                    {"day": 1, "reward": {"name": "秋日种子", "count": 1, "image": image}},
                    {"day": 2, "reward": {"name": "化肥", "count": 2, "image": image}},
                    {"day": 3, "reward": {"name": "钻石", "count": 5, "image": image}}
                ]
            },
            "happy": {
                "title": "快乐不独享",
                "active": true,
                "startTime": 1790812800000_i64,
                "endTime": 1793404800000_i64,
                "rules": ["分享快乐值给好友", "累积快乐值解锁档位奖励"],
                "score": 45,
                "dailyReward": 5,
                "firstShareReward": 10,
                "claimedCount": 1,
                "claimLimit": 3,
                "poolClaimedCount": 2,
                "poolClaimLimit": 8,
                "canClaimDaily": true,
                "canShare": true,
                "firstShareAwarded": false,
                "canClaimMilestones": true,
                "milestones": [
                    {"id": "20", "threshold": 20, "state": 3, "rewards": [{"name": "星砂", "count": 1, "image": image}]},
                    {"id": "40", "threshold": 40, "state": 2, "rewards": [{"name": "化肥", "count": 2, "image": image}]},
                    {"id": "80", "threshold": 80, "state": 0, "rewards": [{"name": "钻石", "count": 5, "image": image}]}
                ],
                "logs": {"total": 2, "logs": [
                    {"seq": 1, "score": 3, "createdAt": 1790000000, "actor": {"name": "好友A"}},
                    {"seq": 2, "score": -1, "createdAt": 1790000600, "actor": {}}
                ]}
            }
        }
    })
}
