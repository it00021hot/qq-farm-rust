//! 新手引导 — 完成引导节点 / 领取引导奖励。
//!
//! 1:1 翻译原 `core/src/services/guide.ts`（66 行）。
//!
//! ## 协议
//!
//! - `gamepb.guidepb.GuideService.SetWeakGuideNodeComplete` — 标记引导节点完成
//! - `gamepb.guidepb.GuideService.ClaimWeakGuideReward` — 领取引导节点奖励

use std::sync::Arc;

use prost::Message;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::guidepb::{
    ClaimWeakGuideRewardReply, ClaimWeakGuideRewardRequest, SetWeakGuideNodeCompleteReply,
    SetWeakGuideNodeCompleteRequest,
};

const GUIDE_SERVICE: &str = "gamepb.guidepb.GuideService";

/// 引导节点奖励领取结果
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuideClaimResult {
    /// 是否至少成功领取了 1 次（实际拿到奖励的次数）
    pub claimed: i32,
    /// 累计获得的奖励物品条目数
    pub reward_items: usize,
}

/// 引导服务
pub struct GuideService {
    gateway: Arc<Gateway>,
}

impl GuideService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 标记指定引导节点完成
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn set_weak_guide_node_complete(
        &self,
        node_id: i64,
    ) -> Result<SetWeakGuideNodeCompleteReply> {
        let req = SetWeakGuideNodeCompleteRequest { node_id };
        let body = self
            .gateway
            .request(
                GUIDE_SERVICE,
                "SetWeakGuideNodeComplete",
                &req.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(SetWeakGuideNodeCompleteReply::decode(&body[..])?)
    }

    /// 领取指定引导节点的奖励
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn claim_weak_guide_reward(
        &self,
        node_id: i64,
    ) -> Result<ClaimWeakGuideRewardReply> {
        let req = ClaimWeakGuideRewardRequest { node_id };
        let body = self
            .gateway
            .request(
                GUIDE_SERVICE,
                "ClaimWeakGuideReward",
                &req.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(ClaimWeakGuideRewardReply::decode(&body[..])?)
    }

    /// 业务便捷入口：尝试领取通用引导奖励（`nodeId=0`）
    pub async fn claim_guide_rewards(&self) -> GuideClaimResult {
        match self.claim_weak_guide_reward(0).await {
            Ok(reply) => {
                let items = reply.items.len();
                if items > 0 {
                    let reward = get_reward_summary(&reply.items);
                    if reward.is_empty() {
                        tracing::info!("[引导] 领取引导奖励成功");
                    } else {
                        tracing::info!("[引导] 领取引导奖励 → {}", reward);
                    }
                    GuideClaimResult {
                        claimed: 1,
                        reward_items: items,
                    }
                } else {
                    GuideClaimResult::default()
                }
            }
            Err(e) => {
                tracing::warn!("[引导] 领取引导奖励失败: {}", e);
                GuideClaimResult::default()
            }
        }
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

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_constant_matches_ts() {
        assert_eq!(GUIDE_SERVICE, "gamepb.guidepb.GuideService");
    }

    #[test]
    fn reward_summary_empty() {
        assert_eq!(get_reward_summary(&[]), "");
    }

    #[test]
    fn reward_summary_gold() {
        let items = vec![Item {
            id: 1,
            count: 500,
            ..Default::default()
        }];
        assert_eq!(get_reward_summary(&items), "金币500");
    }

    #[test]
    fn reward_summary_experience() {
        let items = vec![Item {
            id: 2,
            count: 1000,
            ..Default::default()
        }];
        assert_eq!(get_reward_summary(&items), "经验1000");
    }

    #[test]
    fn reward_summary_ticket() {
        let items = vec![Item {
            id: 1002,
            count: 5,
            ..Default::default()
        }];
        assert_eq!(get_reward_summary(&items), "点券5");
    }

    #[test]
    fn reward_summary_skips_zero() {
        let items = vec![Item {
            id: 1,
            count: 0,
            ..Default::default()
        }];
        assert_eq!(get_reward_summary(&items), "");
    }

    #[test]
    fn reward_summary_unknown_id() {
        let items = vec![Item {
            id: 12345,
            count: 1,
            ..Default::default()
        }];
        assert_eq!(get_reward_summary(&items), "物品#12345x1");
    }

    #[test]
    fn reward_summary_multi_joined() {
        let items = vec![
            Item { id: 1, count: 100, ..Default::default() },
            Item { id: 1101, count: 50, ..Default::default() },
        ];
        let s = get_reward_summary(&items);
        assert!(s.contains("金币100"));
        assert!(s.contains("经验50"));
        assert!(s.contains('/'));
    }

    #[test]
    fn set_node_request_roundtrip() {
        let req = SetWeakGuideNodeCompleteRequest { node_id: 42 };
        let bytes = req.encode_to_vec();
        let back = SetWeakGuideNodeCompleteRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.node_id, 42);
    }

    #[test]
    fn claim_request_roundtrip() {
        let req = ClaimWeakGuideRewardRequest { node_id: 0 };
        let bytes = req.encode_to_vec();
        let back = ClaimWeakGuideRewardRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.node_id, 0);
    }

    #[test]
    fn guide_claim_result_default() {
        let r = GuideClaimResult::default();
        assert_eq!(r.claimed, 0);
        assert_eq!(r.reward_items, 0);
    }
}
