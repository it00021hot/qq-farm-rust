use std::collections::HashSet;

use prost::Message;

use crate::constants::{
    ACTIVITY_SERVICE, CLAIM_QINGMEI_SEED_OPERATE_TYPE, CONTINUE_QINGMEI_BREW_OPERATE_TYPE,
    QINGMEI_BREW_ACTIVITY_ID, QINGMEI_DAILY_ACTIVITY_ID, QINGMEI_DAILY_ALREADY_CLAIMED_CODE,
    QINGMEI_DAILY_GRANT_ID, QINGMEI_ITEM_ID, QINGMEI_SHARED_SETTLEMENT_MODE, QINGMEI_SHARE_SCENE,
    QINGMEI_SHARE_SOURCE, QUERY_QINGMEI_OPERATE_TYPE, SELL_QINGMEI_BREW_OPERATE_TYPE,
    START_QINGMEI_BREW_OPERATE_TYPE,
};
use crate::error::Result;
use crate::proto::generated::gamepb::activitypb::{
    ActivityOperateReply, ClaimQingMeiDailySeedRequest, ContinueQingMeiBrewRequest,
    QueryActivityRequest, SettleQingMeiBrewRequest, StartQingMeiBrewRequest,
};

use super::dto::{
    activity_item_dto, force_qingmei_seed_claimed_in_snapshot, is_qingmei_already_claimed_message,
    item_dto, item_from_id, json_i64, json_positive_decimal, json_text, text_content,
};
use super::error::{ActivityError, ActivityErrorCode};
use super::ActivityCenterService;
use super::{ItemDto, QingMeiDto};
use crate::services::warehouse::WarehouseService;

impl ActivityCenterService {
    // ----- 青梅 -----

    async fn operate_qingmei(
        &self,
        body: Vec<u8>,
        expected_error_codes: &[i64],
    ) -> Result<(ActivityOperateReply, bool)> {
        match self.gateway.request(ACTIVITY_SERVICE, "Operate", &body).await {
            Ok(bytes) => Ok((ActivityOperateReply::decode(&bytes[..])?, false)),
            Err(crate::network::error::NetworkError::Gateway { code, error_message, .. })
                if expected_error_codes.contains(&code)
                    || is_qingmei_already_claimed_message(&error_message) =>
            {
                Ok((ActivityOperateReply::default(), true))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn query_qingmei_reply(&self) -> Result<ActivityOperateReply> {
        let req = QueryActivityRequest {
            activity_id: QINGMEI_BREW_ACTIVITY_ID,
            operate_type: QUERY_QINGMEI_OPERATE_TYPE,
        };
        self.operate_qingmei(req.encode_to_vec(), &[]).await.map(|(r, _)| r)
    }

    /// 当前青梅活动
    pub async fn get_current_qingmei_activity(&self) -> Result<QingMeiDto> {
        let reply = self.query_qingmei_reply().await?;
        let ingredients = self.qingmei_ingredients().await.ok();
        Ok(self.qingmei_dto(&reply, ingredients.as_deref()))
    }

    async fn qingmei_ingredients(&self) -> Result<Vec<serde_json::Value>> {
        let warehouse = self.warehouse.lock().clone();
        let bag = if let Some(wh) = warehouse {
            wh.get_bag().await?
        } else {
            WarehouseService::get_bag_via(&self.gateway).await?
        };
        let items = bag.item_bag.as_ref().map(|b| b.items.as_slice()).unwrap_or(&[]);
        Ok(items
            .iter()
            .filter(|i| i.id == QINGMEI_ITEM_ID && i.count > 0)
            .map(|i| {
                let mutant_types: Vec<String> =
                    i.mutant_types.iter().filter(|t| **t != 0).map(ToString::to_string).collect();
                let uid = i.uid.to_string();
                let mut dto = serde_json::to_value(item_from_id(i.id, i.count))
                    .unwrap_or_else(|_| serde_json::json!({}));
                if let Some(obj) = dto.as_object_mut() {
                    obj.insert("uid".into(), serde_json::json!(uid.clone()));
                    obj.insert("mutantTypes".into(), serde_json::json!(mutant_types.clone()));
                    obj.insert(
                        "key".into(),
                        serde_json::json!(format!("{}:{}", uid, mutant_types.join(","))),
                    );
                }
                dto
            })
            .collect())
    }

    fn qingmei_dto(
        &self,
        reply: &ActivityOperateReply,
        ingredients: Option<&[serde_json::Value]>,
    ) -> QingMeiDto {
        let activity = reply.data.as_ref().and_then(|d| d.activity.as_ref());
        let brew =
            reply.data.as_ref().and_then(|d| d.qingmei_brew.as_ref()).cloned().unwrap_or_default();
        let quote = reply
            .qingmei_quote
            .clone()
            .or_else(|| reply.data.as_ref().and_then(|d| d.qingmei_quote.clone()));
        let daily_seed = reply.data.as_ref().and_then(|d| d.qingmei_daily_seed.as_ref());
        let current_round = brew.current_round;
        let started = brew.base_gold > 0;
        let max_rounds = brew.max_rounds.max(1);
        let claimed_today =
            self.qingmei_seed_claimed_today() || daily_seed.map(|d| d.claimed).unwrap_or(false);
        let balance: i64 =
            ingredients.unwrap_or(&[]).iter().filter_map(|v| json_i64(v.get("count"))).sum();
        let activity_id = activity
            .map(|a| a.activity_id)
            .filter(|id| *id != 0)
            .unwrap_or(QINGMEI_BREW_ACTIVITY_ID);
        QingMeiDto {
            activity_id: activity_id.to_string(),
            daily_activity_id: QINGMEI_DAILY_ACTIVITY_ID.to_string(),
            name: activity
                .map(|a| a.name.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "青酿换万金".to_string()),
            start_time: activity.map(|a| a.begin_time).unwrap_or(0).to_string(),
            end_time: activity.map(|a| a.end_time).unwrap_or(0).to_string(),
            rules: activity
                .map(|a| text_content(&a.extra))
                .unwrap_or_else(|| serde_json::json!({ "title": "", "paragraphs": [] })),
            ingredient: ItemDto {
                name: "青梅".to_string(),
                ..item_from_id(QINGMEI_ITEM_ID, balance)
            },
            ingredients: ingredients.unwrap_or(&[]).to_vec(),
            balance: balance.to_string(),
            balance_known: ingredients.is_some(),
            base_gold: brew.base_gold.to_string(),
            base_price: brew.base_price.to_string(),
            guaranteed_price: brew.guaranteed_price.to_string(),
            current_round,
            started,
            max_rounds,
            finished: brew.finished,
            quote_prices: brew.quote_prices.iter().map(ToString::to_string).collect(),
            quote_totals: brew.quote_totals.iter().map(ToString::to_string).collect(),
            quote: quote.map(|q| {
                serde_json::json!({
                    "round": q.round,
                    "unitPrice": q.unit_price.to_string(),
                    "totalGold": q.total_gold.to_string(),
                    "doubled": q.doubled,
                })
            }),
            daily_seed: serde_json::json!({
                "claimed": claimed_today,
                "grantId": daily_seed
                    .and_then(|d| d.grant.as_ref())
                    .map(|g| g.grant_id.to_string())
                    .unwrap_or_else(|| QINGMEI_DAILY_GRANT_ID.to_string()),
                "reward": daily_seed
                    .and_then(|d| d.grant.as_ref())
                    .and_then(|g| g.item.as_ref())
                    .map(activity_item_dto),
            }),
            actions: serde_json::json!({
                "claimSeed": { "enabled": !claimed_today, "available": !claimed_today },
                "start": {
                    "enabled": ingredients.map(|i| !i.is_empty()).unwrap_or(true),
                    "available": ingredients.map(|i| !i.is_empty()).unwrap_or(true)
                },
                "continue": {
                    "enabled": current_round < max_rounds && !brew.finished && brew.base_gold > 0,
                    "available": current_round < max_rounds && !brew.finished && brew.base_gold > 0
                },
                "settle": {
                    "enabled": !brew.quote_totals.is_empty() || brew.finished,
                    "available": !brew.quote_totals.is_empty() || brew.finished
                },
            }),
        }
    }

    /// 领取青梅每日种子
    pub async fn claim_qingmei_daily_seed(&self) -> Result<serde_json::Value> {
        // 本地已记今日已领：直接幂等成功，避免再打 RPC 报错而按钮仍可点
        if self.qingmei_seed_claimed_today() {
            let mut snapshot = self.snapshot_with_shop(None).await.ok();
            force_qingmei_seed_claimed_in_snapshot(&mut snapshot);
            return Ok(serde_json::json!({
                "rewards": [],
                "message": "今日青梅种子已经领取，无需重复领取",
                "snapshot": snapshot,
            }));
        }

        let (reply, already) = {
            let _guard = self.mutation_lock.lock().await;
            if self.qingmei_seed_claimed_today() {
                (ActivityOperateReply::default(), true)
            } else {
                let req = ClaimQingMeiDailySeedRequest {
                    activity_id: QINGMEI_DAILY_ACTIVITY_ID,
                    operate_type: CLAIM_QINGMEI_SEED_OPERATE_TYPE,
                    params: Some(
                        crate::proto::generated::gamepb::activitypb::claim_qing_mei_daily_seed_request::Params {
                            grant_id: QINGMEI_DAILY_GRANT_ID,
                        },
                    ),
                };
                match self
                    .operate_qingmei(req.encode_to_vec(), &[QINGMEI_DAILY_ALREADY_CLAIMED_CODE])
                    .await
                {
                    Ok(v) => v,
                    Err(e) if is_qingmei_already_claimed_message(&e.to_string()) => {
                        (ActivityOperateReply::default(), true)
                    }
                    Err(e) => return Err(e),
                }
            }
        };
        // 无论新领还是 1034014，都标记今日已领（对齐 bot 写 qingMeiSeedClaimedDateKey）
        self.mark_qingmei_seed_claimed_today();
        let rewards: Vec<ItemDto> = reply.rewards.iter().map(item_dto).collect();
        // 释放 mutation_lock 后再拉快照，避免长查询占锁；并强制 dailySeed.claimed=true
        let mut snapshot = self.snapshot_with_shop(None).await.ok();
        force_qingmei_seed_claimed_in_snapshot(&mut snapshot);
        Ok(serde_json::json!({
            "rewards": rewards,
            "message": if already {
                "今日青梅种子已经领取，无需重复领取"
            } else {
                "青梅种子领取成功"
            },
            "snapshot": snapshot,
        }))
    }

    /// 开始青梅酿造
    pub async fn start_qingmei_brew(&self, input: serde_json::Value) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let candidates = self.qingmei_ingredients().await.unwrap_or_default();
        let requested: Vec<serde_json::Value> = if let Some(arr) = input.as_array() {
            arr.clone()
        } else {
            let count = json_positive_decimal(
                input.get("count").unwrap_or(&input),
                ActivityErrorCode::InvalidQingmeiCount,
                "count",
            )?;
            let candidate =
                candidates.iter().find(|c| json_i64(c.get("count")).unwrap_or(0) >= count);
            vec![serde_json::json!({
                "uid": candidate.and_then(|c| c.get("uid").cloned()).unwrap_or(serde_json::Value::Null),
                "count": count,
            })]
        };
        if requested.is_empty() {
            return Err(ActivityError {
                code: ActivityErrorCode::InvalidQingmeiIngredients,
                message: "至少选择一组青梅".to_string(),
            }
            .into());
        }
        let mut seen_uids = HashSet::new();
        let mut ingredients = Vec::new();
        for entry in &requested {
            let uid = json_positive_decimal(
                entry.get("uid").unwrap_or(&serde_json::Value::Null),
                ActivityErrorCode::InvalidQingmeiUid,
                "uid",
            )?;
            let count = json_positive_decimal(
                entry.get("count").unwrap_or(&serde_json::Value::Null),
                ActivityErrorCode::InvalidQingmeiCount,
                "count",
            )?;
            let uid_key = uid.to_string();
            if !seen_uids.insert(uid_key.clone()) {
                return Err(ActivityError {
                    code: ActivityErrorCode::DuplicateQingmeiUid,
                    message: format!("青梅 UID {uid} 重复"),
                }
                .into());
            }
            let candidate = candidates.iter().find(|c| json_text(c.get("uid")) == uid_key);
            let available = candidate.and_then(|c| json_i64(c.get("count"))).unwrap_or(0);
            if candidate.is_none() || available < count {
                return Err(ActivityError {
                    code: ActivityErrorCode::InsufficientQingmei,
                    message: format!("青梅 UID {uid} 数量不足"),
                }
                .into());
            }
            ingredients.push(
                crate::proto::generated::gamepb::activitypb::start_qing_mei_brew_request::Ingredient {
                    uid,
                    count,
                },
            );
        }
        let total: i64 = ingredients.iter().map(|i| i.count).sum();
        let req = StartQingMeiBrewRequest {
            activity_id: QINGMEI_BREW_ACTIVITY_ID,
            operate_type: START_QINGMEI_BREW_OPERATE_TYPE,
            params: Some(
                crate::proto::generated::gamepb::activitypb::start_qing_mei_brew_request::Params {
                    ingredients,
                },
            ),
        };
        let (reply, _) = self.operate_qingmei(req.encode_to_vec(), &[]).await?;
        Ok(serde_json::json!({
            "activity": self.qingmei_dto(&reply, None),
            "message": format!("已投入 {total} 个青梅开始酿造"),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }

    /// 继续青梅酿造
    pub async fn continue_qingmei_brew(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let req = ContinueQingMeiBrewRequest {
            activity_id: QINGMEI_BREW_ACTIVITY_ID,
            operate_type: CONTINUE_QINGMEI_BREW_OPERATE_TYPE,
            params: Some(
                crate::proto::generated::gamepb::activitypb::continue_qing_mei_brew_request::Empty {},
            ),
        };
        let (reply, _) = self.operate_qingmei(req.encode_to_vec(), &[]).await?;
        Ok(serde_json::json!({
            "activity": self.qingmei_dto(&reply, None),
            "quote": reply.qingmei_quote.as_ref().map(|q| serde_json::json!({
                "round": q.round,
                "unitPrice": q.unit_price.to_string(),
                "totalGold": q.total_gold.to_string(),
                "doubled": q.doubled,
            })),
            "message": reply.qingmei_quote.as_ref().map(|q| {
                format!("第 {} 轮报价：{} 金币", q.round, q.total_gold)
            }).unwrap_or_else(|| "酿造进度已更新".to_string()),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }

    /// 结算青梅酿造
    pub async fn settle_qingmei_brew(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        crate::services::share::ShareService::new(self.gateway.clone())
            .report_activity_share(QINGMEI_SHARE_SOURCE, QINGMEI_SHARE_SCENE)
            .await?;
        let req = SettleQingMeiBrewRequest {
            activity_id: QINGMEI_BREW_ACTIVITY_ID,
            operate_type: SELL_QINGMEI_BREW_OPERATE_TYPE,
            params: Some(
                crate::proto::generated::gamepb::activitypb::settle_qing_mei_brew_request::Params {
                    settlement_mode: QINGMEI_SHARED_SETTLEMENT_MODE,
                },
            ),
        };
        let (reply, _) = self.operate_qingmei(req.encode_to_vec(), &[]).await?;
        let settlement = reply.qingmei_settlement.as_ref();
        let rewards: Vec<ItemDto> = if let Some(s) = settlement {
            s.reward.as_ref().map(item_dto).into_iter().collect()
        } else {
            reply.rewards.iter().map(item_dto).collect()
        };
        let total_gold = settlement.map(|s| s.total_gold).unwrap_or(0);
        Ok(serde_json::json!({
            "rewards": rewards,
            "settlement": {
                "mode": settlement.map(|s| s.settlement_mode).unwrap_or(QINGMEI_SHARED_SETTLEMENT_MODE),
                "totalGold": total_gold.to_string(),
            },
            "message": format!("分享出售成功（1.5倍），获得 {total_gold} 金币"),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }
}
