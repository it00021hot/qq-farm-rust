//! 活动类型、操作码、道具 / 容器 ID。

/// 活动类型（对齐 SeasonActivity.type）
pub const SHOP_ACTIVITY_TYPE: i64 = 3;
pub const CONSTELLATION_ACTIVITY_TYPE: i64 = 13;

/// 活动操作类型
pub const EXCHANGE_SHOP_OPERATE_TYPE: i64 = 1;
pub const QUERY_SHOP_OPERATE_TYPE: i64 = 7;
pub const LIGHT_CONSTELLATION_OPERATE_TYPE: i64 = 21;

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

/// 仓库 / 化肥容器
pub const SELL_BATCH_SIZE: usize = 15;
pub const FERTILIZER_CONTAINER_LIMIT_HOURS: i64 = 990;
pub const NORMAL_CONTAINER_ID: i64 = 1011;
pub const ORGANIC_CONTAINER_ID: i64 = 1012;

/// 微信开放平台 / 桌面与扫码共用的小程序 AppId（面板 wx 登录另有 TARGET）
pub const WX_MINI_APP_ID: &str = "wx5306c5978fdb76e4";

/// 网关 Origin（WS 握手）
pub const DEFAULT_GATEWAY_ORIGIN: &str = "https://gate-obt.nqf.qq.com";
