//! 同气连枝礼包（宠物 90021 技能掉落，item 101351）。
//!
//! 1:1 对应原 `core/src/services/dog-skill-gifts.ts`：
//! - PendingGiftCountNotify 推送 / 帮忙务农回包奖励 → 自动 ClaimSkillGifts（一次领全部）；
//! - 单飞锁防并发重复领取。

use std::sync::Arc;

use prost::Message as _;

use crate::config::game_config::global as global_game_config;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::dogpb::{
    ClaimSkillGiftsReply, ClaimSkillGiftsRequest, GetDogInfoReply, GetDogInfoRequest,
};

pub use crate::constants::DOG_SKILL_GIFT_ITEM_ID;

const DOG_SERVICE: &str = "gamepb.dogpb.DogService";

pub struct DogSkillGiftService {
    gateway: Arc<Gateway>,
    /// 单飞锁：防止推送 + 面板并发触发重复领取
    pending_claim: tokio::sync::Mutex<()>,
}

impl DogSkillGiftService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, pending_claim: tokio::sync::Mutex::new(()) }
    }

    pub async fn get_dog_info(&self) -> crate::error::Result<GetDogInfoReply> {
        let req = GetDogInfoRequest { host_gid: 0 };
        let body =
            self.gateway.request(DOG_SERVICE, "GetDogInfo", &req.encode_to_vec()).await?;
        Ok(GetDogInfoReply::decode(&body[..])?)
    }

    async fn claim_skill_gifts(&self) -> crate::error::Result<ClaimSkillGiftsReply> {
        let req = ClaimSkillGiftsRequest {};
        let body =
            self.gateway.request(DOG_SERVICE, "ClaimSkillGifts", &req.encode_to_vec()).await?;
        Ok(ClaimSkillGiftsReply::decode(&body[..])?)
    }

    /// 待领取数量
    #[must_use]
    pub fn pending_gift_count(reply: &GetDogInfoReply) -> i64 {
        reply.pending_gift_count.max(0)
    }

    /// 帮忙务农回包中掉落的礼包数量（FarmingReply.results[].reward.id == 101351）
    #[must_use]
    pub fn farming_skill_gift_count(
        results: &[crate::proto::generated::gamepb::plantpb::FarmingResult],
    ) -> i64 {
        results
            .iter()
            .filter_map(|r| r.reward.as_ref())
            .filter(|reward| reward.id == DOG_SKILL_GIFT_ITEM_ID)
            .map(|reward| reward.count.max(0))
            .sum()
    }

    /// 查询并领取（pendingCountHint > 0 时跳过查询）。
    /// 返回 `{claimed, pending, item}`；失败不抛错，返回 error 字段。
    pub async fn check_and_claim(&self, pending_count_hint: i64) -> serde_json::Value {
        let _guard = self.pending_claim.lock().await;
        let result = self.check_and_claim_inner(pending_count_hint).await;
        match result {
            Ok(value) => value,
            Err(err) => {
                crate::services::panel_log::log(
                    "",
                    "宠物",
                    format!("拾取同气连枝礼包失败: {err}"),
                    crate::constants::PanelEvent::DogSkillGift,
                    Some(serde_json::json!({ "module": "dog", "isWarn": true })),
                );
                serde_json::json!({
                    "claimed": 0,
                    "pending": pending_count_hint.max(0),
                    "item": null,
                    "error": err.to_string(),
                })
            }
        }
    }

    async fn check_and_claim_inner(
        &self,
        pending_count_hint: i64,
    ) -> crate::error::Result<serde_json::Value> {
        let pending_count = if pending_count_hint > 0 {
            pending_count_hint
        } else {
            Self::pending_gift_count(&self.get_dog_info().await?)
        };
        if pending_count <= 0 {
            return Ok(serde_json::json!({ "claimed": 0, "pending": 0, "item": null }));
        }

        let reply = self.claim_skill_gifts().await?;
        let item = reply.item.clone();
        let item_id = item.as_ref().map(|i| i.id).unwrap_or(0);
        let item_count = item.as_ref().map(|i| i.count.max(0)).unwrap_or(0);
        let claimed_count = if reply.claimed_count > 0 { reply.claimed_count } else { item_count };
        let item_name = if item_id > 0 {
            global_game_config()
                .get_item_by_id(item_id)
                .map(|i| i.name)
                .unwrap_or_else(|| format!("物品#{item_id}"))
        } else {
            "宠物礼包".to_string()
        };
        if claimed_count > 0 {
            crate::services::panel_log::log(
                "",
                "宠物",
                format!("拾取{item_name} x{claimed_count}"),
                crate::constants::PanelEvent::DogSkillGift,
                Some(serde_json::json!({ "module": "dog", "itemId": item_id, "count": claimed_count })),
            );
        }
        Ok(serde_json::json!({
            "claimed": claimed_count,
            "pending": (pending_count - claimed_count).max(0),
            "item": item.as_ref().map(|i| serde_json::json!({
                "id": i.id,
                "count": i.count,
            })),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farming_gift_count_sums_only_gift_rewards() {
        use crate::proto::generated::corepb::Item;
        use crate::proto::generated::gamepb::plantpb::FarmingResult;
        let mk = |id: i64, count: i64| FarmingResult {
            reward: Some(Item { id, count, ..Default::default() }),
            ..Default::default()
        };
        let results = vec![mk(DOG_SKILL_GIFT_ITEM_ID, 1), mk(1001, 5), mk(DOG_SKILL_GIFT_ITEM_ID, 2)];
        assert_eq!(DogSkillGiftService::farming_skill_gift_count(&results), 3);
        assert_eq!(DogSkillGiftService::farming_skill_gift_count(&[]), 0);
    }
}
