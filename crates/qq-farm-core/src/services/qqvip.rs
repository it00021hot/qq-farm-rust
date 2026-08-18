//! QQ 会员 — 每日礼包自动领取。
//!
//! 1:1 翻译原 `core/src/services/qqvip.ts`（134 行）。
//!
//! ## 协议
//!
//! - `gamepb.qqvippb.QQVipService.GetDailyGiftStatus` — 拉取每日礼包状态
//! - `gamepb.qqvippb.QQVipService.ClaimDailyGift` — 领取每日礼包
//!
//! ## 业务
//!
//! - 每日限领一次（跨天重置 + 10min 检查冷却）
//! - "已领取"错误（`code=1021002` / "今日已领取"）视为成功

use std::sync::Arc;

use parking_lot::Mutex;
use prost::Message;
use serde::Serialize;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::qqvippb::{
    ClaimDailyGiftReply, ClaimDailyGiftRequest, GetDailyGiftStatusReply, GetDailyGiftStatusRequest,
};

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
    pub has_gift: Option<bool>,
    pub can_claim: Option<bool>,
}

/// 会员服务
pub struct QQVipService {
    gateway: Arc<Gateway>,
    done_date_key: Mutex<String>,
    last_check_at: Mutex<i64>,
    last_claim_at: Mutex<i64>,
    last_result: Mutex<&'static str>,
    last_has_gift: Mutex<Option<bool>>,
    last_can_claim: Mutex<Option<bool>>,
}

impl QQVipService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            done_date_key: Mutex::new(String::new()),
            last_check_at: Mutex::new(0),
            last_claim_at: Mutex::new(0),
            last_result: Mutex::new(""),
            last_has_gift: Mutex::new(None),
            last_can_claim: Mutex::new(None),
        }
    }

    /// 拉取每日礼包状态
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn get_daily_gift_status(&self) -> Result<GetDailyGiftStatusReply> {
        let body = self
            .gateway
            .request(
                VIP_SERVICE,
                "GetDailyGiftStatus",
                &GetDailyGiftStatusRequest {}.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(GetDailyGiftStatusReply::decode(&body[..])?)
    }

    /// 领取每日礼包
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn claim_daily_gift(&self) -> Result<ClaimDailyGiftReply> {
        let body = self
            .gateway
            .request(
                VIP_SERVICE,
                "ClaimDailyGift",
                &ClaimDailyGiftRequest {}.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(ClaimDailyGiftReply::decode(&body[..])?)
    }

    /// 每日自动领取
    pub async fn perform_daily_vip_gift(&self, force: bool) -> bool {
        let now = now_ms();
        if !force && self.is_done_today() {
            return false;
        }
        if !force && (now - *self.last_check_at.lock()) < CHECK_COOLDOWN_MS {
            return false;
        }
        *self.last_check_at.lock() = now;

        let status = match self.get_daily_gift_status().await {
            Ok(s) => s,
            Err(e) => {
                *self.last_result.lock() = "error";
                tracing::warn!("[会员] 拉取会员礼包状态失败: {}", e);
                return false;
            }
        };
        *self.last_has_gift.lock() = Some(status.has_gift);
        *self.last_can_claim.lock() = Some(status.can_claim);
        if !status.can_claim {
            self.mark_done_today();
            *self.last_result.lock() = "none";
            tracing::info!("[会员] 今日暂无可领取会员礼包");
            return false;
        }
        match self.claim_daily_gift().await {
            Ok(rep) => {
                let reward = get_reward_summary(&rep.items);
                if reward.is_empty() {
                    tracing::info!("[会员] 领取成功");
                } else {
                    tracing::info!("[会员] 领取成功 → {}", reward);
                }
                *self.last_claim_at.lock() = now_ms();
                self.mark_done_today();
                *self.last_result.lock() = "ok";
                true
            }
            Err(e) => {
                if is_already_claimed_error(&e.to_string()) {
                    self.mark_done_today();
                    *self.last_claim_at.lock() = now_ms();
                    *self.last_result.lock() = "ok";
                    tracing::info!("[会员] 今日会员礼包已领取");
                    return false;
                }
                *self.last_result.lock() = "error";
                tracing::warn!("[会员] 领取会员礼包失败: {}", e);
                false
            }
        }
    }

    #[must_use]
    pub fn get_vip_daily_state(&self) -> VipDailyState {
        VipDailyState {
            key: DAILY_KEY,
            done_today: self.is_done_today(),
            last_check_at: *self.last_check_at.lock(),
            last_claim_at: *self.last_claim_at.lock(),
            result: *self.last_result.lock(),
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

/// 判断错误信息是否表示"已领取"
pub fn is_already_claimed_error(msg: &str) -> bool {
    msg.contains("code=1021002") || msg.contains("今日已领取") || msg.contains("已领取")
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

    #[test]
    fn service_constant_matches_ts() {
        assert_eq!(VIP_SERVICE, "gamepb.qqvippb.QQVipService");
    }

    #[test]
    fn daily_key_constant() {
        assert_eq!(DAILY_KEY, "vip_daily_gift");
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
        assert!(is_already_claimed_error("已领取该奖励"));
        assert!(!is_already_claimed_error("其他错误"));
    }

    #[test]
    fn date_key_format() {
        let k = get_date_key();
        assert_eq!(k.len(), 10);
    }
}
