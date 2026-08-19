use prost::Message;

use crate::constants::{
    ACTIVITY_SERVICE, QIXI_BRIDGE_ACTIVITY_ID, QIXI_BRIDGE_OPERATE_TYPE, QIXI_FEATHER_ITEM_ID,
    QIXI_GIFT_ACTIVITY_ID, QIXI_GIFT_OPERATE_TYPE, QIXI_GROUP_ID, QIXI_RECEIVED_SACHET_ITEM_ID,
    QIXI_SACHET_ITEM_ID,
};
use crate::error::Result;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::{
    ActivityData, ActivityOperateReply, ClaimQixiBridgeRewardsRequest, GetGroupReply,
    GetGroupRequest, GiftQixiSachetRequest,
};

use super::dto::{activity_item_dto, item_dto, item_from_id, text_content};
use super::error::{ActivityError, ActivityErrorCode};
use super::ActivityCenterService;
use super::ItemDto;

fn qixi_err(code: ActivityErrorCode, message: &str) -> ActivityError {
    ActivityError { code, message: message.to_string() }
}

fn find_qixi_child(group: Option<&ActivityData>, activity_id: i64) -> Option<&ActivityData> {
    group?.children.iter().find(|child| {
        child.activity.as_ref().is_some_and(|activity| activity.activity_id == activity_id)
    })
}

fn qixi_activity_active(begin_time: i64, end_time: i64, server_time: i64) -> bool {
    if begin_time > 0 && server_time < begin_time {
        return false;
    }
    if end_time > 0 && server_time > end_time {
        return false;
    }
    true
}

fn bag_balances(items: &[CoreItem], ids: &[i64]) -> Option<std::collections::HashMap<i64, i64>> {
    let mut out: std::collections::HashMap<i64, i64> = ids.iter().copied().map(|id| (id, 0)).collect();
    let wanted: std::collections::HashSet<i64> = ids.iter().copied().collect();
    for item in items {
        if wanted.contains(&item.id) && item.count > 0 {
            *out.entry(item.id).or_insert(0) += item.count;
        }
    }
    Some(out)
}

fn balance_text(balances: Option<&std::collections::HashMap<i64, i64>>, id: i64) -> Option<String> {
    balances.map(|map| map.get(&id).copied().unwrap_or(0).to_string())
}

impl ActivityCenterService {
    async fn qixi_balances(&self) -> Option<std::collections::HashMap<i64, i64>> {
        let warehouse = self.warehouse.lock().clone();
        let bag = if let Some(wh) = warehouse {
            wh.get_bag().await.ok()?
        } else {
            crate::services::warehouse::WarehouseService::get_bag_via(&self.gateway).await.ok()?
        };
        let items = bag.item_bag.as_ref().map(|b| b.items.as_slice()).unwrap_or(&[]);
        bag_balances(
            items,
            &[QIXI_FEATHER_ITEM_ID, QIXI_SACHET_ITEM_ID, QIXI_RECEIVED_SACHET_ITEM_ID],
        )
    }

    fn qixi_dto(
        &self,
        group_reply: &GetGroupReply,
        balances: Option<&std::collections::HashMap<i64, i64>>,
    ) -> Result<serde_json::Value> {
        let group = group_reply.group.as_ref().ok_or_else(|| {
            qixi_err(ActivityErrorCode::QixiUnavailable, "服务端未发现鹊桥寄情活动")
        })?;
        let bridge_child = find_qixi_child(Some(group), QIXI_BRIDGE_ACTIVITY_ID);
        let mut gift_child = find_qixi_child(Some(group), QIXI_GIFT_ACTIVITY_ID);
        if gift_child.is_none() {
            gift_child = bridge_child;
        }
        let (Some(bridge_child), Some(gift_child)) = (bridge_child, gift_child) else {
            return Err(qixi_err(ActivityErrorCode::QixiUnavailable, "服务端未发现鹊桥寄情活动").into());
        };
        let bridge_activity = bridge_child.activity.as_ref().ok_or_else(|| {
            qixi_err(ActivityErrorCode::QixiUnavailable, "服务端未发现鹊桥寄情活动")
        })?;
        let _gift_activity = gift_child.activity.as_ref().ok_or_else(|| {
            qixi_err(ActivityErrorCode::QixiUnavailable, "服务端未发现鹊桥寄情活动")
        })?;
        let server_time = crate::utils::time::get_server_time_secs();
        let config = bridge_child.qixi_bridge.clone().unwrap_or_default();
        let gift = gift_child.qixi_gift.clone().unwrap_or_default();
        let current_stage = config.current_stage;
        let bridge_claimable = bridge_activity.field_23 != 0;
        let stages: Vec<serde_json::Value> = config
            .stages
            .iter()
            .map(|stage| {
                let status_code = stage.status.to_string();
                let completed =
                    status_code == "2" || (current_stage > 0 && stage.stage > 0 && stage.stage <= current_stage);
                let claimable = bridge_claimable && stage.stage == current_stage;
                let rewards: Vec<ItemDto> =
                    stage.rewards.iter().map(activity_item_dto).collect();
                let cost = stage
                    .cost
                    .as_ref()
                    .map(activity_item_dto)
                    .unwrap_or_else(|| item_from_id(0, 0));
                serde_json::json!({
                    "id": stage.stage.to_string(),
                    "stage": stage.stage,
                    "statusCode": status_code,
                    "completed": completed,
                    "claimed": completed && !claimable,
                    "claimable": claimable,
                    "current": stage.stage == current_stage,
                    "cost": cost,
                    "rewards": rewards,
                })
            })
            .collect();
        let feather_balance = balance_text(balances, QIXI_FEATHER_ITEM_ID);
        let sachet_balance = balance_text(balances, QIXI_SACHET_ITEM_ID);
        let received_balance = balance_text(balances, QIXI_RECEIVED_SACHET_ITEM_ID);
        let active = qixi_activity_active(bridge_activity.begin_time, bridge_activity.end_time, server_time);
        let sachet_count = sachet_balance.as_deref().and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        let gift_enabled = active && (balances.is_none() || sachet_count > 0);
        let display_items: Vec<ItemDto> =
            config.display_items.iter().map(activity_item_dto).collect();
        let mut exchange = serde_json::json!({
            "sentItem": item_from_id(0, 0),
            "receivedItem": item_from_id(0, 0),
            "field3": false,
            "enabled": false,
        });
        if let Some(ex) = gift.exchange.as_ref() {
            if let Some(sent) = ex.sent_item.as_ref() {
                exchange["sentItem"] = serde_json::to_value(activity_item_dto(sent)).unwrap_or_default();
            }
            if let Some(received) = ex.received_item.as_ref() {
                exchange["receivedItem"] =
                    serde_json::to_value(activity_item_dto(received)).unwrap_or_default();
            }
            exchange["field3"] = serde_json::json!(ex.field_3);
            exchange["enabled"] = serde_json::json!(ex.enabled);
        }
        let name = if bridge_activity.name.trim().is_empty() {
            "鹊桥寄情".to_string()
        } else {
            bridge_activity.name.clone()
        };
        Ok(serde_json::json!({
            "groupId": QIXI_GROUP_ID.to_string(),
            "bridgeActivityId": QIXI_BRIDGE_ACTIVITY_ID.to_string(),
            "giftActivityId": QIXI_GIFT_ACTIVITY_ID.to_string(),
            "activityId": QIXI_BRIDGE_ACTIVITY_ID.to_string(),
            "name": name,
            "title": name,
            "startTime": bridge_activity.begin_time.to_string(),
            "endTime": bridge_activity.end_time.to_string(),
            "serverTime": server_time.to_string(),
            "active": active,
            "known": true,
            "rules": text_content(&bridge_activity.extra),
            "feather": item_from_id(QIXI_FEATHER_ITEM_ID, feather_balance.as_deref().and_then(|v| v.parse().ok()).unwrap_or(0)),
            "sachet": item_from_id(QIXI_SACHET_ITEM_ID, sachet_count),
            "receivedSachet": item_from_id(QIXI_RECEIVED_SACHET_ITEM_ID, received_balance.as_deref().and_then(|v| v.parse().ok()).unwrap_or(0)),
            "balances": {
                "feather": feather_balance,
                "sachet": sachet_balance,
                "receivedSachet": received_balance,
                "known": balances.is_some(),
            },
            "bridge": {
                "currentStage": current_stage,
                "stages": stages,
                "claimable": bridge_claimable,
                "rewardRedDot": bridge_claimable,
                "displayItems": display_items,
            },
            "gift": {
                "sentCount": gift.sent_count.to_string(),
                "field2Code": gift.field_2.to_string(),
                "field3Code": gift.field_3.to_string(),
                "exchange": exchange,
            },
            "actions": {
                "bridge": {
                    "enabled": active && bridge_claimable,
                    "available": active && bridge_claimable,
                    "availabilityKnown": true
                },
                "gift": {
                    "enabled": gift_enabled,
                    "available": gift_enabled,
                    "availabilityKnown": balances.is_some()
                },
            },
        }))
    }

    async fn query_qixi_group(&self) -> Result<GetGroupReply> {
        let req = GetGroupRequest { group_id: QIXI_GROUP_ID };
        let body = self.gateway.request(ACTIVITY_SERVICE, "GetGroup", &req.encode_to_vec()).await?;
        Ok(GetGroupReply::decode(&body[..])?)
    }

    /// 当前鹊桥寄情。活动不存在时返回业务错误，快照侧忽略。
    pub async fn get_current_qixi_activity(&self) -> Result<serde_json::Value> {
        let reply = self.query_qixi_group().await?;
        let balances = self.qixi_balances().await;
        self.qixi_dto(&reply, balances.as_ref())
    }

    /// 领取当前筑桥阶段奖励。
    pub async fn claim_qixi_bridge_rewards(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let activity = self.get_current_qixi_activity().await?;
        let enabled = activity
            .pointer("/actions/bridge/enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !enabled {
            return Err(qixi_err(ActivityErrorCode::QixiBridgeUnavailable, "当前没有可领取的鹊桥奖励").into());
        }
        let req = ClaimQixiBridgeRewardsRequest {
            activity_id: QIXI_BRIDGE_ACTIVITY_ID,
            operate_type: QIXI_BRIDGE_OPERATE_TYPE,
            params: Some(crate::proto::generated::gamepb::activitypb::claim_qixi_bridge_rewards_request::Params {
                claim_mode: 0,
            }),
        };
        let body =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec()).await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        if reply.activity_id != QIXI_BRIDGE_ACTIVITY_ID || reply.operate_type != QIXI_BRIDGE_OPERATE_TYPE
        {
            return Err(qixi_err(ActivityErrorCode::QixiResponseInvalid, "鹊桥领取回包不匹配").into());
        }
        let mut claimed = Vec::new();
        let mut rewards: Vec<ItemDto> = Vec::new();
        if let Some(result) = reply.qixi_bridge_result.as_ref() {
            claimed = result.claimed_stages.iter().map(ToString::to_string).collect();
            rewards.extend(result.rewards.iter().map(item_dto));
        }
        if rewards.is_empty() {
            rewards.extend(reply.rewards.iter().map(item_dto));
        }
        let message = if claimed.is_empty() {
            "鹊桥奖励领取成功".to_string()
        } else {
            format!("已完成第 {} 阶段鹊桥并领取奖励", claimed.join("、"))
        };
        let snapshot = self.snapshot_with_shop(None).await.ok();
        Ok(serde_json::json!({
            "claimedStages": claimed,
            "rewards": rewards,
            "message": message,
            "snapshot": snapshot,
        }))
    }

    /// 向好友赠送鹊羽香囊。
    pub async fn gift_qixi_sachet(&self, friend_gid: i64, count: i64) -> Result<serde_json::Value> {
        if friend_gid <= 0 {
            return Err(qixi_err(ActivityErrorCode::InvalidQixiFriendGid, "好友 GID 必须是正十进制整数").into());
        }
        if count <= 0 {
            return Err(qixi_err(ActivityErrorCode::InvalidQixiSachetCount, "赠送数量必须是正十进制整数").into());
        }
        let _guard = self.mutation_lock.lock().await;
        let activity = self.get_current_qixi_activity().await?;
        let enabled = activity
            .pointer("/actions/gift/enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !enabled {
            return Err(qixi_err(ActivityErrorCode::QixiGiftUnavailable, "当前无法赠送鹊羽香囊").into());
        }
        let known = activity.pointer("/balances/known").and_then(serde_json::Value::as_bool).unwrap_or(false);
        if known {
            let have = activity
                .pointer("/balances/sachet")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            if have < count {
                return Err(qixi_err(ActivityErrorCode::InsufficientQixiSachet, "鹊羽香囊数量不足").into());
            }
        }
        let req = GiftQixiSachetRequest {
            activity_id: QIXI_GIFT_ACTIVITY_ID,
            operate_type: QIXI_GIFT_OPERATE_TYPE,
            params: Some(crate::proto::generated::gamepb::activitypb::gift_qixi_sachet_request::Params {
                friend_gid,
                count,
            }),
        };
        let body =
            self.gateway.request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec()).await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        if reply.activity_id != QIXI_GIFT_ACTIVITY_ID || reply.operate_type != QIXI_GIFT_OPERATE_TYPE {
            return Err(qixi_err(ActivityErrorCode::QixiResponseInvalid, "赠送香囊回包不匹配").into());
        }
        if reply.qixi_gift_result.as_ref().is_some_and(|r| !r.success) {
            return Err(qixi_err(ActivityErrorCode::QixiGiftFailed, "赠送鹊羽香囊失败").into());
        }
        let snapshot = self.snapshot_with_shop(None).await.ok();
        Ok(serde_json::json!({
            "friendGid": friend_gid.to_string(),
            "count": count.to_string(),
            "message": format!("已赠送 {count} 个鹊羽香囊"),
            "snapshot": snapshot,
        }))
    }
}
