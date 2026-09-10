//! 活动类型、操作码、道具 / 容器 ID。

/// 活动类型（对齐 SeasonActivity.type）
pub const SHOP_ACTIVITY_TYPE: i64 = 3;
pub const CONSTELLATION_ACTIVITY_TYPE: i64 = 13;

/// 活动操作类型
pub const EXCHANGE_SHOP_OPERATE_TYPE: i64 = 1;
pub const QUERY_SHOP_OPERATE_TYPE: i64 = 7;
pub const LIGHT_CONSTELLATION_OPERATE_TYPE: i64 = 21;

/// 鹊桥寄情
pub const QIXI_GROUP_ID: i64 = 2_026_081_800;
pub const QIXI_BRIDGE_ACTIVITY_ID: i64 = 2_026_081_801;
pub const QIXI_GIFT_ACTIVITY_ID: i64 = 2_026_081_802;
pub const QIXI_BRIDGE_OPERATE_TYPE: i64 = 25;
pub const QIXI_GIFT_OPERATE_TYPE: i64 = 26;
pub const QIXI_FEATHER_ITEM_ID: i64 = 1024;
pub const QIXI_SACHET_ITEM_ID: i64 = 1025;
pub const QIXI_RECEIVED_SACHET_ITEM_ID: i64 = 1026;
/// 七夕灵露（好友互动道具，可对好友/自己农场使用）
pub const QIXI_DEW_ITEM_ID: i64 = 301_103;
/// 同气连枝礼包（狗狗技能掉落）
pub const DOG_SKILL_GIFT_ITEM_ID: i64 = 101_351;

/// 青梅酿酒
pub const QINGMEI_DAILY_ACTIVITY_ID: i64 = 2_026_081_201;
pub const QINGMEI_BREW_ACTIVITY_ID: i64 = 2_026_081_202;
pub const QINGMEI_ITEM_ID: i64 = 41221;
pub const QINGMEI_DAILY_GRANT_ID: i64 = 3;
pub const QUERY_QINGMEI_OPERATE_TYPE: i64 = 7;
pub const CLAIM_QINGMEI_SEED_OPERATE_TYPE: i64 = 4;
pub const START_QINGMEI_BREW_OPERATE_TYPE: i64 = 14;
pub const CONTINUE_QINGMEI_BREW_OPERATE_TYPE: i64 = 15;
pub const SELL_QINGMEI_BREW_OPERATE_TYPE: i64 = 16;
pub const QINGMEI_SHARED_SETTLEMENT_MODE: i64 = 2;
pub const QINGMEI_SHARE_SOURCE: i32 = 11;
pub const QINGMEI_SHARE_SCENE: i32 = 215;
pub const QINGMEI_DAILY_ALREADY_CLAIMED_CODE: i64 = 1_034_014;

/// 公益小红花
pub const CHARITY_RED_FLOWER_GROUP_ID: i64 = 2_026_090_900;
pub const CHARITY_RED_FLOWER_ACTIVITY_ID: i64 = 2_026_090_901;
pub const CLAIM_CHARITY_SEED_OPERATE_TYPE: i64 = 35;
pub const DONATE_CHARITY_LOVE_OPERATE_TYPE: i64 = 36;
pub const CLAIM_CHARITY_PROGRESS_REWARD_OPERATE_TYPE: i64 = 37;
pub const CLAIM_CHARITY_DAILY_GIFT_OPERATE_TYPE: i64 = 38;
pub const CHARITY_PROGRESS_ALREADY_CLAIMED_CODE: i64 = 1_034_087;

/// 萌宠成长日记（活动组 + 养成 / 种子赠礼 / 拾物小铺三个子活动）
pub const PET_DIARY_GROUP_ID: i64 = 2_026_090_100;
pub const PET_DIARY_ACTIVITY_ID: i64 = 2_026_090_101;
pub const PET_DIARY_SEEDS_ID: i64 = 2_026_090_102;
pub const PET_DIARY_SHOP_ID: i64 = 2_026_090_103;
pub const PET_DIARY_INITIALIZE_OPERATE_TYPE: i64 = 27;
pub const PET_DIARY_FEED_OPERATE_TYPE: i64 = 29;
pub const PET_DIARY_DRAW_OPERATE_TYPE: i64 = 30;
pub const PET_DIARY_INTERACT_LOG_OPERATE_TYPE: i64 = 31;
pub const PET_DIARY_CLAIM_STORY_OPERATE_TYPE: i64 = 32;
pub const PET_DIARY_REFRESH_CHARM_OPERATE_TYPE: i64 = 41;
pub const PET_DIARY_EQUIP_CHARM_OPERATE_TYPE: i64 = 42;
pub const PET_DIARY_BATTLE_OPERATE_TYPE: i64 = 43;
pub const PET_DIARY_PLUNDER_LOG_OPERATE_TYPE: i64 = 44;
pub const PET_DIARY_OPEN_TREASURE_OPERATE_TYPE: i64 = 45;
pub const PET_DIARY_COMPENSATION_OPERATE_TYPE: i64 = 46;
pub const PET_DIARY_FRIEND_INFO_OPERATE_TYPE: i64 = 47;
pub const PET_DIARY_CLAIM_DOG_OPERATE_TYPE: i64 = 48;
pub const PET_DIARY_MARK_STORIES_OPERATE_TYPE: i64 = 49;
pub const PET_DIARY_SKIP_BATTLE_OPERATE_TYPE: i64 = 50;
pub const PET_DIARY_SEEDS_CLAIM_ALL_OPERATE_TYPE: i64 = 21;
/// 元气糕（投喂 / 寻宝消耗）
pub const PET_DIARY_FEED_ITEM_ID: i64 = 1028;
/// 幸运星（寻宝产出）
pub const PET_DIARY_LUCKY_STAR_ITEM_ID: i64 = 1029;
/// 钻石：任何消耗钻石的活动操作一律拒绝（用户决策）
pub const DIAMOND_ITEM_ID: i64 = 1004;
/// 夺宝挑战书白名单（80101 初级 / 80102 中级 / 80103 高级）
pub const PET_DIARY_CHALLENGE_ITEM_IDS: [i64; 3] = [80101, 80102, 80103];
/// 比熊幼崽培育至成年后永久获得的宠物
pub const PET_DIARY_DOG_ID: i64 = 90031;

/// 仓库 / 化肥容器
pub const SELL_BATCH_SIZE: usize = 15;
pub const FERTILIZER_CONTAINER_LIMIT_HOURS: i64 = 990;
pub const NORMAL_CONTAINER_ID: i64 = 1011;
pub const ORGANIC_CONTAINER_ID: i64 = 1012;
pub const COLLECTION_NORMAL_ID: i64 = 3001;
pub const COLLECTION_RARE_ID: i64 = 3002;

/// 背包货币（与商城 goodsId 命名空间不同）
pub const COUPON_ITEM_ID: i64 = 1002;
pub const GOLD_BEAN_ITEM_ID: i64 = 1005;

/// 商城商品：有机肥（数字碰巧与点券 1002 相同，但是不同域）
pub const MALL_ORGANIC_FERTILIZER_GOODS_ID: i32 = 1002;

/// 植物操作 ID（`OperationLimit.id` / `CheckCanOperate`）
pub const OP_HARVEST: i64 = 10001;
pub const OP_REMOVE: i64 = 10002;
pub const OP_PUT_WEED: i64 = 10003;
pub const OP_PUT_BUG: i64 = 10004;
pub const OP_HELP_WEED: i64 = 10005;
pub const OP_HELP_BUG: i64 = 10006;
pub const OP_HELP_WATER: i64 = 10007;
/// 偷菜日配额。QQ 有 `day_times`；微信不受限，不要调 `CheckCanOperate(10008)`。
pub const OP_STEAL: i64 = 10008;

/// 微信无 10008 日偷次数；仅 QQ 走 `OperationLimit` / `CheckCanOperate`。
#[must_use]
pub fn steal_daily_quota_applies(platform: &str) -> bool {
    !platform.trim().eq_ignore_ascii_case("wx")
}

/// 好友农场 Harvest：这块地当前不可偷
pub const GATEWAY_UNSTEALABLE: i64 = 1_001_040;
/// 好友 Farming 无事可做
pub const GATEWAY_FARMING_NOOP: i64 = 1_001_057;

/// 微信开放平台 / 桌面与扫码共用的小程序 AppId（面板 wx 登录另有 TARGET）
pub const WX_MINI_APP_ID: &str = "wx5306c5978fdb76e4";

/// 应用宝网站应用 OAuth appid（qrconnect / 本机快速授权）
pub const WX_OAUTH_APP_ID: &str = "wxd44977328b36e647";
pub const WX_OAUTH_SCOPE: &str = "snsapi_login,snsapi_runtime_pcsdk";
pub const WX_OAUTH_STATE: &str = "web";
pub const WX_OAUTH_REDIRECT_URI: &str =
    "https://yybadaccess.3g.qq.com/pc_yyb/pcyyb_oauth?login_type=WX";

/// 桌面微信本地 HTTP API 端口（对齐官方快捷登录：Windows / macOS 均探测全部 6 个）
pub const DESKTOP_WECHAT_PORTS: &[u16] = &[14013, 14014, 14015, 13013, 13014, 13015];
/// 桌面微信本地 HTTPS 主机名（解析到 127.0.0.1）
pub const LOCAL_WECHAT_HOST: &str = "localhost.weixin.qq.com";
pub const LOCAL_WECHAT_CHECK_PATH: &str = "/api/check-login";
pub const LOCAL_WECHAT_AUTHORIZE_PATH: &str = "/api/authorize";

/// 网关 Origin（WS 握手）
pub const DEFAULT_GATEWAY_ORIGIN: &str = "https://gate-obt.nqf.qq.com";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wechat_has_no_steal_daily_quota() {
        assert!(!steal_daily_quota_applies("wx"));
        assert!(!steal_daily_quota_applies("WX"));
        assert!(steal_daily_quota_applies("qq"));
        assert!(steal_daily_quota_applies(""));
    }
}
