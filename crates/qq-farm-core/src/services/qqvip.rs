//! QQ 会员 — 每日礼包自动领取 + SVIP 商城免费礼包。
//!
//! 1:1 翻译原 `core/src/services/qqvip.ts`（bot fecf531 重构后）。
//!
//! ## 协议
//!
//! - `gamepb.qqvippb.QQVipService.GetQQVipRewardsStatus` — 拉取会员礼包状态
//!   （field 2 = can_claim、3 = claimed_today、4 = rewards_can_claim、7 = remaining_days、
//!   9 = mall_free_can_claim；子项 field 1 = type、3 = is_enable、4 = reward_type）
//! - `gamepb.qqvippb.QQVipService.ClaimQQVipRewards` — 领取会员礼包
//!
//! ## 业务（bot fecf531 后）
//!
//! - 每日限领一次（跨天重置 + 10min 检查冷却）；强制重查先清「已完成」标记，
//!   避免旧标记掩盖本次失败
//! - 常规日查直接查权益（`GetQQVipRewardsStatus`，与游戏客户端一致）；
//!   `RefreshVipInfo` 是订阅状态同步、可能独立失败，不再参与日查
//! - 非会员由 `is_qq_vip` 直接判定；可领判定：`type === 1 ? can_claim : rewards_can_claim`
//! - 会员礼包与 SVIP 商城免费礼包两个来源**独立容错**：部分失败记 `partial`
//!   且不 markDoneToday（当天冷却后重试）；SVIP 商城礼包（slot 4 免费、非广告、
//!   非分享、未拥有、未达限购）自动逐个购买
//! - "已领取"错误（`code=1021002` / "今日已领取"）视为成功

use std::sync::Arc;

use parking_lot::Mutex;
use prost::Message;
use serde::Serialize;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::mallpb::MallGoods;
use crate::proto::generated::gamepb::qqvippb::{
    ClaimQqVipRewardsReply, ClaimQqVipRewardsRequest, GetQqVipRewardsStatusReply,
    GetQqVipRewardsStatusRequest,
};

use crate::services::mall::{is_svip_free_gift_goods, MallService};

const VIP_SERVICE: &str = "gamepb.qqvippb.QQVipService";
const DAILY_KEY: &str = "vip_daily_gift";
const CHECK_COOLDOWN_MS: i64 = 10 * 60 * 1000;
/// 会员每日状态
#[derive(Debug, Clone, Serialize)]
pub struct VipDailyState {
    pub key: &'static str,
    pub done_today: bool,
    pub last_check_at: i64,
    pub last_claim_at: i64,
    pub result: &'static str,
    /// 最近一次失败详情（bot fecf531；空串表示无失败）
    pub last_error: String,
    /// 三态：None=尚未检查（bot `isVip: null`）
    pub is_vip: Option<bool>,
    pub has_gift: Option<bool>,
    pub can_claim: Option<bool>,
}

/// 会员服务
pub struct QQVipService {
    gateway: Arc<Gateway>,
    /// SVIP 商城免费礼包走商城协议（bot 里 `require('./mall')` 懒加载，这里直接注入）
    mall: Arc<MallService>,
    done_date_key: Mutex<String>,
    last_check_at: Mutex<i64>,
    last_claim_at: Mutex<i64>,
    last_result: Mutex<&'static str>,
    last_error: Mutex<String>,
    last_is_vip: Mutex<Option<bool>>,
    last_has_gift: Mutex<Option<bool>>,
    last_can_claim: Mutex<Option<bool>>,
}

impl QQVipService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>, mall: Arc<MallService>) -> Self {
        Self {
            gateway,
            mall,
            done_date_key: Mutex::new(String::new()),
            last_check_at: Mutex::new(0),
            last_claim_at: Mutex::new(0),
            last_result: Mutex::new(""),
            last_error: Mutex::new(String::new()),
            last_is_vip: Mutex::new(None),
            last_has_gift: Mutex::new(None),
            last_can_claim: Mutex::new(None),
        }
    }

    /// 拉取会员礼包状态（bot `getQQVipRewardsStatus`）
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn get_qq_vip_rewards_status(&self) -> Result<GetQqVipRewardsStatusReply> {
        let body = self
            .gateway
            .request(
                VIP_SERVICE,
                "GetQQVipRewardsStatus",
                &GetQqVipRewardsStatusRequest {}.encode_to_vec(),
            )
            .await?;
        Ok(GetQqVipRewardsStatusReply::decode(&body[..])?)
    }

    /// 领取当前可领的会员礼包。
    ///
    /// # Errors
    /// - 网络 / 网关错误；「已领取 / 非 QQ 会员」由调用方按错误文案识别
    pub async fn claim_daily_gift(&self) -> Result<ClaimQqVipRewardsReply> {
        let status = self.get_qq_vip_rewards_status().await?;
        let mut reward_types = claimable_reward_types(&status);
        reward_types.dedup();
        if reward_types.is_empty() {
            return Ok(ClaimQqVipRewardsReply::default());
        }
        let body = self
            .gateway
            .request(
                VIP_SERVICE,
                "ClaimQQVipRewards",
                &ClaimQqVipRewardsRequest { reward_types }.encode_to_vec(),
            )
            .await?;
        Ok(ClaimQqVipRewardsReply::decode(&body[..])?)
    }

    /// SVIP 商城免费礼包自动领取（bot `claimSvipMallFreeGift`）。
    ///
    /// 前置：`is_qq_vip && mall_free_can_claim`；对 slot 4 中免费、非广告、非分享、
    /// 未拥有、未达限购的商品逐个购买 1 件。任一商品失败即整体报错（对齐 bot await 链）。
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - 购买未确认（见 [`MallService::purchase_mall_goods`])
    pub async fn claim_svip_mall_free_gift(
        &self,
        status: &GetQqVipRewardsStatusReply,
    ) -> Result<bool> {
        if !status.is_qq_vip || !status.mall_free_can_claim {
            return Ok(false);
        }
        let reply = self.mall.get_mall_list_by_slot_type(4, 0).await?;
        let goods: Vec<MallGoods> =
            reply.goods_list.iter().filter(|g| is_svip_free_gift_goods(g)).cloned().collect();
        for product in &goods {
            self.mall.purchase_mall_goods(product.goods_id, 1).await?;
        }
        Ok(!goods.is_empty())
    }

    /// 每日自动领取（对齐 bot fecf531 后的 `performDailyVipGift`）。
    ///
    /// 与旧版的关键差异：
    /// - 不再先调 `RefreshVipInfo`：常规日查直接查权益（与游戏客户端一致），
    ///   订阅状态同步是独立请求、可能独立失败，不再挡住整个流程；
    /// - 非会员由 `status.is_qq_vip` 直接判定（错误分支只作兜底）；
    /// - 会员礼包与 SVIP 商城礼包两个来源独立容错，部分失败记 `partial` 且
    ///   **不** markDoneToday（当天还会重试）。
    pub async fn perform_daily_vip_gift(&self, force: bool) -> bool {
        let now = now_ms();
        if !force && self.is_done_today() {
            return false;
        }
        if !force && (now - *self.last_check_at.lock()) < CHECK_COOLDOWN_MS {
            return false;
        }
        *self.last_check_at.lock() = now;
        // 强制重查不得让更早的「已完成」标记掩盖本次失败
        *self.done_date_key.lock() = String::new();
        *self.last_error.lock() = String::new();
        *self.last_is_vip.lock() = None;
        *self.last_has_gift.lock() = None;
        *self.last_can_claim.lock() = None;

        let status = match self.get_qq_vip_rewards_status().await {
            Ok(s) => s,
            Err(e) => {
                if is_not_qq_vip_error(&e.to_string()) {
                    self.mark_non_member();
                } else {
                    *self.last_result.lock() = "error";
                    let message = Self::record_failure("查询会员资格", &e);
                    *self.last_error.lock() = message;
                }
                return false;
            }
        };

        *self.last_is_vip.lock() = Some(status.is_qq_vip);
        if !status.is_qq_vip {
            self.mark_non_member();
            return false;
        }

        let mut reward_types = claimable_reward_types(&status);
        reward_types.dedup();
        let mall_can_claim = status.mall_free_can_claim;
        *self.last_has_gift.lock() =
            Some(status.reward_statuses.iter().any(|item| item.is_enable) || mall_can_claim);
        *self.last_can_claim.lock() = Some(!reward_types.is_empty() || mall_can_claim);

        let mut errors: Vec<String> = Vec::new();
        let mut claimed = false;
        let mut already_claimed = status.claimed_today;

        // 两个奖励来源相互独立：商城失败不挡会员礼包，反之亦然
        if !reward_types.is_empty() {
            match self.claim_reward_types(reward_types).await {
                Ok(rep) => {
                    let reward = get_reward_summary(&rep.items);
                    claimed = true;
                    *self.last_claim_at.lock() = now_ms();
                    if reward.is_empty() {
                        tracing::info!("[会员] 会员礼包领取成功");
                    } else {
                        tracing::info!("[会员] 会员礼包领取成功 → {reward}");
                    }
                }
                Err(e) => {
                    if is_not_qq_vip_error(&e.to_string()) {
                        self.mark_non_member();
                        return false;
                    }
                    if is_already_claimed_error(&e.to_string()) {
                        already_claimed = true;
                        tracing::info!("[会员] 今日会员礼包已领取");
                    } else {
                        let message = Self::record_failure("领取会员礼包", &e);
                        errors.push(message);
                    }
                }
            }
        }

        if mall_can_claim {
            match self.claim_svip_mall_free_gift(&status).await {
                Ok(true) => {
                    claimed = true;
                    *self.last_claim_at.lock() = now_ms();
                    tracing::info!("[会员] SVIP 商城免费礼包领取成功");
                }
                Ok(false) => {}
                Err(e) => {
                    let message = Self::record_failure("领取 SVIP 商城免费礼包", &e);
                    errors.push(message);
                }
            }
        }

        if errors.is_empty() {
            self.mark_done_today();
            *self.last_can_claim.lock() = Some(false);
            *self.last_result.lock() = if claimed || already_claimed { "ok" } else { "none" };
            if !claimed && !already_claimed {
                tracing::info!("[会员] 今日暂无可领取会员礼包");
            }
        } else {
            *self.last_result.lock() = if claimed || already_claimed { "partial" } else { "error" };
            *self.last_error.lock() = errors.join("；");
        }
        claimed
    }

    fn record_failure(stage: &str, error: &crate::error::Error) -> String {
        // 对齐 bot：截断到 500（bot slice 按字节，这里按字符边界截，防中文劈半 panic）
        let text = error.to_string();
        let truncated: String = text.chars().take(500).collect();
        let message = format!("{stage}失败: {truncated}");
        tracing::warn!("[会员] {message}");
        message
    }

    fn mark_non_member(&self) {
        self.mark_done_today();
        *self.last_result.lock() = "none";
        *self.last_error.lock() = String::new();
        *self.last_is_vip.lock() = Some(false);
        *self.last_has_gift.lock() = Some(false);
        *self.last_can_claim.lock() = Some(false);
        tracing::info!("[会员] 当前账号非 QQ 会员，今日跳过会员礼包");
    }

    async fn claim_reward_types(&self, reward_types: Vec<i64>) -> Result<ClaimQqVipRewardsReply> {
        let body = self
            .gateway
            .request(
                VIP_SERVICE,
                "ClaimQQVipRewards",
                &ClaimQqVipRewardsRequest { reward_types }.encode_to_vec(),
            )
            .await?;
        Ok(ClaimQqVipRewardsReply::decode(&body[..])?)
    }

    #[must_use]
    pub fn get_vip_daily_state(&self) -> VipDailyState {
        VipDailyState {
            key: DAILY_KEY,
            done_today: self.is_done_today(),
            last_check_at: *self.last_check_at.lock(),
            last_claim_at: *self.last_claim_at.lock(),
            result: *self.last_result.lock(),
            last_error: self.last_error.lock().clone(),
            is_vip: *self.last_is_vip.lock(),
            has_gift: *self.last_has_gift.lock(),
            can_claim: *self.last_can_claim.lock(),
        }
    }

    fn is_done_today(&self) -> bool {
        *self.done_date_key.lock() == get_date_key()
    }

    fn mark_done_today(&self) {
        *self.done_date_key.lock() = get_date_key();
    }
}

// =====================================================================
// 纯函数
// =====================================================================

/// 可领奖励类型（1:1 对齐 bot `qqvip.ts:96-100`）：
/// `is_enable && is_qq_vip && (type == 1 ? can_claim : rewards_can_claim)`，取 reward_type > 0。
#[must_use]
pub fn claimable_reward_types(status: &GetQqVipRewardsStatusReply) -> Vec<i64> {
    status
        .reward_statuses
        .iter()
        .filter(|item| {
            item.is_enable
                && status.is_qq_vip
                && if item.r#type == 1 { status.can_claim } else { status.rewards_can_claim }
        })
        .map(|item| item.reward_type)
        .filter(|reward_type| *reward_type > 0)
        .collect()
}

/// 汇总奖励为可读字符串
pub fn get_reward_summary(items: &[Item]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for it in items {
        let id = it.id;
        let count = it.count;
        if count <= 0 {
            continue;
        }
        if id == 1 || id == 1001 {
            parts.push(format!("金币{}", count));
        } else if id == 2 || id == 1101 {
            parts.push(format!("经验{}", count));
        } else if id == 1002 {
            parts.push(format!("点券{}", count));
        } else {
            parts.push(format!("物品#{}x{}", id, count));
        }
    }
    parts.join("/")
}

/// 判断错误信息是否表示"已领取"（bot fecf531 收窄：仅错误码与「今日已领取」，
/// 泛化的「已领取」子串不再命中，避免把其它业务文案误判成成功）
pub fn is_already_claimed_error(msg: &str) -> bool {
    msg.contains("code=1021002") || msg.contains("今日已领取")
}

/// 判断错误信息是否表示"非 QQ 会员"（bot `NOT_QQ_VIP_ERROR_CODE`）
pub fn is_not_qq_vip_error(msg: &str) -> bool {
    msg.contains("code=1021001")
}

fn get_date_key() -> String {
    use chrono::Datelike;
    use chrono::Local;
    let now = Local::now();
    format!("{}-{:02}-{:02}", now.year(), now.month(), now.day())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::qqvippb::QqVipRewardStatus;

    #[test]
    fn service_constant_matches_ts() {
        assert_eq!(VIP_SERVICE, "gamepb.qqvippb.QQVipService");
    }

    #[test]
    fn daily_key_constant() {
        assert_eq!(DAILY_KEY, "vip_daily_gift");
    }

    fn status(
        can_claim: bool,
        rewards_can_claim: bool,
        is_qq_vip: bool,
    ) -> GetQqVipRewardsStatusReply {
        GetQqVipRewardsStatusReply { can_claim, rewards_can_claim, is_qq_vip, ..Default::default() }
    }

    fn item(r#type: i32, reward_type: i64, is_enable: bool) -> QqVipRewardStatus {
        QqVipRewardStatus { r#type, reward_type, is_enable, ..Default::default() }
    }

    #[test]
    fn claimable_types_follow_bot_gate() {
        // type==1 走 can_claim，其余走 rewards_can_claim；非会员全不可领
        let mut s = status(true, false, true);
        s.reward_statuses = vec![item(1, 101, true), item(2, 202, true)];
        assert_eq!(claimable_reward_types(&s), vec![101]);

        let mut s2 = status(false, true, true);
        s2.reward_statuses = vec![item(1, 101, true), item(2, 202, true)];
        assert_eq!(claimable_reward_types(&s2), vec![202]);

        let mut s3 = status(true, true, false);
        s3.reward_statuses = vec![item(1, 101, true), item(2, 202, true)];
        assert!(claimable_reward_types(&s3).is_empty());

        let mut s4 = status(true, true, true);
        s4.reward_statuses = vec![item(1, 101, false), item(2, 0, true)];
        assert!(claimable_reward_types(&s4).is_empty());
    }

    #[test]
    fn reward_summary_empty() {
        assert_eq!(get_reward_summary(&[]), "");
    }

    #[test]
    fn reward_summary_gold() {
        let items = vec![Item { id: 1, count: 1000, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "金币1000");
    }

    #[test]
    fn reward_summary_multi() {
        let items = vec![
            Item { id: 1, count: 100, ..Default::default() },
            Item { id: 1002, count: 10, ..Default::default() },
        ];
        let s = get_reward_summary(&items);
        assert!(s.contains("金币100"));
        assert!(s.contains("点券10"));
    }

    #[test]
    fn already_claimed_detection() {
        assert!(is_already_claimed_error("code=1021002"));
        assert!(is_already_claimed_error("今日已领取"));
        // bot fecf531：泛化「已领取」子串不再命中
        assert!(!is_already_claimed_error("已领取该奖励"));
        assert!(!is_already_claimed_error("其他错误"));
    }

    #[test]
    fn not_qq_vip_detection() {
        assert!(is_not_qq_vip_error("业务错误: code=1021001 非 QQ 会员"));
        assert!(!is_not_qq_vip_error("code=1021002"));
        assert!(!is_not_qq_vip_error("网络超时"));
    }

    #[test]
    fn date_key_format() {
        let k = get_date_key();
        assert_eq!(k.len(), 10);
    }
}
