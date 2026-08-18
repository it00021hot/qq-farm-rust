//! 月卡 — 每日自动领取月卡礼包。
//!
//! 1:1 翻译原 `core/src/services/monthcard.ts`（154 行）。
//!
//! ## 协议
//!
//! - `gamepb.mallpb.MallService.GetMonthCardInfos` — 拉取月卡信息
//! - `gamepb.mallpb.MallService.ClaimMonthCardReward` — 领取月卡奖励
//!
//! ## 业务
//!
//! - 每日限领一次（跨天重置 + 10min 检查冷却）
//! - 支持多张月卡：依次领取所有 `can_claim` 的月卡

use std::sync::Arc;

use parking_lot::Mutex;
use prost::Message;
use serde::Serialize;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::mallpb::{
    ClaimMonthCardRewardReply, ClaimMonthCardRewardRequest, GetMonthCardInfosReply,
    GetMonthCardInfosRequest,
};

const MALL_SERVICE: &str = "gamepb.mallpb.MallService";
const DAILY_KEY: &str = "month_card_gift";
const CHECK_COOLDOWN_MS: i64 = 10 * 60 * 1000;

/// 月卡每日状态
#[derive(Debug, Clone, Serialize)]
pub struct MonthCardDailyState {
    pub key: &'static str,
    pub done_today: bool,
    pub last_check_at: i64,
    pub last_claim_at: i64,
    pub result: &'static str,
    pub has_card: Option<bool>,
    pub has_claimable: Option<bool>,
}

/// 月卡服务
pub struct MonthCardService {
    gateway: Arc<Gateway>,
    done_date_key: Mutex<String>,
    last_check_at: Mutex<i64>,
    last_claim_at: Mutex<i64>,
    last_result: Mutex<&'static str>,
    last_has_card: Mutex<Option<bool>>,
    last_has_claimable: Mutex<Option<bool>>,
}

impl MonthCardService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            done_date_key: Mutex::new(String::new()),
            last_check_at: Mutex::new(0),
            last_claim_at: Mutex::new(0),
            last_result: Mutex::new(""),
            last_has_card: Mutex::new(None),
            last_has_claimable: Mutex::new(None),
        }
    }

    /// 拉取月卡信息
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn get_month_card_infos(&self) -> Result<GetMonthCardInfosReply> {
        let body = self
            .gateway
            .request(
                MALL_SERVICE,
                "GetMonthCardInfos",
                &GetMonthCardInfosRequest {}.encode_to_vec(),
            )
            .await?;
        Ok(GetMonthCardInfosReply::decode(&body[..])?)
    }

    /// 领取指定 goodsId 的月卡奖励
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn claim_month_card_reward(
        &self,
        goods_id: i32,
    ) -> Result<ClaimMonthCardRewardReply> {
        let req = ClaimMonthCardRewardRequest { goods_id };
        let body = self
            .gateway
            .request(MALL_SERVICE, "ClaimMonthCardReward", &req.encode_to_vec())
            .await?;
        Ok(ClaimMonthCardRewardReply::decode(&body[..])?)
    }

    /// 每日自动领取月卡礼包
    pub async fn perform_daily_month_card_gift(&self, force: bool) -> bool {
        let now = now_ms();
        if !force && self.is_done_today() {
            return false;
        }
        if !force && (now - *self.last_check_at.lock()) < CHECK_COOLDOWN_MS {
            return false;
        }
        *self.last_check_at.lock() = now;

        let reply = match self.get_month_card_infos().await {
            Ok(r) => r,
            Err(e) => {
                *self.last_result.lock() = "error";
                tracing::warn!("[月卡] 查询月卡礼包失败: {}", e);
                return false;
            }
        };
        let infos = reply.infos;
        *self.last_has_card.lock() = Some(!infos.is_empty());

        if infos.is_empty() {
            self.mark_done_today();
            *self.last_result.lock() = "none";
            tracing::info!("[月卡] 当前没有月卡或已过期");
            return false;
        }

        let claimable: Vec<_> = infos.iter().filter(|x| x.can_claim && x.goods_id > 0).collect();
        *self.last_has_claimable.lock() = Some(!claimable.is_empty());

        if claimable.is_empty() {
            self.mark_done_today();
            *self.last_result.lock() = "none";
            tracing::info!("[月卡] 今日暂无可领取月卡礼包");
            return false;
        }

        let mut claimed: usize = 0;
        for info in claimable {
            let gid = info.goods_id;
            match self.claim_month_card_reward(gid).await {
                Ok(ret) => {
                    let reward = get_reward_summary(&ret.items);
                    if reward.is_empty() {
                        tracing::info!("[月卡] 领取成功");
                    } else {
                        tracing::info!("[月卡] 领取成功 → {}", reward);
                    }
                    claimed += 1;
                }
                Err(e) => {
                    tracing::warn!("[月卡] 领取失败(gid={}): {}", gid, e);
                }
            }
        }

        if claimed > 0 {
            *self.last_claim_at.lock() = now_ms();
            self.mark_done_today();
            *self.last_result.lock() = "ok";
            return true;
        }
        tracing::info!("[月卡] 本次未成功领取月卡礼包");
        *self.last_result.lock() = "none";
        false
    }

    #[must_use]
    pub fn get_month_card_daily_state(&self) -> MonthCardDailyState {
        MonthCardDailyState {
            key: DAILY_KEY,
            done_today: self.is_done_today(),
            last_check_at: *self.last_check_at.lock(),
            last_claim_at: *self.last_claim_at.lock(),
            result: *self.last_result.lock(),
            has_card: *self.last_has_card.lock(),
            has_claimable: *self.last_has_claimable.lock(),
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
///
/// 1:1 对齐原 TS `getRewardSummary`，但简化了 item 来源（proto 中
/// `ClaimMonthCardRewardReply.items` 实际为 `corepb.Item` 列表）。
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
        assert_eq!(MALL_SERVICE, "gamepb.mallpb.MallService");
    }

    #[test]
    fn daily_key_constant() {
        assert_eq!(DAILY_KEY, "month_card_gift");
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
    fn reward_summary_ticket() {
        let items = vec![Item { id: 1002, count: 50, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "点券50");
    }

    #[test]
    fn reward_summary_experience() {
        let items = vec![Item { id: 2, count: 500, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "经验500");
    }

    #[test]
    fn reward_summary_unknown() {
        let items = vec![Item { id: 9999, count: 3, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "物品#9999x3");
    }

    #[test]
    fn reward_summary_skips_zero_count() {
        let items = vec![Item { id: 1, count: 0, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "");
    }

    #[test]
    fn reward_summary_multi() {
        let items = vec![
            Item { id: 1, count: 100, ..Default::default() },
            Item { id: 1002, count: 10, ..Default::default() },
            Item { id: 9999, count: 1, ..Default::default() },
        ];
        let s = get_reward_summary(&items);
        assert!(s.contains("金币100"));
        assert!(s.contains("点券10"));
        assert!(s.contains("物品#9999x1"));
    }

    #[test]
    fn date_key_format() {
        let k = get_date_key();
        assert_eq!(k.len(), 10);
        assert_eq!(k.chars().nth(4), Some('-'));
        assert_eq!(k.chars().nth(7), Some('-'));
    }

    #[test]
    fn claim_request_roundtrip() {
        let req = ClaimMonthCardRewardRequest { goods_id: 1001 };
        let bytes = req.encode_to_vec();
        let back = ClaimMonthCardRewardRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.goods_id, 1001);
    }
}
