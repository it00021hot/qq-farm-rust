//! 蓝晶藏品宝典（bot `collections.ts`）。
//!
//! 读取 GetCollections，按系列配置合并星级、积分和可领里程碑。领取只确认积分回包。

use std::sync::{Arc, OnceLock};

use prost::Message as _;
use serde_json::{json, Value};

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::collectionpb::{
    CategoryDetail, ClaimCollectionRewardReply, ClaimCollectionRewardRequest, DisplayInfo,
    DisplaySlot, GetCollectionsReply, GetCollectionsRequest, SwitchSceneDisplayReply,
    SwitchSceneDisplayRequest, UpgradeCollectionReply, UpgradeCollectionRequest,
};

const SERVICE: &str = "gamepb.collectionpb.CollectionService";
const CATEGORY: i32 = 1;
const RARITY_NAMES: [&str; 5] = ["", "普通", "稀有", "珍品", "天工"];

pub struct CollectionService {
    gateway: Arc<Gateway>,
}

impl CollectionService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 藏品宝典快照。
    pub async fn snapshot(&self) -> Result<Value> {
        normalize(&self.read().await?)
    }

    /// 领取已达到的积分奖励。没有可领项时不发请求。
    pub async fn claim(&self) -> Result<Value> {
        let mut raw = self.read().await?;
        let before = normalize(&raw)?;
        let eligible = milestone_scores(&before, true);
        if eligible.is_empty() {
            return Ok(json!({ "snapshot": before, "skipped": true }));
        }
        let body = self
            .gateway
            .request(
                SERVICE,
                "ClaimCollectionReward",
                &ClaimCollectionRewardRequest { category: CATEGORY }.encode_to_vec(),
            )
            .await?;
        let reply = ClaimCollectionRewardReply::decode(body.as_slice())?;
        let last = eligible[eligible.len() - 1];
        let score = before.get("score").and_then(Value::as_i64).unwrap_or(0);
        if !claim_confirmed(
            score,
            last,
            i64::from(reply.current_score),
            i64::from(reply.claimed_reward_score),
        ) {
            return Err(Error::Business("藏品操作结果无法确认，请刷新核对，不要重复提交".into()));
        }
        if let Some(detail) = raw.details.iter_mut().find(|detail| detail.category == CATEGORY) {
            detail.current_score = reply.current_score;
            detail.claimed_reward_score = reply.claimed_reward_score;
        }
        let rewards = reply
            .total_rewards
            .iter()
            .map(|item| {
                let name = global_game_config()
                    .get_item_by_id(item.id)
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| format!("物品{}", item.id));
                json!({ "id": item.id, "name": name, "count": item.count })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "snapshot": normalize(&raw)?,
            "claimedScores": reply.claimed_scores,
            "rewards": rewards,
        }))
    }

    /// 激活或升星。材料、星级和积分回包必须与升星前一致。
    pub async fn upgrade(&self, input: &Value) -> Result<Value> {
        let id = collection_id(input.get("collectionId"))?;
        let level = input
            .get("expectedStarLevel")
            .and_then(Value::as_i64)
            .filter(|level| (0..5).contains(level))
            .ok_or_else(|| Error::Business("请提供当前藏品星级（0–4）".into()))?;
        let mut raw = self.read().await?;
        let snapshot = normalize(&raw)?;
        let (rarity, item) =
            find_item(&snapshot, id).ok_or_else(|| Error::Business("藏品编号无效".into()))?;
        let star = item.get("starLevel").and_then(Value::as_i64).unwrap_or(0);
        let own = item.get("ownCount").and_then(Value::as_i64).unwrap_or(0);
        if star != level {
            return Err(Error::Business("藏品星级已变化，请刷新后重新确认".into()));
        }
        if item.get("unlocked").and_then(Value::as_bool) != Some(true) {
            return Err(Error::Business("尚未满足藏品解锁条件".into()));
        }
        let costs = cost_list(item.get("nextStarCosts"));
        if costs.is_empty() {
            return Err(Error::Business("升星材料无法确认".into()));
        }
        if let Some(expected) = input.get("expectedCosts") {
            let shown = cost_list(Some(expected));
            if cost_signature(&shown) != cost_signature(&costs) {
                return Err(Error::Business("激活或升星材料已变化，请重新确认".into()));
            }
        }
        let copies = costs
            .iter()
            .find(|cost| cost.id == id)
            .map(|cost| cost.count)
            .ok_or_else(|| Error::Business("升星材料无法确认".into()))?;
        let mut before = Vec::new();
        for cost in &costs {
            let balance = if cost.id == id {
                Balance { total: own, available: own }
            } else {
                self.bag_balance(cost.id).await?
            };
            if balance.available < cost.count {
                return Err(Error::Business(format!("{}不足，需要 {}", cost.name, cost.count)));
            }
            before.push(balance);
        }
        let body = self
            .gateway
            .request(
                SERVICE,
                "UpgradeCollection",
                &UpgradeCollectionRequest { category: CATEGORY, collection_id: id }.encode_to_vec(),
            )
            .await?;
        let reply = UpgradeCollectionReply::decode(body.as_slice())?;
        let old_score = item.get("score").and_then(Value::as_i64).unwrap_or(0);
        let book_score = snapshot.get("score").and_then(Value::as_i64).unwrap_or(0);
        if !upgrade_confirmed(
            reply.category,
            reply.collection_id,
            i64::from(reply.star_level),
            reply.own_count,
            i64::from(reply.current_score),
            id,
            star,
            own,
            copies,
            book_score,
            old_score,
            star_score(star + 1, rarity),
        ) {
            return Err(Error::Business("藏品操作结果无法确认，请刷新核对，不要重复提交".into()));
        }
        for (cost, earlier) in costs.iter().zip(before.iter()) {
            if cost.id == id {
                continue;
            }
            let after = self.bag_balance(cost.id).await?;
            if after.total != earlier.total - cost.count {
                return Err(Error::Business(
                    "藏品操作结果无法确认，请刷新核对，不要重复提交".into(),
                ));
            }
        }
        if let Some(detail) = raw.details.iter_mut().find(|detail| detail.category == CATEGORY) {
            if let Some(owned) =
                detail.collections.iter_mut().find(|owned| owned.collection_id == id)
            {
                owned.star_level = reply.star_level;
                owned.own_count = reply.own_count;
            } else {
                detail.collections.push(
                    crate::proto::generated::gamepb::collectionpb::CollectionInfo {
                        collection_id: id,
                        star_level: reply.star_level,
                        own_count: reply.own_count,
                        ..Default::default()
                    },
                );
            }
            detail.current_score = reply.current_score;
        }
        Ok(json!({ "snapshot": normalize(&raw)? }))
    }

    /// 把已激活的藏品放上或撤下珍藏室。
    pub async fn display(&self, input: &Value) -> Result<Value> {
        let id = collection_id(input.get("collectionId"))?;
        let Some(displayed) = input.get("displayed").and_then(Value::as_bool) else {
            return Err(Error::Business("请指定展示或取消展示".into()));
        };
        let mut raw = self.read().await?;
        let snapshot = normalize(&raw)?;
        let (_, item) =
            find_item(&snapshot, id).ok_or_else(|| Error::Business("藏品编号无效".into()))?;
        if item.get("activated").and_then(Value::as_bool) != Some(true) {
            return Err(Error::Business("请先激活藏品".into()));
        }
        let current = snapshot.pointer("/display/itemId").and_then(Value::as_i64).unwrap_or(0);
        if let Some(expected) = input.get("expectedDisplayItemId").and_then(Value::as_i64) {
            if expected != current {
                return Err(Error::Business("展示藏品已变化，请刷新后重新确认".into()));
            }
        }
        let showing = current == id;
        if displayed == showing {
            return Ok(json!({ "snapshot": snapshot, "skipped": true }));
        }
        let target = if displayed { id } else { 0 };
        let body = self
            .gateway
            .request(
                SERVICE,
                "SwitchSceneDisplay",
                &SwitchSceneDisplayRequest {
                    display_slot: Some(DisplaySlot {
                        slot_type: 1,
                        display: Some(DisplayInfo {
                            category: CATEGORY,
                            item_id: target,
                            pedestal_id: 0,
                        }),
                    }),
                }
                .encode_to_vec(),
            )
            .await?;
        let reply = SwitchSceneDisplayReply::decode(body.as_slice())?;
        let slot = reply.display_slot.as_ref();
        let shown = slot.and_then(|slot| slot.display.as_ref());
        if !display_confirmed(
            slot.map(|slot| slot.slot_type).unwrap_or(0),
            shown.map(|info| info.category).unwrap_or(0),
            shown.map(|info| info.item_id).unwrap_or(-1),
            target,
        ) {
            return Err(Error::Business("藏品操作结果无法确认，请刷新核对，不要重复提交".into()));
        }
        raw.display_slots.retain(|slot| slot.slot_type != 1);
        if let Some(slot) = reply.display_slot {
            raw.display_slots.push(slot);
        }
        Ok(json!({ "snapshot": normalize(&raw)? }))
    }

    async fn read(&self) -> Result<GetCollectionsReply> {
        let body = self
            .gateway
            .request(
                SERVICE,
                "GetCollections",
                &GetCollectionsRequest { categories: vec![CATEGORY] }.encode_to_vec(),
            )
            .await?;
        Ok(GetCollectionsReply::decode(body.as_slice())?)
    }

    async fn bag_balance(&self, item_id: i64) -> Result<Balance> {
        let bag = crate::services::warehouse::WarehouseService::get_bag_via(&self.gateway).await?;
        let mut total = 0_i64;
        let mut locked = 0_i64;
        for item in crate::services::warehouse::get_bag_items(&bag) {
            if item.id != item_id {
                continue;
            }
            if item.count < 0 {
                return Err(Error::Business("物品余额无法确认".into()));
            }
            total = total
                .checked_add(item.count)
                .ok_or_else(|| Error::Business("物品余额无法确认".into()))?;
            if item.locked {
                locked = locked
                    .checked_add(item.count)
                    .ok_or_else(|| Error::Business("物品余额无法确认".into()))?;
            }
        }
        Ok(Balance { total, available: total - locked })
    }
}

struct Balance {
    total: i64,
    available: i64,
}

struct Material {
    id: i64,
    count: i64,
    name: String,
}

fn collection_id(value: Option<&Value>) -> Result<i64> {
    let id = match value {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(0),
        Some(Value::String(text)) => text.parse().unwrap_or(0),
        _ => 0,
    };
    if table("item").iter().any(|row| field_i64(row, "item_id") == id) {
        Ok(id)
    } else {
        Err(Error::Business("藏品编号无效".into()))
    }
}

fn find_item(snapshot: &Value, id: i64) -> Option<(i64, &Value)> {
    snapshot.get("series").and_then(Value::as_array)?.iter().find_map(|series| {
        let rarity = series.get("rarity").and_then(Value::as_i64).unwrap_or(0);
        series
            .get("items")
            .and_then(Value::as_array)?
            .iter()
            .find(|item| item.get("id").and_then(Value::as_i64) == Some(id))
            .map(|item| (rarity, item))
    })
}

fn cost_list(value: Option<&Value>) -> Vec<Material> {
    value
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some(Material {
                        id: row.get("itemId").and_then(Value::as_i64)?,
                        count: row.get("count").and_then(Value::as_i64).unwrap_or(0),
                        name: row.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn cost_signature(costs: &[Material]) -> String {
    let mut parts =
        costs.iter().map(|cost| format!("{}:{}", cost.id, cost.count)).collect::<Vec<_>>();
    parts.sort();
    parts.join("|")
}

fn upgrade_confirmed(
    category: i32,
    reply_id: i64,
    reply_star: i64,
    reply_own: i64,
    reply_score: i64,
    item_id: i64,
    star: i64,
    own: i64,
    copies: i64,
    before_score: i64,
    old_score: i64,
    new_score: i64,
) -> bool {
    category == CATEGORY
        && reply_id == item_id
        && reply_star == star + 1
        && reply_own == own - copies
        && reply_score == before_score - old_score + new_score
}

fn display_confirmed(slot_type: i32, category: i32, item_id: i64, target: i64) -> bool {
    slot_type == 1 && category == CATEGORY && item_id == target
}

fn claim_confirmed(before_score: i64, eligible_last: i64, reply_score: i64, claimed: i64) -> bool {
    reply_score >= before_score && claimed >= eligible_last && claimed <= reply_score
}

fn milestone_scores(snapshot: &Value, unclaimed: bool) -> Vec<i64> {
    snapshot
        .get("milestones")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| {
                    row.get("reached").and_then(Value::as_bool) == Some(true)
                        && (!unclaimed || row.get("claimed").and_then(Value::as_bool) != Some(true))
                })
                .filter_map(|row| row.get("score").and_then(Value::as_i64))
                .collect()
        })
        .unwrap_or_default()
}

fn normalize(reply: &GetCollectionsReply) -> Result<Value> {
    let detail = reply
        .details
        .iter()
        .find(|detail| detail.category == CATEGORY)
        .ok_or_else(|| Error::Business("当前账号未返回蓝晶藏品信息".into()))?;
    let display = reply.display_slots.iter().find_map(|slot| {
        let display = slot.display.as_ref()?;
        (slot.slot_type == 1 && display.category == CATEGORY && display.item_id > 0)
            .then_some(display)
    });
    let series = series_rows(detail, display.map(|item| item.item_id).unwrap_or(0));
    let activated = series.iter().map(|row| row.activated).sum::<i64>();
    let total = series.iter().map(|row| row.total).sum::<i64>();
    let milestones =
        milestone_rows(i64::from(detail.current_score), i64::from(detail.claimed_reward_score));
    let next = milestones.iter().find(|row| !row.claimed).map(|row| row.score);
    Ok(json!({
        "category": CATEGORY,
        "score": detail.current_score,
        "claimedScore": detail.claimed_reward_score,
        "activatedCount": activated,
        "totalCount": total,
        "nextRewardScore": next,
        "canClaim": milestones.iter().any(|row| row.reached && !row.claimed),
        "milestones": milestones.iter().map(Milestone::json).collect::<Vec<_>>(),
        "series": series.iter().map(SeriesView::json).collect::<Vec<_>>(),
        "display": display.map(|item| json!({ "itemId": item.item_id, "pedestalId": item.pedestal_id })),
    }))
}

struct SeriesView {
    id: i64,
    name: String,
    rarity: i64,
    rarity_name: String,
    activated: i64,
    total: i64,
    items: Vec<Value>,
}

impl SeriesView {
    fn json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "rarity": self.rarity,
            "rarityName": self.rarity_name,
            "activatedCount": self.activated,
            "totalCount": self.total,
            "items": self.items,
        })
    }
}

struct Milestone {
    level: i64,
    score: i64,
    claimed: bool,
    reached: bool,
    buffs: Vec<Value>,
}

impl Milestone {
    fn json(&self) -> Value {
        json!({
            "level": self.level,
            "score": self.score,
            "claimed": self.claimed,
            "reached": self.reached,
            "buffs": self.buffs,
        })
    }
}

fn series_rows(detail: &CategoryDetail, displayed: i64) -> Vec<SeriesView> {
    let mut rows = table("series")
        .iter()
        .filter(|row| category_of(row) == i64::from(CATEGORY))
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| field_i64(row, "display_order"));
    rows.into_iter()
        .map(|series| {
            let series_id = field_i64(&series, "series_id");
            let rarity = field_i64(&series, "series_rarity");
            let mut items = table("item")
                .iter()
                .filter(|item| field_i64(item, "series_id") == series_id)
                .cloned()
                .collect::<Vec<_>>();
            items.sort_by_key(|item| field_i64(item, "display_order"));
            let items = items
                .iter()
                .map(|item| item_row(item, detail, &series, displayed))
                .collect::<Vec<_>>();
            let activated = items
                .iter()
                .filter(|item| item.get("activated").and_then(Value::as_bool) == Some(true))
                .count() as i64;
            SeriesView {
                id: series_id,
                name: field_str(&series, "series_name").to_owned(),
                rarity,
                rarity_name: RARITY_NAMES
                    .get(usize::try_from(rarity).unwrap_or(0))
                    .copied()
                    .unwrap_or("")
                    .to_owned(),
                activated,
                total: items.len() as i64,
                items,
            }
        })
        .collect()
}

fn item_row(item: &Value, detail: &CategoryDetail, series: &Value, displayed: i64) -> Value {
    let id = field_i64(item, "item_id");
    let state = detail.collections.iter().find(|owned| owned.collection_id == id);
    let star = state.map(|owned| i64::from(owned.star_level)).unwrap_or(0);
    let own = state.map(|owned| owned.own_count).unwrap_or(0);
    let series_id = field_i64(series, "series_id");
    let unlocked = conditions_met(field_str(series, "unlock_condition_ids"), detail)
        && conditions_met(field_str(item, "unlock_condition_ids"), detail);
    let next = next_costs(series_id, star, id);
    let copies = next.iter().find_map(|cost| (cost.id == id).then_some(cost.count));
    let name = global_game_config()
        .get_item_by_id(id)
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| format!("藏品{id}"));
    json!({
        "id": id,
        "name": name,
        "starLevel": star,
        "ownCount": own,
        "activated": star > 0,
        "unlocked": unlocked,
        "displayed": displayed == id,
        "score": star_score(star, field_i64(series, "series_rarity")),
        "canUpgrade": unlocked && star < 5 && copies.is_some_and(|need| own >= need),
        "nextStarCosts": next.iter().map(|cost| json!({
            "itemId": cost.id,
            "count": cost.count,
            "name": cost.name,
        })).collect::<Vec<_>>(),
    })
}

fn next_costs(series_id: i64, star: i64, collection_id: i64) -> Vec<Material> {
    let Some(row) = table("star").iter().find(|row| {
        field_i64(row, "series_id") == series_id && field_i64(row, "star_level") == star + 1
    }) else {
        return Vec::new();
    };
    field_str(row, "items_need")
        .split(';')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (id, count) = part.split_once(':')?;
            let id = id.parse::<i64>().ok()?;
            let count = count.parse::<i64>().ok()?;
            let id = if id == 0 { collection_id } else { id };
            let name = global_game_config()
                .get_item_by_id(id)
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| format!("物品{id}"));
            Some(Material { id, count, name })
        })
        .collect()
}

fn star_score(star: i64, rarity: i64) -> i64 {
    let Some(row) = table("score").iter().find(|row| {
        category_of(row) == i64::from(CATEGORY) && field_i64(row, "star_level") == star
    }) else {
        return 0;
    };
    field_str(row, "score")
        .split(';')
        .nth(usize::try_from(rarity.saturating_sub(1)).unwrap_or(0))
        .and_then(|text| text.parse().ok())
        .unwrap_or(0)
}

fn conditions_met(value: &str, detail: &CategoryDetail) -> bool {
    value.is_empty()
        || value.split(';').all(|id| {
            let Some(condition) = table("condition")
                .iter()
                .find(|row| field_i64(row, "condition_id") == id.parse().unwrap_or(0))
            else {
                return false;
            };
            if field_i64(condition, "condition_type") != 3 {
                return false;
            }
            let series_id = field_i64(condition, "target_series_id");
            let need = field_i64(condition, "required_star_level");
            let count = field_i64(condition, "required_count");
            let matched = table("item")
                .iter()
                .filter(|item| field_i64(item, "series_id") == series_id)
                .filter(|item| {
                    detail.collections.iter().any(|owned| {
                        owned.collection_id == field_i64(item, "item_id")
                            && i64::from(owned.star_level) >= need
                    })
                })
                .count() as i64;
            matched >= count
        })
}

fn milestone_rows(score: i64, claimed_score: i64) -> Vec<Milestone> {
    let buffs = global_game_config().buffs_by_source("藏品属性");
    let mut rows = table("reward")
        .iter()
        .filter(|row| category_of(row) == i64::from(CATEGORY))
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| field_i64(row, "score"));
    rows.into_iter()
        .map(|row| {
            let level = field_i64(&row, "level");
            let need = field_i64(&row, "score");
            Milestone {
                level,
                score: need,
                claimed: need <= claimed_score,
                reached: need <= score,
                buffs: buffs
                    .iter()
                    .filter(|buff| buff.source_param == level)
                    .map(|buff| {
                        let value = buff.attr_value / 100;
                        let description = if buff.attr_id == "节省10%肥料" {
                            format!("{value}%概率节省10%肥料")
                        } else {
                            format!("{} +{value}%", buff.attr_id)
                        };
                        json!({ "name": buff.attr_id, "value": value, "description": description })
                    })
                    .collect(),
            }
        })
        .collect()
}

fn table(name: &str) -> &'static [Value] {
    fn load(json: &'static str, slot: &'static OnceLock<Vec<Value>>) -> &'static [Value] {
        slot.get_or_init(|| serde_json::from_str(json).expect("藏品配置无法读取"))
    }
    match name {
        "series" => {
            static SLOT: OnceLock<Vec<Value>> = OnceLock::new();
            load(include_str!("../../assets/collections/CollectionSeries.json"), &SLOT)
        }
        "item" => {
            static SLOT: OnceLock<Vec<Value>> = OnceLock::new();
            load(include_str!("../../assets/collections/CollectionItem.json"), &SLOT)
        }
        "star" => {
            static SLOT: OnceLock<Vec<Value>> = OnceLock::new();
            load(include_str!("../../assets/collections/CollectionStarLevelNeed.json"), &SLOT)
        }
        "score" => {
            static SLOT: OnceLock<Vec<Value>> = OnceLock::new();
            load(include_str!("../../assets/collections/CollectionScore.json"), &SLOT)
        }
        "reward" => {
            static SLOT: OnceLock<Vec<Value>> = OnceLock::new();
            load(include_str!("../../assets/collections/CollectionScoreReward.json"), &SLOT)
        }
        "condition" => {
            static SLOT: OnceLock<Vec<Value>> = OnceLock::new();
            load(include_str!("../../assets/collections/CollectionCondition.json"), &SLOT)
        }
        _ => &[],
    }
}

fn category_of(row: &Value) -> i64 {
    match row.get("category") {
        Some(Value::String(text)) => text.parse().unwrap_or(0),
        Some(Value::Number(number)) => number.as_i64().unwrap_or(0),
        _ => 0,
    }
}

fn field_i64(row: &Value, key: &str) -> i64 {
    match row.get(key) {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(0),
        Some(Value::String(text)) => text.parse().unwrap_or(0),
        _ => 0,
    }
}

fn field_str<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use crate::proto::generated::gamepb::collectionpb::CollectionInfo;

    use super::{
        claim_confirmed, cost_signature, display_confirmed, normalize, upgrade_confirmed,
        CategoryDetail, GetCollectionsReply, Material,
    };

    #[test]
    fn book_counts_an_activated_piece_and_a_ready_reward() {
        let reply = GetCollectionsReply {
            details: vec![CategoryDetail {
                category: 1,
                collections: vec![CollectionInfo {
                    collection_id: 710001,
                    star_level: 1,
                    own_count: 2,
                    ..Default::default()
                }],
                current_score: 400,
                claimed_reward_score: 0,
                ..Default::default()
            }],
            ..Default::default()
        };
        let book = normalize(&reply).expect("book");
        let series = book["series"].as_array().expect("series");
        let oxygen = series.iter().find(|row| row["name"] == "氧气精灵").expect("series");
        assert_eq!(oxygen["activatedCount"], 1);
        assert!(book["canClaim"].as_bool().unwrap_or(false));
        assert_eq!(book["score"], 400);
        let item =
            oxygen["items"].as_array().unwrap().iter().find(|item| item["id"] == 710001).unwrap();
        assert!(item["canUpgrade"].as_bool().unwrap_or(false));
        let costs = item["nextStarCosts"].as_array().expect("costs");
        assert!(costs.iter().any(|cost| cost["itemId"] == 710001 && cost["count"] == 2));
    }

    #[test]
    fn upgrade_and_display_replies_must_match_the_plan() {
        assert!(upgrade_confirmed(1, 710001, 2, 0, 720, 710001, 1, 2, 2, 400, 400, 720));
        assert!(!upgrade_confirmed(1, 710001, 2, 1, 720, 710001, 1, 2, 2, 400, 400, 720));
        assert!(display_confirmed(1, 1, 710001, 710001));
        assert!(!display_confirmed(1, 1, 0, 710001));
        let left = vec![
            Material { id: 1001, count: 2_000_000, name: String::new() },
            Material { id: 710001, count: 2, name: String::new() },
        ];
        let right = vec![
            Material { id: 710001, count: 2, name: String::new() },
            Material { id: 1001, count: 2_000_000, name: String::new() },
        ];
        assert_eq!(cost_signature(&left), cost_signature(&right));
    }

    #[test]
    fn claim_requires_the_score_to_advance() {
        assert!(claim_confirmed(400, 400, 400, 400));
        assert!(!claim_confirmed(400, 400, 399, 400));
        assert!(!claim_confirmed(400, 400, 400, 399));
        assert!(!claim_confirmed(400, 400, 400, 401));
    }
}
