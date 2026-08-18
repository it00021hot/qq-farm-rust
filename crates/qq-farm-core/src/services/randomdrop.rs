//! 随机掉落活动 — 拉取活动信息 / 奖励。
//!
//! 1:1 翻译原 `core/src/services/randomdrop.ts`（35 行）。
//!
//! ## 协议
//!
//! - `gamepb.randomdroppb.RandomDropService.GetActivityInfo`

use std::sync::Arc;

use prost::Message;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::randomdroppb::{
    DropActivityInfo, GetActivityInfoReply, GetActivityInfoRequest,
};

const RANDOM_DROP_SERVICE: &str = "gamepb.randomdroppb.RandomDropService";

/// 活动 DTO
#[derive(Debug, Clone)]
pub struct ActivityInfoLite {
    pub activity_id: i64,
    pub name: String,
    pub status: i32,
    pub begin_time: i64,
    pub end_time: i64,
    pub drop_count: i32,
    pub max_drop_count: i32,
    pub rewards: Vec<DropRewardLite>,
}

/// 单条掉落奖励 DTO
#[derive(Debug, Clone)]
pub struct DropRewardLite {
    pub item_id: i64,
    pub count: i64,
    pub probability: i32,
    pub claimed: bool,
}

/// 随机掉落服务
pub struct RandomDropService {
    gateway: Arc<Gateway>,
}

impl RandomDropService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 拉取所有生效的掉落活动
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn get_activity_info(&self) -> Result<Vec<ActivityInfoLite>> {
        let body = self
            .gateway
            .request(
                RANDOM_DROP_SERVICE,
                "GetActivityInfo",
                &GetActivityInfoRequest {}.encode_to_vec(),
            )
            .await?;
        let reply = GetActivityInfoReply::decode(&body[..])?;
        Ok(reply.activities.iter().map(activity_info_lite).collect())
    }
}

// =====================================================================
// 纯函数 / DTO 转换
// =====================================================================

/// 把 `DropActivityInfo` 转为简化 DTO
#[must_use]
pub fn activity_info_lite(a: &DropActivityInfo) -> ActivityInfoLite {
    ActivityInfoLite {
        activity_id: a.activity_id,
        name: a.name.clone(),
        status: a.status,
        begin_time: a.begin_time,
        end_time: a.end_time,
        drop_count: a.drop_count,
        max_drop_count: a.max_drop_count,
        rewards: a.rewards.iter().map(drop_reward_lite).collect(),
    }
}

#[must_use]
pub fn drop_reward_lite(
    r: &crate::proto::generated::gamepb::randomdroppb::DropReward,
) -> DropRewardLite {
    DropRewardLite {
        item_id: r.item_id,
        count: r.count,
        probability: r.probability,
        claimed: r.claimed,
    }
}

/// 汇总奖励为可读字符串
#[must_use]
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
    use crate::proto::generated::gamepb::randomdroppb::DropReward;

    #[test]
    fn service_constant_matches_ts() {
        assert_eq!(RANDOM_DROP_SERVICE, "gamepb.randomdroppb.RandomDropService");
    }

    #[test]
    fn reward_summary_empty() {
        assert_eq!(get_reward_summary(&[]), "");
    }

    #[test]
    fn reward_summary_gold() {
        let items = vec![Item { id: 1, count: 100, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "金币100");
    }

    #[test]
    fn reward_summary_ticket() {
        let items = vec![Item { id: 1002, count: 50, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "点券50");
    }

    #[test]
    fn reward_summary_skips_zero() {
        let items = vec![Item { id: 1, count: 0, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "");
    }

    #[test]
    fn reward_summary_unknown() {
        let items = vec![Item { id: 9999, count: 1, ..Default::default() }];
        assert_eq!(get_reward_summary(&items), "物品#9999x1");
    }

    #[test]
    fn activity_info_lite_basic() {
        let a = DropActivityInfo {
            activity_id: 100,
            name: "Test Activity".to_string(),
            status: 1,
            begin_time: 1000,
            end_time: 2000,
            drop_count: 3,
            max_drop_count: 10,
            rewards: vec![DropReward { item_id: 1, count: 100, probability: 5000, claimed: false }],
        };
        let lite = activity_info_lite(&a);
        assert_eq!(lite.activity_id, 100);
        assert_eq!(lite.name, "Test Activity");
        assert_eq!(lite.rewards.len(), 1);
        assert_eq!(lite.rewards[0].item_id, 1);
        assert_eq!(lite.rewards[0].probability, 5000);
        assert!(!lite.rewards[0].claimed);
    }

    #[test]
    fn drop_reward_lite_basic() {
        let r = DropReward { item_id: 42, count: 7, probability: 100, claimed: true };
        let lite = drop_reward_lite(&r);
        assert_eq!(lite.item_id, 42);
        assert_eq!(lite.count, 7);
        assert_eq!(lite.probability, 100);
        assert!(lite.claimed);
    }

    #[test]
    fn activity_info_lite_default() {
        let a = DropActivityInfo::default();
        let lite = activity_info_lite(&a);
        assert_eq!(lite.activity_id, 0);
        assert!(lite.rewards.is_empty());
    }

    #[test]
    fn encode_request_roundtrip() {
        let req = GetActivityInfoRequest {};
        let bytes = req.encode_to_vec();
        let back = GetActivityInfoRequest::decode(bytes.as_slice()).unwrap();
        // 空消息没有字段
        let _ = back;
    }
}
