//! 图鉴快照（作物 type=1 / 变异 type=2）。
//!
//! 1:1 对应 bot `core/src/services/illustrated.ts`（同步 0533264 的 v2 字段与
//! 领取奖励）：拉 GetIllustratedListV2 + GetIllustratedLevelListV2，合并等级/进度/
//! 每级奖励/变异分组/超变 buff。自动领取在 task.rs（ClaimAllRewardsV2 type=1），
//! 手动领取走本服务的 [`IllustratedService::claim_rewards`]。

use std::sync::Arc;

use prost::Message as _;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item;
use crate::proto::generated::gamepb::illustratedpb::{
    ClaimAllRewardsV2Reply, ClaimAllRewardsV2Request, GetIllustratedLevelListV2Reply,
    GetIllustratedLevelListV2Request, GetIllustratedListV2Reply, IllustratedInfoV2,
};

const SERVICE: &str = "gamepb.illustratedpb.IllustratedService";

pub struct IllustratedService {
    gateway: Arc<Gateway>,
    /// 账号 id：商店种子列表（shopSeed 富化）读等级快照用
    account_id: String,
    /// 领取串行化（对齐 bot `claimTail`）
    claim_lock: tokio::sync::Mutex<()>,
}

fn reward_dto(reward: Option<&Item>) -> Option<serde_json::Value> {
    let reward = reward?;
    if reward.id == 0 {
        return None;
    }
    let gc = global_game_config();
    let name = gc
        .get_item_by_id(reward.id)
        .map(|i| i.name)
        .unwrap_or_else(|| format!("物品{}", reward.id));
    Some(serde_json::json!({
        "itemId": reward.id,
        "count": reward.count.max(0),
        "name": name,
        "image": gc.get_item_image_by_id(reward.id).unwrap_or_default(),
    }))
}

fn item_dto(item: &IllustratedInfoV2) -> serde_json::Value {
    let gc = global_game_config();
    let seed_id = i64::from(item.fruit_id);
    let name = gc
        .get_item_by_id(seed_id)
        .map(|i| i.name)
        .or_else(|| gc.get_plant_by_seed_id(seed_id).map(|p| p.name))
        .unwrap_or_else(|| format!("种子#{seed_id}"));
    serde_json::json!({
        "seedId": seed_id,
        "name": name,
        "image": gc.get_item_image_by_id(seed_id).unwrap_or_default(),
        "rewardCategory": item.rarity,
        "group": gc.illustrated_mutant_group(seed_id),
        "sort": gc.get_illustrated_by_param(seed_id).map(|e| e.sort).unwrap_or(0),
        "cropCategory": item.layer,
        "unlocked": item.unlocked,
        "progress": item.progress.max(0),
        "isNew": item.is_new,
        "reward": reward_dto(item.reward.first()),
        // v2 协议不再下发 attributes（guarantees 取代），对齐 bot 输出空数组
        "attributes": Vec::<serde_json::Value>::new(),
    })
}

fn buff_dto(entry: &crate::config::game_config::BuffConfigItem) -> serde_json::Value {
    let value = entry.attr_value;
    serde_json::json!({
        "id": entry.id,
        "level": entry.source_param,
        "name": entry.attr_id,
        "value": value,
        "valueType": if value > 10 { "probability" } else { "quantity" },
    })
}

impl IllustratedService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, account_id: String::new(), claim_lock: tokio::sync::Mutex::new(()) }
    }

    /// 注入账号 id（面板快照的 shopSeed 需要按账号等级判断 locked）
    #[must_use]
    pub fn with_account_id(mut self, account_id: &str) -> Self {
        self.account_id = account_id.to_string();
        self
    }

    async fn get_list(&self, kind: i32) -> Result<GetIllustratedListV2Reply> {
        // 对齐抓包：rarity=0 显式编码（field 1 = 0；prost 会省略 proto3 默认值，
        // 与 bot `writer.uint32(8).int32(0); writer.uint32(16).int32(type)` 同构）
        let body = vec![0x08, 0x00, 0x10, kind as u8];
        let body = self.gateway.request(SERVICE, "GetIllustratedListV2", &body).await?;
        Ok(GetIllustratedListV2Reply::decode(&body[..])?)
    }

    async fn get_levels(&self, kind: i32) -> Result<GetIllustratedLevelListV2Reply> {
        let req = GetIllustratedLevelListV2Request { r#type: kind };
        let body = self
            .gateway
            .request(SERVICE, "GetIllustratedLevelListV2", &req.encode_to_vec())
            .await?;
        Ok(GetIllustratedLevelListV2Reply::decode(&body[..])?)
    }

    fn normalize_book(
        &self,
        kind: i32,
        list: &GetIllustratedListV2Reply,
        level_reply: &GetIllustratedLevelListV2Reply,
    ) -> serde_json::Value {
        let gc = global_game_config();
        let levels: Vec<serde_json::Value> = level_reply
            .level_list
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "level": entry.level,
                    "progress": entry.point_condition.max(0),
                    "claimed": entry.claimed,
                    "canClaim": entry.can_claim && !entry.claimed,
                    "rewards": entry.rewards.iter().filter_map(|r| reward_dto(Some(r))).collect::<Vec<_>>(),
                })
            })
            .collect();
        let claimable = level_reply.level_list.iter().any(|v| v.can_claim && !v.claimed);
        let current_level = list.current_level.max(level_reply.current_level);
        let next_level_progress = if list.next_level_progress > 0 {
            list.next_level_progress
        } else {
            level_reply
                .level_list
                .iter()
                .find(|l| l.level > current_level)
                .map(|l| l.point_condition.max(0))
                .unwrap_or(0)
        };
        let mut items: Vec<serde_json::Value> = list.list.iter().map(item_dto).collect();
        items.sort_by_key(|it| it["sort"].as_i64().unwrap_or(0));
        serde_json::json!({
            "type": kind,
            "level": current_level,
            "progress": list.total_progress.max(level_reply.total_progress),
            "nextLevelProgress": next_level_progress,
            "claimable": claimable,
            "rewards": list.total_rewards.iter().filter_map(|r| reward_dto(Some(r))).collect::<Vec<_>>(),
            "buffs": if kind == 2 { gc.illustrated_buffs().iter().map(buff_dto).collect::<Vec<_>>() } else { Vec::new() },
            "currentBuffs": if kind == 2 { gc.illustrated_buffs_by_level(current_level as i64).iter().map(buff_dto).collect::<Vec<_>>() } else { Vec::new() },
            "items": items,
            "levels": levels,
        })
    }

    /// 手动领取图鉴奖励（对齐 bot `claimIllustratedRewards`，0533264）。
    ///
    /// 串行化 → 先确认存在可领 → `ClaimAllRewardsV2(type)` →
    /// `claimed_level_ids` 非空校验 → 附带最新快照；刷新失败时给
    /// `refreshRequired`，不把已成功的写操作报成失败。
    pub async fn claim_rewards(&self, kind: i32) -> Result<serde_json::Value> {
        if kind != 1 && kind != 2 {
            return Err(Error::Business("图鉴类型无效".into()));
        }
        let _guard = self.claim_lock.lock().await;
        let levels = self.get_levels(kind).await?;
        if !levels.level_list.iter().any(|v| v.can_claim && !v.claimed) {
            return Err(Error::Business("暂无可领取奖励".into()));
        }
        let req = ClaimAllRewardsV2Request { r#type: kind };
        let body = self.gateway.request(SERVICE, "ClaimAllRewardsV2", &req.encode_to_vec()).await?;
        let decoded = ClaimAllRewardsV2Reply::decode(&body[..])?;
        if decoded.claimed_level_ids.is_empty() {
            return Err(Error::Business("领取结果未确认，请刷新图鉴后查看".into()));
        }
        let mut result = serde_json::json!({
            "rewards": decoded.total_rewards.iter().filter_map(|r| reward_dto(Some(r))).collect::<Vec<_>>(),
            "claimedLevels": decoded.claimed_level_ids,
        });
        match self.get_snapshot().await {
            Ok(snapshot) => {
                result["snapshot"] = snapshot;
            }
            Err(_) => {
                result["snapshot"] = serde_json::Value::Null;
                result["refreshRequired"] = serde_json::json!(true);
            }
        }
        Ok(result)
    }

    /// 图鉴快照：作物（type=1）+ 变异（type=2）
    pub async fn get_snapshot(&self) -> Result<serde_json::Value> {
        let (crop_list, mutant_list) = tokio::join!(self.get_list(1), self.get_list(2));
        let crop_list = crop_list?;
        let mutant_list = mutant_list?;
        let (crop_levels, mutant_levels) = tokio::join!(self.get_levels(1), self.get_levels(2));
        let crop_levels = crop_levels?;
        let mutant_levels = mutant_levels?;
        let mut crop = self.normalize_book(1, &crop_list, &crop_levels);
        self.attach_shop_seeds(&mut crop).await;
        Ok(serde_json::json!({
            "crop": crop,
            "mutant": self.normalize_book(2, &mutant_list, &mutant_levels),
            "updatedAt": crate::utils::time::now_ms(),
        }))
    }

    /// 作物条目挂商店种子购买信息（对齐 bot `buildIllustratedSnapshot`，0f09702）：
    /// seedId 归一为植物种子 id，可购买的挂完整 shopSeed，仅本地目录可查的挂
    /// `unknownMeta` 版本（goodsId=0），都没有则 null。
    async fn attach_shop_seeds(&self, crop: &mut serde_json::Value) {
        let gc = global_game_config();
        let seeds = crate::services::farm::planting::fetch_available_seeds(
            self.gateway.clone(),
            &self.account_id,
            false,
        )
        .await
        .unwrap_or_default();
        let seed_by_id: std::collections::HashMap<i64, serde_json::Value> = seeds
            .iter()
            .filter_map(|seed| {
                seed.get("seedId").and_then(serde_json::Value::as_i64).map(|id| (id, seed.clone()))
            })
            .collect();
        let local_seeds: std::collections::HashMap<i64, serde_json::Value> = gc
            .get_all_seeds()
            .into_iter()
            .filter_map(|s| serde_json::to_value(&s).ok().map(|v| (s.seed_id, v)))
            .collect();
        let Some(items) = crop.get_mut("items").and_then(serde_json::Value::as_array_mut) else {
            return;
        };
        for item in items.iter_mut() {
            let Some(obj) = item.as_object_mut() else { continue };
            let item_id = obj.get("seedId").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let plant =
                gc.get_plant_by_seed_id(item_id).or_else(|| gc.get_plant_by_fruit_id(item_id));
            let seed_id =
                plant.as_ref().and_then(|p| p.seed_id).filter(|id| *id > 0).unwrap_or(item_id);
            let shop_seed = match seed_by_id.get(&seed_id) {
                Some(shop) => shop.clone(),
                None => match local_seeds.get(&seed_id) {
                    Some(local) => {
                        let mut value = local.clone();
                        if let Some(local_obj) = value.as_object_mut() {
                            local_obj.insert("goodsId".into(), serde_json::json!(0));
                            local_obj.insert("price".into(), serde_json::Value::Null);
                            local_obj.insert("unknownMeta".into(), serde_json::json!(true));
                            local_obj.insert("requiresUnlockCard".into(), serde_json::json!(false));
                        }
                        value
                    }
                    None => serde_json::Value::Null,
                },
            };
            obj.insert("seedId".into(), serde_json::json!(seed_id));
            obj.insert("shopSeed".into(), shop_seed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutant_group_by_param() {
        crate::config::game_config::global().load();
        let gc = crate::config::game_config::global();
        // 40002001 是普通果实 → gold
        assert_eq!(gc.illustrated_mutant_group(40002001), "gold");
    }

    #[test]
    fn item_dto_maps_fields() {
        crate::config::game_config::global().load();
        let item = IllustratedInfoV2 {
            fruit_id: 40002001,
            rarity: 2,
            unlocked: true,
            progress: 40,
            layer: 1,
            reward: vec![Item { id: 1002, count: 40, ..Default::default() }],
            is_new: false,
            guarantees: vec![],
        };
        let dto = item_dto(&item);
        assert_eq!(dto["seedId"].as_i64(), Some(40002001));
        assert_eq!(dto["unlocked"].as_bool(), Some(true));
        assert_eq!(dto["rewardCategory"].as_i64(), Some(2));
        assert_eq!(dto["cropCategory"].as_i64(), Some(1));
        assert_eq!(dto["reward"]["itemId"].as_i64(), Some(1002));
        assert_eq!(dto["reward"]["count"].as_i64(), Some(40));
        assert!(dto["attributes"].as_array().is_some_and(Vec::is_empty));
    }

    /// 领取入参校验：只允许 1/2（在触网前拒绝），对齐 bot 图鉴测试.
    #[tokio::test]
    async fn claim_rejects_invalid_type_before_network() {
        let gateway = Arc::new(Gateway::new(
            crate::network::gateway::GatewayConfig {
                server_url: "wss://gate.example.com/ws".into(),
                platform: "qq".into(),
                os: "Windows".into(),
                client_version: "1.15.0.2_20261001".into(),
                auth_code: "test".into(),
                headers: std::collections::HashMap::new(),
            },
            Arc::new(crate::network::encryptor::NoopEncryptor),
        ));
        let service = IllustratedService::new(gateway);
        let error = service.claim_rewards(3).await.expect_err("类型必须被拒绝");
        assert!(error.to_string().contains("图鉴类型无效"), "{error}");
    }
}
