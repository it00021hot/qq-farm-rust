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
pub const COLLECTION_NORMAL_ID: i64 = 3001;
pub const COLLECTION_RARE_ID: i64 = 3002;

/// 背包货币（与商城 goodsId 命名空间不同）
pub const COUPON_ITEM_ID: i64 = 1002;
pub const GOLD_BEAN_ITEM_ID: i64 = 1005;

/// 商城商品：有机肥（数字碰巧与点券 1002 相同，但是不同域）
pub const MALL_ORGANIC_FERTILIZER_GOODS_ID: i32 = 1002;

/// 微信开放平台 / 桌面与扫码共用的小程序 AppId（面板 wx 登录另有 TARGET）
pub const WX_MINI_APP_ID: &str = "wx5306c5978fdb76e4";

/// 应用宝网站应用 OAuth appid（qrconnect / 本机快速授权）
pub const WX_OAUTH_APP_ID: &str = "wxd44977328b36e647";
pub const WX_OAUTH_SCOPE: &str = "snsapi_login,snsapi_runtime_pcsdk";
pub const WX_OAUTH_STATE: &str = "web";
pub const WX_OAUTH_REDIRECT_URI: &str =
    "https://yybadaccess.3g.qq.com/pc_yyb/pcyyb_oauth?login_type=WX";

/// 桌面微信本地 HTTP API 端口（Windows fast_login）
pub const DESKTOP_WECHAT_PORTS: &[u16] = &[14013, 14014, 14015, 13013, 13014, 13015];
/// 桌面微信本地 HTTPS 主机名（解析到 127.0.0.1）
pub const LOCAL_WECHAT_HOST: &str = "localhost.weixin.qq.com";
pub const LOCAL_WECHAT_CHECK_PATH: &str = "/api/check-login";
pub const LOCAL_WECHAT_AUTHORIZE_PATH: &str = "/api/authorize";

/// 网关 Origin（WS 握手）
pub const DEFAULT_GATEWAY_ORIGIN: &str = "https://gate-obt.nqf.qq.com";
