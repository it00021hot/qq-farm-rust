//! 道具 / 容器 ID。

/// 同气连枝礼包（狗狗技能掉落）
pub const DOG_SKILL_GIFT_ITEM_ID: i64 = 101_351;

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
