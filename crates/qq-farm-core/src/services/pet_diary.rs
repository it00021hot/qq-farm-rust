//! 萌宠成长日记（bot `activity-center/pet-diary.ts`）。
//!
//! `GetGroup` 组 `2026090100`。养成 `2026090101`，种子 `2026090102`，小铺 `2026090103`。
//! 开放要求开始大于 0，并且现在落在开始和结束之间，结束时刻包含在内。
//! 协议里的引导步进没有生产入口。

use std::collections::HashMap;
use std::sync::Arc;

use prost::Message as _;

use crate::config::game_config::global as global_game_config;
use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::activitypb::{
    ActivityBodyPetTreasureHunt, GetGroupRequest, PetDiaryActivityData, PetDiaryGetGroupReply,
    PetDiaryMegaEventRewardState, PetDiaryOperateReply, PetDiaryOperateRequest, PetDiaryShopBuyReq,
    PetDiaryShopItemInfo, PetTreasureHuntBattle, PetTreasureHuntClaimDogReq,
    PetTreasureHuntClaimPlunderCompensationReq, PetTreasureHuntClaimStoryReq,
    PetTreasureHuntDrawReq, PetTreasureHuntEquipCharmsReq, PetTreasureHuntFeedReq,
    PetTreasureHuntFinishCgReq, PetTreasureHuntGetLogReq, PetTreasureHuntGetPlunderedLogReq,
    PetTreasureHuntMarkStoryAnimatedReq, PetTreasureHuntOpenTreasureReq,
    PetTreasureHuntRefreshCharmPoolReq, PetTreasureHuntSetSkipBattleCgReq,
    PetTreasureHuntStartBattleReq, PetTreasureHuntStoryInfo, PetTreasureHuntTreasure,
};
use crate::proto::generated::gamepb::itempb::{BagReply, BagRequest};
use crate::proto::generated::gamepb::solartermspb::{
    ClaimSolarTermsReply, ClaimSolarTermsRequest, GetSolarTermsReply, GetSolarTermsRequest,
};
use crate::utils::time::get_server_time_secs;

const ACTIVITY_SERVICE: &str = "gamepb.activitypb.ActivityService";
const ITEM_SERVICE: &str = "gamepb.itempb.ItemService";
const SOLAR_SERVICE: &str = "gamepb.solartermspb.SolarTermsService";
const GROUP_ID: i64 = 2_026_090_100;
const PET_ID: i64 = 2_026_090_101;
const SEEDS_ID: i64 = 2_026_090_102;
const SHOP_ID: i64 = 2_026_090_103;
const DIAMOND_ID: i64 = 1004;
const ADULT_STAGE: i32 = 2;
const BABY_STAGE: i32 = 1;
const PLANT_IDS: [i64; 6] = [20516, 29004, 25995, 21625, 20154, 21072];
const SHOP_QUERY: i64 = 7;
const INITIALIZE: i64 = 27;
const FEED: i64 = 29;
const DRAW: i64 = 30;
const STORY: i64 = 32;
const REFRESH_CHARM: i64 = 41;
const EQUIP_CHARM: i64 = 42;
const BATTLE: i64 = 43;
const OPEN_TREASURE: i64 = 45;
const COMPENSATION: i64 = 46;
const CLAIM_DOG: i64 = 48;
const MARK_STORIES: i64 = 49;
const SKIP_BATTLE: i64 = 50;
const SEEDS: i64 = 21;
const EXCHANGE: i64 = 1;
const INTERACT_LOG: i64 = 31;
const PLUNDER_LOG: i64 = 44;
const FRIEND_INFO: i64 = 47;
const SESSION_MOVED: &str = "账号会话已变化，未发送操作";
const ACCOUNT_INVALID: &str = "账号身份无效，未发送操作";
const CLOSED: &str = "萌宠成长日记当前不在活动时间内";
const MISSING_RESULT: &str = "活动响应缺少操作结果，请刷新后查看结果";

#[derive(serde::Deserialize)]
struct PetCatalog {
    #[serde(rename = "ActivityPetTreasureHuntBase")]
    base: Vec<PetBase>,
    #[serde(rename = "ActivityPetTreasureHuntFight")]
    fight: Vec<PetFight>,
    #[serde(rename = "ActivityPetTreasureCharmRefresh")]
    refresh: Vec<PetRefresh>,
    #[serde(rename = "ActivityPetTreasureHuntCharm")]
    charms: Vec<PetCharm>,
}

#[derive(serde::Deserialize)]
struct PetBase {
    feed_items: String,
    growth_adult_threshold: i64,
    daily_feed_limit: i32,
    daily_treasure_limit: i32,
}

#[derive(serde::Deserialize)]
struct PetFight {
    daily_battle_limit: i32,
}

#[derive(serde::Deserialize)]
struct PetRefresh {
    free_refresh_daily_limit: i64,
    manual_refresh_cost_id: i64,
    manual_refresh_cost_count: i64,
    manual_refresh_daily_limit: i64,
}

#[derive(serde::Deserialize)]
struct PetCharm {
    charm_id: i32,
    name: String,
    desc: String,
    #[serde(default)]
    short_desc: String,
    use_limit: i32,
    #[serde(default)]
    icon_path: String,
}

fn catalog() -> &'static PetCatalog {
    static LOADED: std::sync::OnceLock<PetCatalog> = std::sync::OnceLock::new();
    LOADED.get_or_init(|| {
        serde_json::from_str(include_str!("fixtures/pet-diary-2026090101.json"))
            .expect("萌宠配置无法解析")
    })
}

fn feed_pair() -> (i64, i64) {
    let mut parts = catalog().base[0].feed_items.split(':');
    let id = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let count = parts.next().unwrap_or("0").parse().unwrap_or(0);
    (id, count)
}

fn charm_config(id: i32) -> Option<&'static PetCharm> {
    catalog().charms.iter().find(|charm| charm.charm_id == id)
}

fn feed_limit() -> i32 {
    catalog().base[0].daily_feed_limit
}

fn treasure_limit() -> i32 {
    catalog().base[0].daily_treasure_limit
}

fn battle_limit() -> i32 {
    catalog().fight[0].daily_battle_limit
}

fn adult_growth() -> i64 {
    catalog().base[0].growth_adult_threshold
}

fn free_refresh_limit() -> i64 {
    catalog().refresh[0].free_refresh_daily_limit
}

fn paid_refresh_limit() -> i64 {
    catalog().refresh[0].manual_refresh_daily_limit
}

fn refresh_cost() -> (i64, i64) {
    let refresh = &catalog().refresh[0];
    (refresh.manual_refresh_cost_id, refresh.manual_refresh_cost_count)
}

pub struct PetDiaryService {
    gateway: Arc<Gateway>,
    mutation_lock: tokio::sync::Mutex<()>,
}

struct PetGroup {
    pet: PetDiaryActivityData,
    seeds: Option<PetDiaryActivityData>,
}

impl PetDiaryService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, mutation_lock: tokio::sync::Mutex::new(()) }
    }

    pub async fn snapshot(&self) -> Result<serde_json::Value> {
        let group = self.read_group().await?;
        let (bag, bag_warning) = match self.bag().await {
            Ok(bag) => (Some(bag), None),
            Err(_) => (None, Some("背包读取失败，消耗资源的操作已暂停".to_owned())),
        };
        let (solar, solar_warning) = match self.solar().await {
            Ok(reply) => (Some(reply), None),
            Err(_) => (None, Some("节令小礼读取失败，请稍后刷新".to_owned())),
        };
        let (shop, shop_warning) = match self.query_shop().await {
            Ok(data) => (Some(data), None),
            Err(error) => (None, Some(format!("拾物小铺：{error}"))),
        };
        let mut warnings = Vec::new();
        if let Some(warning) = bag_warning {
            warnings.push(warning);
        }
        if let Some(warning) = solar_warning {
            warnings.push(warning);
        }
        if let Some(warning) = shop_warning {
            warnings.push(warning);
        }
        Ok(self.dto(&group, shop.as_ref(), bag.as_ref(), solar.as_ref(), &warnings))
    }

    pub async fn feed(&self) -> Result<serde_json::Value> {
        self.feed_if(|| true).await
    }

    pub async fn feed_if(&self, still_current: impl Fn() -> bool) -> Result<serde_json::Value> {
        self.mutate(still_current, |group, bag| {
            let state = hunt_state(&group.pet)?;
            let nurture = state.nurture.clone().unwrap_or_default();
            let feed = state.feed.clone().unwrap_or_default();
            if nurture.stage != BABY_STAGE || feed.feed_count >= feed_limit() {
                return Err(Error::Business("当前不可投喂".into()));
            }
            if !costs_ok(&[feed_pair()], bag, 1) {
                return Err(Error::Business("萌宠元气糕不足，请先种植活动作物".into()));
            }
            Ok((
                PET_ID,
                FEED,
                with_field(PET_ID, FEED, |request| {
                    request.pet_treasure_hunt_feed = Some(PetTreasureHuntFeedReq {});
                }),
            ))
        })
        .await
    }

    pub async fn draw_if(&self, still_current: impl Fn() -> bool) -> Result<serde_json::Value> {
        self.mutate(still_current, |group, bag| {
            let state = hunt_state(&group.pet)?;
            let nurture = state.nurture.clone().unwrap_or_default();
            let hunt = state.hunt.clone().unwrap_or_default();
            if nurture.stage != ADULT_STAGE || hunt.treasure_count >= treasure_limit() {
                return Err(Error::Business("当前不可寻宝".into()));
            }
            let costs = item_pairs(&hunt.treasure_cost);
            if !costs_ok(&costs, bag, 1) {
                return Err(Error::Business("萌宠元气糕不足，请先种植活动作物".into()));
            }
            Ok((
                PET_ID,
                DRAW,
                with_field(PET_ID, DRAW, |request| {
                    request.pet_treasure_hunt_draw = Some(PetTreasureHuntDrawReq {});
                }),
            ))
        })
        .await
    }

    pub async fn initialize_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.mutate(still_current, |group, _bag| {
            let nurture = hunt_state(&group.pet)?.nurture.clone().unwrap_or_default();
            if nurture.cg_played {
                return Err(Error::Business("已领养比熊，请刷新状态".into()));
            }
            Ok((
                PET_ID,
                INITIALIZE,
                with_field(PET_ID, INITIALIZE, |request| {
                    request.pet_treasure_hunt_finish_cg = Some(PetTreasureHuntFinishCgReq {});
                }),
            ))
        })
        .await
    }

    pub async fn claim_dog_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.mutate(still_current, |group, _bag| {
            let nurture = hunt_state(&group.pet)?.nurture.clone().unwrap_or_default();
            if nurture.stage != ADULT_STAGE || nurture.dog_granted {
                return Err(Error::Business("比熊尚未成年或已经领取".into()));
            }
            Ok((
                PET_ID,
                CLAIM_DOG,
                with_field(PET_ID, CLAIM_DOG, |request| {
                    request.pet_treasure_hunt_claim_dog = Some(PetTreasureHuntClaimDogReq {});
                }),
            ))
        })
        .await
    }

    pub async fn claim_story_if(
        &self,
        order: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if order <= 0 {
            return Err(Error::Business("手记编号必须是正整数".into()));
        }
        self.mutate(still_current, |group, _bag| {
            let stories = hunt_state(&group.pet)?.story.clone().unwrap_or_default().stories;
            let ready = stories
                .iter()
                .any(|story| i64::from(story.order) == order && story.unlocked && !story.claimed);
            if !ready {
                return Err(Error::Business("手记尚未解锁或已领取".into()));
            }
            let mut request = empty_operate(PET_ID, STORY);
            request.pet_treasure_hunt_claim_story =
                Some(PetTreasureHuntClaimStoryReq { order: i32::try_from(order).unwrap_or(0) });
            Ok((PET_ID, STORY, request))
        })
        .await
    }

    pub async fn claim_seeds_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.mutate(still_current, |group, _bag| {
            let seeds = group
                .seeds
                .as_ref()
                .ok_or_else(|| Error::Business("当前没有可领取的种子礼包".into()))?;
            if !head_active(seeds)
                || !seeds
                    .mega_event
                    .clone()
                    .unwrap_or_default()
                    .rewards
                    .iter()
                    .any(|reward| reward.claimable && !reward.claimed)
            {
                return Err(Error::Business("当前没有可领取的种子礼包".into()));
            }
            Ok((
                SEEDS_ID,
                SEEDS,
                with_field(SEEDS_ID, SEEDS, |request| {
                    request.mega_event_claim_all = Some(PetTreasureHuntFinishCgReq {});
                }),
            ))
        })
        .await
    }

    pub async fn refresh_charm_if(
        &self,
        payment: &str,
        expected_paid: Option<i64>,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if payment != "free" && payment != "tickets" {
            return Err(Error::Business("锦囊刷新不支持使用钻石".into()));
        }
        self.mutate(still_current, |group, bag| {
            let state = hunt_state(&group.pet)?;
            let nurture = state.nurture.clone().unwrap_or_default();
            let battle = state.battle.clone().unwrap_or_default();
            if nurture.stage != ADULT_STAGE {
                return Err(Error::Business("比熊成年后才可刷新锦囊".into()));
            }
            if !battle.charm_equipped.is_empty() && !battle.charm_pick_used {
                return Err(Error::Business("请先替换或保留当前锦囊，再刷新".into()));
            }
            let free = battle.charm_free_refresh_count < free_refresh_limit();
            if free {
                if payment != "free" {
                    return Err(Error::Business("刷新次数已变化，请刷新状态后重试".into()));
                }
            } else {
                if payment != "tickets" {
                    return Err(Error::Business("免费刷新已用完，请确认点券费用后再操作".into()));
                }
                if battle.charm_paid_refresh_count >= paid_refresh_limit() {
                    return Err(Error::Business("今日付费刷新次数已用完".into()));
                }
                if expected_paid != Some(battle.charm_paid_refresh_count) {
                    return Err(Error::Business("刷新次数已变化，请刷新状态后重试".into()));
                }
                if !costs_ok(&[refresh_cost()], bag, 1) {
                    return Err(Error::Business("点券不足，已停止刷新，不使用钻石".into()));
                }
            }
            Ok((
                PET_ID,
                REFRESH_CHARM,
                with_field(PET_ID, REFRESH_CHARM, |request| {
                    request.pet_treasure_hunt_refresh_charm_pool =
                        Some(PetTreasureHuntRefreshCharmPoolReq {});
                }),
            ))
        })
        .await
    }

    pub async fn equip_charm_if(
        &self,
        charm_id: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if charm_id <= 0 {
            return Err(Error::Business("锦囊编号必须是正整数".into()));
        }
        self.mutate(still_current, |group, _bag| {
            let state = hunt_state(&group.pet)?;
            let nurture = state.nurture.clone().unwrap_or_default();
            let battle = state.battle.clone().unwrap_or_default();
            let charm = i32::try_from(charm_id).unwrap_or(0);
            let choices = battle.charm_daily_pool.iter().chain(battle.charm_equipped.iter());
            if nurture.stage != ADULT_STAGE
                || battle.charm_pick_used
                || !choices.clone().any(|id| *id == charm)
            {
                return Err(Error::Business("该锦囊不可选择或本轮已经选择".into()));
            }
            let mut request = empty_operate(PET_ID, EQUIP_CHARM);
            request.pet_treasure_hunt_equip_charms =
                Some(PetTreasureHuntEquipCharmsReq { charm_ids: vec![charm] });
            Ok((PET_ID, EQUIP_CHARM, request))
        })
        .await
    }

    pub async fn battle_if(
        &self,
        gid: i64,
        treasure_id: &str,
        challenge_id: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if gid <= 0 || treasure_id.is_empty() || ![80101, 80102, 80103].contains(&challenge_id) {
            return Err(Error::Business("夺宝目标或挑战书无效".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let group = self.read_group().await?;
        if !head_active(&group.pet) {
            return Err(Error::Business(CLOSED.into()));
        }
        let state = hunt_state(&group.pet)?;
        let hunt = state.hunt.clone().unwrap_or_default();
        let battle = state.battle.clone().unwrap_or_default();
        if !hunt.can_play_plunder || battle.battle_count >= battle_limit() {
            return Err(Error::Business("当前不可夺宝".into()));
        }
        let bag = self.bag().await?;
        if !costs_ok(&[(challenge_id, 1)], Some(&bag), 1) {
            return Err(Error::Business("对应挑战书不足".into()));
        }
        let friend = self
            .gateway
            .request(
                ACTIVITY_SERVICE,
                "Operate",
                &with_field(PET_ID, FRIEND_INFO, |request| {
                    request.pet_treasure_hunt_get_friend_activity_info = Some(
                        crate::proto::generated::gamepb::activitypb::PetTreasureHuntGetFriendActivityInfoReq {
                            friend_gid: gid,
                        },
                    );
                })
                .encode_to_vec(),
            )
            .await
            .map_err(Error::Network)?;
        let friend = PetDiaryOperateReply::decode(friend.as_slice())?;
        if friend.activity_id != PET_ID || friend.operate_type != FRIEND_INFO {
            return Err(Error::Business("活动响应不匹配，请刷新后查看结果".into()));
        }
        let info = friend
            .pet_treasure_hunt_get_friend_activity_info
            .as_ref()
            .ok_or_else(|| Error::Business("好友响应不匹配".into()))?;
        if info.gid != gid {
            return Err(Error::Business("好友响应不匹配".into()));
        }
        let ready = info.info.as_ref().is_some_and(|item| {
            item.treasures.iter().any(|treasure| {
                treasure.id == treasure_id
                    && treasure.status == 2
                    && treasure.battle_previews.iter().any(|preview| {
                        preview.challenge_item_id == challenge_id && preview.can_start
                    })
            })
        });
        if !ready {
            return Err(Error::Business("好友宝藏状态已变化，请重新查看".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let mut request = empty_operate(PET_ID, BATTLE);
        request.pet_treasure_hunt_start_battle = Some(PetTreasureHuntStartBattleReq {
            defender_gid: gid,
            treasure_id: treasure_id.to_owned(),
            challenge_item_id: challenge_id,
        });
        self.finish(PET_ID, BATTLE, request, "操作成功").await
    }

    pub async fn open_treasure_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.mutate(still_current, |group, _bag| {
            let now = get_server_time_secs();
            let ready =
                hunt_state(&group.pet)?.pool.clone().unwrap_or_default().treasures.iter().any(
                    |treasure| {
                        treasure.status == 3
                            || (treasure.status == 2
                                && treasure.end_at > 0
                                && treasure.end_at <= now)
                    },
                );
            if !ready {
                return Err(Error::Business("还没有完成护送的宝藏".into()));
            }
            Ok((
                PET_ID,
                OPEN_TREASURE,
                with_field(PET_ID, OPEN_TREASURE, |request| {
                    request.pet_treasure_hunt_open_treasure =
                        Some(PetTreasureHuntOpenTreasureReq {});
                }),
            ))
        })
        .await
    }

    pub async fn claim_compensation_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.mutate(still_current, |group, _bag| {
            let count = hunt_state(&group.pet)?
                .plunder
                .clone()
                .unwrap_or_default()
                .plunder_compensation_count;
            if count <= 0 {
                return Err(Error::Business("当前没有可领取的夺宝补偿".into()));
            }
            Ok((
                PET_ID,
                COMPENSATION,
                with_field(PET_ID, COMPENSATION, |request| {
                    request.pet_treasure_hunt_claim_plunder_compensation =
                        Some(PetTreasureHuntClaimPlunderCompensationReq {});
                }),
            ))
        })
        .await
    }

    pub async fn skip_battle_if(
        &self,
        skip: bool,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.mutate(still_current, |_group, _bag| {
            let mut request = empty_operate(PET_ID, SKIP_BATTLE);
            request.pet_treasure_hunt_set_skip_battle_cg =
                Some(PetTreasureHuntSetSkipBattleCgReq { skip });
            Ok((PET_ID, SKIP_BATTLE, request))
        })
        .await
    }

    pub async fn mark_stories_if(
        &self,
        orders: &[i64],
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if orders.is_empty() || orders.iter().any(|order| *order <= 0) {
            return Err(Error::Business("手记编号无效".into()));
        }
        self.mutate(still_current, |group, _bag| {
            let stories = hunt_state(&group.pet)?.story.clone().unwrap_or_default().stories;
            let valid = orders.iter().all(|order| {
                stories.iter().any(|story| i64::from(story.order) == *order && story.unlocked)
            });
            if !valid {
                return Err(Error::Business("手记编号无效".into()));
            }
            let mut request = empty_operate(PET_ID, MARK_STORIES);
            request.pet_treasure_hunt_mark_story_animated =
                Some(PetTreasureHuntMarkStoryAnimatedReq {
                    orders: orders.iter().filter_map(|order| i32::try_from(*order).ok()).collect(),
                });
            Ok((PET_ID, MARK_STORIES, request))
        })
        .await
    }

    pub async fn exchange_if(
        &self,
        goods_id: i64,
        count: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if goods_id <= 0 || count <= 0 {
            return Err(Error::Business("兑换数量必须是正整数".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let group = self.read_group().await?;
        if !head_active(&group.pet) {
            return Err(Error::Business(CLOSED.into()));
        }
        let shop = self.query_shop().await?;
        if !head_active(&shop) {
            return Err(Error::Business("拾物小铺当前不可兑换".into()));
        }
        let goods = shop
            .shop
            .clone()
            .unwrap_or_default()
            .goods
            .into_iter()
            .find(|goods| goods.id == goods_id)
            .ok_or_else(|| Error::Business("服务端目录未发现该商品".into()))?;
        if goods.diamond_cost_count > 0 || goods.cost.iter().any(|cost| cost.id == DIAMOND_ID) {
            return Err(Error::Business("该商品可能消耗钻石，已阻止兑换".into()));
        }
        if goods.purchase_limit > 0
            && goods.purchased_count.saturating_add(count) > goods.purchase_limit
        {
            return Err(Error::Business("兑换数量超过剩余限购次数".into()));
        }
        let bag = self.bag().await?;
        if !costs_ok(&item_pairs_shop(&goods.cost), Some(&bag), count) {
            return Err(Error::Business("兑换余额不足".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let mut request = empty_operate(SHOP_ID, EXCHANGE);
        request.shop_buy = Some(PetDiaryShopBuyReq { goods_id, count });
        self.finish(SHOP_ID, EXCHANGE, request, "操作成功").await
    }

    pub async fn interact_log_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.mutate(still_current, |_group, _bag| {
            Ok((
                PET_ID,
                INTERACT_LOG,
                with_field(PET_ID, INTERACT_LOG, |request| {
                    request.pet_treasure_hunt_get_log = Some(PetTreasureHuntGetLogReq {});
                }),
            ))
        })
        .await
    }

    pub async fn plunder_log_if(
        &self,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        self.mutate(still_current, |_group, _bag| {
            Ok((
                PET_ID,
                PLUNDER_LOG,
                with_field(PET_ID, PLUNDER_LOG, |request| {
                    request.pet_treasure_hunt_get_plundered_log =
                        Some(PetTreasureHuntGetPlunderedLogReq {});
                }),
            ))
        })
        .await
    }

    pub async fn friend_if(
        &self,
        gid: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if gid <= 0 {
            return Err(Error::Business("好友 GID 必须是正整数".into()));
        }
        self.mutate(still_current, |_group, _bag| {
            let mut request = empty_operate(PET_ID, FRIEND_INFO);
            request.pet_treasure_hunt_get_friend_activity_info = Some(
                crate::proto::generated::gamepb::activitypb::PetTreasureHuntGetFriendActivityInfoReq {
                    friend_gid: gid,
                },
            );
            Ok((PET_ID, FRIEND_INFO, request))
        })
        .await
    }

    pub async fn claim_solar_if(
        &self,
        term_id: i64,
        still_current: impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if term_id <= 0 {
            return Err(Error::Business("节令编号必须是正整数".into()));
        }
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let group = self.read_group().await?;
        if !head_active(&group.pet) {
            return Err(Error::Business(CLOSED.into()));
        }
        let solar = self.solar().await?;
        let start = group.pet.head.clone().unwrap_or_default().start_time;
        let end = group.pet.head.clone().unwrap_or_default().end_time;
        let term = solar.terms.iter().find(|term| {
            term.term_id == term_id
                && term.end_time >= start
                && term.begin_time <= end
                && term.status == 2
        });
        if term.is_none() {
            return Err(Error::Business("该节令当前不可领取".into()));
        }
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let request = ClaimSolarTermsRequest { term_id };
        let bytes = self
            .gateway
            .request(SOLAR_SERVICE, "ClaimSolarTerms", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = ClaimSolarTermsReply::decode(bytes.as_slice())?;
        if reply.term.as_ref().is_none_or(|term| term.term_id != term_id || term.status != 3) {
            return Err(Error::Business("节令回包不匹配，请刷新确认领取状态".into()));
        }
        let mut payload = serde_json::json!({
            "action": "solar",
            "message": "节令好礼领取成功",
            "rewards": reply.rewards.iter().map(|reward| serde_json::json!({
                "id": reward.item_id.to_string(),
                "count": reward.count,
                "name": item_name(reward.item_id),
            })).collect::<Vec<_>>(),
        });
        self.attach_snapshot(&mut payload).await;
        Ok(payload)
    }

    async fn mutate(
        &self,
        still_current: impl Fn() -> bool,
        prepare: impl Fn(
            &PetGroup,
            Option<&HashMap<i64, i64>>,
        ) -> Result<(i64, i64, PetDiaryOperateRequest)>,
    ) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        let group = self.read_group().await?;
        if !head_active(&group.pet) {
            return Err(Error::Business(CLOSED.into()));
        }
        let bag = self.bag().await.ok();
        let (activity, operate, request) = prepare(&group, bag.as_ref())?;
        if !still_current() {
            return Err(Error::Business(SESSION_MOVED.into()));
        }
        self.finish(activity, operate, request, "操作成功").await
    }

    async fn finish(
        &self,
        activity: i64,
        operate: i64,
        request: PetDiaryOperateRequest,
        message: &str,
    ) -> Result<serde_json::Value> {
        let bytes = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &request.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = PetDiaryOperateReply::decode(bytes.as_slice())?;
        if reply.activity_id != activity || reply.operate_type != operate {
            return Err(Error::Business("活动响应不匹配，请刷新后查看结果".into()));
        }
        if !selector_present(&reply, operate) {
            return Err(Error::Business(MISSING_RESULT.into()));
        }
        let rewards = reply_rewards(&reply);
        let mut payload = serde_json::json!({
            "message": if operate == BATTLE {
                if reply.pet_treasure_hunt_start_battle.as_ref().is_some_and(|item| item.won) {
                    "夺宝成功"
                } else {
                    "本次夺宝未获胜，已按规则结算"
                }
            } else {
                message
            },
            "rewards": rewards,
        });
        attach_operation(&mut payload, &reply, operate);
        self.attach_snapshot(&mut payload).await;
        Ok(payload)
    }

    async fn attach_snapshot(&self, payload: &mut serde_json::Value) {
        match self.snapshot().await {
            Ok(snapshot) => payload["snapshot"] = snapshot,
            Err(error) => {
                payload["refreshRequired"] = serde_json::json!(true);
                payload["refreshError"] = serde_json::json!(refresh_failure(&error));
            }
        }
    }

    async fn read_group(&self) -> Result<PetGroup> {
        let body = GetGroupRequest { group_id: GROUP_ID }.encode_to_vec();
        let bytes = self
            .gateway
            .request(ACTIVITY_SERVICE, "GetGroup", &body)
            .await
            .map_err(Error::Network)?;
        let reply = PetDiaryGetGroupReply::decode(bytes.as_slice())?;
        let group =
            reply.group.ok_or_else(|| Error::Business("服务端未返回萌宠成长日记活动".into()))?;
        if group.head.as_ref().is_none_or(|head| head.id != GROUP_ID) {
            return Err(Error::Business("服务端未返回萌宠成长日记活动".into()));
        }
        let pet = group
            .children
            .iter()
            .find(|child| {
                child.head.as_ref().is_some_and(|head| head.id == PET_ID)
                    && child.pet_treasure_hunt.is_some()
            })
            .cloned()
            .ok_or_else(|| Error::Business("服务端未返回萌宠养成状态".into()))?;
        let seeds = group
            .children
            .iter()
            .find(|child| child.head.as_ref().is_some_and(|head| head.id == SEEDS_ID))
            .cloned();
        Ok(PetGroup { pet, seeds })
    }

    async fn query_shop(&self) -> Result<PetDiaryActivityData> {
        let bytes = self
            .gateway
            .request(
                ACTIVITY_SERVICE,
                "Operate",
                &empty_operate(SHOP_ID, SHOP_QUERY).encode_to_vec(),
            )
            .await
            .map_err(Error::Network)?;
        let reply = PetDiaryOperateReply::decode(bytes.as_slice())?;
        if reply.activity_id != SHOP_ID || reply.operate_type != SHOP_QUERY {
            return Err(Error::Business("活动响应不匹配，请刷新后查看结果".into()));
        }
        reply
            .data
            .filter(|data| data.shop.is_some())
            .ok_or_else(|| Error::Business("拾物小铺目录缺失".into()))
    }

    async fn bag(&self) -> Result<HashMap<i64, i64>> {
        let bytes = self
            .gateway
            .request(ITEM_SERVICE, "Bag", &BagRequest {}.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        let reply = BagReply::decode(bytes.as_slice())?;
        let mut totals = HashMap::new();
        for item in reply.item_bag.map(|bag| bag.items).unwrap_or_default() {
            *totals.entry(item.id).or_insert(0) += item.count;
        }
        Ok(totals)
    }

    async fn solar(&self) -> Result<GetSolarTermsReply> {
        let bytes = self
            .gateway
            .request(SOLAR_SERVICE, "GetSolarTerms", &GetSolarTermsRequest {}.encode_to_vec())
            .await
            .map_err(Error::Network)?;
        Ok(GetSolarTermsReply::decode(bytes.as_slice())?)
    }

    fn dto(
        &self,
        group: &PetGroup,
        shop: Option<&PetDiaryActivityData>,
        bag: Option<&HashMap<i64, i64>>,
        solar: Option<&GetSolarTermsReply>,
        warnings: &[String],
    ) -> serde_json::Value {
        let head = group.pet.head.clone().unwrap_or_default();
        let state = group.pet.pet_treasure_hunt.clone().unwrap_or_default();
        let nurture = state.nurture.clone().unwrap_or_default();
        let feed = state.feed.clone().unwrap_or_default();
        let hunt = state.hunt.clone().unwrap_or_default();
        let battle = state.battle.clone().unwrap_or_default();
        let active = head_active(&group.pet);
        let adult = nurture.stage == ADULT_STAGE;
        let known = bag.is_some();
        let feed_costs = vec![feed_pair()];
        let draw_costs = item_pairs(&hunt.treasure_cost);
        let seeds = group.seeds.as_ref();
        let seed_rewards = seeds
            .and_then(|item| item.mega_event.clone())
            .map(|event| event.rewards)
            .unwrap_or_default();
        let start = head.start_time;
        let end = head.end_time;
        let (rules_title, rules) =
            crate::services::activity_text::text_content(head.desc.as_bytes());
        let (treasure_title, treasure_rules) = treasure_rule_lines(&head.desc);
        serde_json::json!({
            "available": true,
            "activityId": PET_ID.to_string(),
            "groupId": GROUP_ID.to_string(),
            "title": "萌宠成长日记",
            "active": active,
            "startTime": start.to_string(),
            "endTime": end.to_string(),
            "endTimeText": readable_time(end),
            "rulesTitle": rules_title,
            "rules": rules,
            "treasureRulesTitle": treasure_title,
            "treasureRules": treasure_rules,
            "warnings": warnings,
            "balances": balance_rows(bag),
            "nurture": {
                "initialized": nurture.cg_played,
                "adult": adult,
                "growth": nurture.growth,
                "adultGrowth": adult_growth(),
                "dogGranted": nurture.dog_granted,
                "feedCount": feed.feed_count,
                "feedLimit": feed_limit(),
                "feedCosts": cost_rows(&feed_costs, bag),
                "canFeed": active && !adult && nurture.stage == BABY_STAGE && feed.feed_count < feed_limit() && costs_ok(&feed_costs, bag, 1),
            },
            "hunt": {
                "count": hunt.treasure_count,
                "limit": treasure_limit(),
                "total": hunt.treasure_total.to_string(),
                "luckyStarTotal": hunt.lucky_star_gained_total.to_string(),
                "costs": cost_rows(&draw_costs, bag),
                "canDraw": active && adult && hunt.treasure_count < treasure_limit() && costs_ok(&draw_costs, bag, 1),
                "canPlunder": active && hunt.can_play_plunder && battle.battle_count < battle_limit(),
            },
            "seeds": {
                "canClaim": active && seeds.is_some_and(head_active) && seed_rewards.iter().any(|reward| reward.claimable && !reward.claimed),
                "days": seed_rewards.iter().map(seed_row).collect::<Vec<_>>(),
            },
            "stories": state.story.clone().unwrap_or_default().stories.iter().map(story_row).collect::<Vec<_>>(),
            "charms": charm_rows(&battle, bag, active, adult),
            "treasures": state.pool.clone().unwrap_or_default().treasures.iter().map(treasure_row).collect::<Vec<_>>(),
            "compensationCount": state.plunder.clone().unwrap_or_default().plunder_compensation_count.to_string(),
            "battleCount": battle.battle_count,
            "battleLimit": battle_limit(),
            "skipBattle": battle.is_skip_battle_cg,
            "plants": plant_rows(bag),
            "shop": shop.map(|data| shop_rows(data, bag, active)).unwrap_or_default(),
            "shopKnown": shop.is_some(),
            "bagKnown": known,
            "solarTerms": solar.map(|reply| solar_rows(reply, start, end)).unwrap_or_default(),
        })
    }
}

pub fn require_pet_account(account_id: &str) -> Result<()> {
    if account_id.is_empty() || account_id.chars().all(char::is_whitespace) {
        Err(Error::Business(ACCOUNT_INVALID.into()))
    } else {
        Ok(())
    }
}

/// 开始必须大于 0。现在不早于开始，也不晚于结束。
#[must_use]
pub fn pet_window_is_open(begin: i64, end: i64, now: i64) -> bool {
    begin > 0 && now >= begin && now <= end
}

fn selector_present(reply: &PetDiaryOperateReply, operate: i64) -> bool {
    match operate {
        INITIALIZE => reply.pet_treasure_hunt_finish_cg.is_some(),
        FEED => reply.pet_treasure_hunt_feed.is_some(),
        DRAW => reply.pet_treasure_hunt_draw.is_some(),
        STORY => reply.pet_treasure_hunt_claim_story.is_some(),
        REFRESH_CHARM => reply.pet_treasure_hunt_refresh_charm_pool.is_some(),
        EQUIP_CHARM => reply.pet_treasure_hunt_equip_charms.is_some(),
        BATTLE => reply.pet_treasure_hunt_start_battle.is_some(),
        OPEN_TREASURE => reply.pet_treasure_hunt_open_treasure.is_some(),
        COMPENSATION => reply.pet_treasure_hunt_claim_plunder_compensation.is_some(),
        CLAIM_DOG => reply.pet_treasure_hunt_claim_dog.is_some(),
        MARK_STORIES => reply.pet_treasure_hunt_mark_story_animated.is_some(),
        SKIP_BATTLE => reply.pet_treasure_hunt_set_skip_battle_cg.is_some(),
        SEEDS => reply.mega_event_claim_all.is_some(),
        EXCHANGE => reply.shop_buy.is_some(),
        INTERACT_LOG => reply.pet_treasure_hunt_get_log.is_some(),
        PLUNDER_LOG => reply.pet_treasure_hunt_get_plundered_log.is_some(),
        FRIEND_INFO => reply.pet_treasure_hunt_get_friend_activity_info.is_some(),
        SHOP_QUERY => reply.data.as_ref().is_some_and(|data| data.shop.is_some()),
        _ => false,
    }
}

fn attach_operation(payload: &mut serde_json::Value, reply: &PetDiaryOperateReply, operate: i64) {
    if operate == FRIEND_INFO {
        if let Some(result) = reply.pet_treasure_hunt_get_friend_activity_info.as_ref() {
            payload["friend"] = serde_json::json!({
                "gid": result.gid.to_string(),
                "treasures": result.info.clone().unwrap_or_default().treasures.iter().map(treasure_row).collect::<Vec<_>>(),
            });
        }
    }
    if operate == INTERACT_LOG {
        if let Some(result) = reply.pet_treasure_hunt_get_log.as_ref() {
            payload["logs"] = serde_json::json!(result
                .logs
                .iter()
                .map(|entry| serde_json::json!({
                    "timeText": readable_time(entry.ts),
                    "type": entry.r#type,
                    "costs": entry.costs.iter().map(reward_value).collect::<Vec<_>>(),
                    "rewards": entry.rewards.iter().map(reward_value).collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>());
        }
    }
    if operate == PLUNDER_LOG {
        if let Some(result) = reply.pet_treasure_hunt_get_plundered_log.as_ref() {
            payload["plunderLogs"] =
                serde_json::json!(result.logs.iter().map(|entry| serde_json::json!({
                "timeText": readable_time(entry.ts),
                "name": entry.attacker_name,
                "gid": entry.attacker_gid.to_string(),
                "won": entry.attacker_won,
                "challengeId": entry.challenge_item_id.to_string(),
                "challengeName": item_name(entry.challenge_item_id),
                "lost": entry.lost_items.iter().map(reward_value).collect::<Vec<_>>(),
                "injected": entry.injected_items.iter().map(reward_value).collect::<Vec<_>>(),
                "level": entry.attacker_level,
                "fake": entry.is_fake,
            })).collect::<Vec<_>>());
        }
    }
}

fn refresh_failure(error: &Error) -> String {
    let text = error.to_string();
    let safe = if text.to_ascii_lowercase().contains("token")
        || text.to_ascii_lowercase().contains("password")
        || text.to_ascii_lowercase().contains("secret")
    {
        "内部错误".to_owned()
    } else {
        text
    };
    format!("操作已成功，刷新失败：{safe}")
}

fn head_active(data: &PetDiaryActivityData) -> bool {
    let head = data.head.clone().unwrap_or_default();
    pet_window_is_open(head.start_time, head.end_time, get_server_time_secs())
}

fn hunt_state(data: &PetDiaryActivityData) -> Result<ActivityBodyPetTreasureHunt> {
    data.pet_treasure_hunt.clone().ok_or_else(|| Error::Business("服务端未返回萌宠养成状态".into()))
}

fn empty_operate(activity: i64, operate: i64) -> PetDiaryOperateRequest {
    PetDiaryOperateRequest {
        activity_id: activity,
        operate_type: operate,
        ..PetDiaryOperateRequest::default()
    }
}

fn with_field(
    activity: i64,
    operate: i64,
    fill: impl FnOnce(&mut PetDiaryOperateRequest),
) -> PetDiaryOperateRequest {
    let mut request = empty_operate(activity, operate);
    fill(&mut request);
    request
}

fn costs_ok(costs: &[(i64, i64)], bag: Option<&HashMap<i64, i64>>, count: i64) -> bool {
    let Some(bag) = bag else {
        return false;
    };
    if costs.is_empty() || count <= 0 {
        return false;
    }
    let mut totals = HashMap::<i64, i64>::new();
    for (id, amount) in costs {
        if *id == DIAMOND_ID || *id <= 0 || *amount <= 0 {
            return false;
        }
        *totals.entry(*id).or_insert(0) += amount.saturating_mul(count);
    }
    totals.iter().all(|(id, amount)| bag.get(id).copied().unwrap_or(0) >= *amount)
}

fn item_pairs(items: &[CoreItem]) -> Vec<(i64, i64)> {
    items.iter().map(|item| (item.id, item.count)).collect()
}

fn item_pairs_shop(items: &[PetDiaryShopItemInfo]) -> Vec<(i64, i64)> {
    items.iter().map(|item| (item.id, item.count)).collect()
}

fn reward_value(item: &CoreItem) -> serde_json::Value {
    serde_json::json!({ "id": item.id.to_string(), "count": item.count, "name": item_name(item.id) })
}

fn item_name(id: i64) -> String {
    global_game_config()
        .get_item_by_id(id)
        .map(|item| item.name)
        .unwrap_or_else(|| format!("物品 #{id}"))
}

fn reply_rewards(reply: &PetDiaryOperateReply) -> Vec<serde_json::Value> {
    let rows = reply
        .pet_treasure_hunt_feed
        .as_ref()
        .map(|item| item.rewards.clone())
        .or_else(|| reply.pet_treasure_hunt_draw.as_ref().map(|item| item.rewards.clone()))
        .or_else(|| reply.pet_treasure_hunt_claim_story.as_ref().map(|item| item.awards.clone()))
        .or_else(|| reply.pet_treasure_hunt_open_treasure.as_ref().map(|item| item.rewards.clone()))
        .or_else(|| {
            reply
                .pet_treasure_hunt_claim_plunder_compensation
                .as_ref()
                .map(|item| item.rewards.clone())
        })
        .or_else(|| reply.shop_buy.as_ref().map(|item| item.awards.clone()))
        .or_else(|| reply.mega_event_claim_all.as_ref().map(|item| item.awards.clone()))
        .or_else(|| reply.pet_treasure_hunt_start_battle.as_ref().map(|item| item.rewards.clone()))
        .unwrap_or_default();
    rows.iter().map(reward_value).collect()
}

fn cost_rows(costs: &[(i64, i64)], bag: Option<&HashMap<i64, i64>>) -> Vec<serde_json::Value> {
    costs
        .iter()
        .map(|(id, count)| {
            serde_json::json!({
                "id": id.to_string(),
                "count": count,
                "name": item_name(*id),
                "known": bag.is_some(),
                "owned": bag.and_then(|bag| bag.get(id).copied()).unwrap_or(0).to_string(),
            })
        })
        .collect()
}

fn balance_rows(bag: Option<&HashMap<i64, i64>>) -> Vec<serde_json::Value> {
    [1028, 1029, 80101, 80102, 80103, 1002]
        .into_iter()
        .map(|id| {
            serde_json::json!({
                "id": id.to_string(),
                "name": item_name(id),
                "count": bag.and_then(|bag| bag.get(&id).copied()).unwrap_or(0).to_string(),
                "known": bag.is_some(),
            })
        })
        .collect()
}

fn seed_row(reward: &PetDiaryMegaEventRewardState) -> serde_json::Value {
    serde_json::json!({
        "day": reward.unlock_day,
        "claimed": reward.claimed,
        "claimable": reward.claimable,
        "rewards": reward.reward.iter().map(reward_value).collect::<Vec<_>>(),
    })
}

fn story_row(story: &PetTreasureHuntStoryInfo) -> serde_json::Value {
    let parsed =
        serde_json::from_str::<serde_json::Value>(&story.selected_desc).unwrap_or_default();
    serde_json::json!({
        "order": story.order,
        "unlocked": story.unlocked,
        "claimed": story.claimed,
        "animated": story.animated,
        "photo": parsed.get("photo").and_then(|item| item.as_str()).unwrap_or(""),
        "note": parsed.get("note").or_else(|| parsed.get("text")).and_then(|item| item.as_str()).unwrap_or(""),
    })
}

fn treasure_row(treasure: &PetTreasureHuntTreasure) -> serde_json::Value {
    let status = match treasure.status {
        1 => "待护送",
        2 if treasure.end_at > 0 && treasure.end_at <= get_server_time_secs() => "待领取",
        2 => "护送中",
        3 => "待领取",
        4 => "已领取",
        _ => "待刷新",
    };
    serde_json::json!({
        "id": treasure.id.to_string(),
        "status": status,
        "statusCode": treasure.status,
        "item": reward_value(&CoreItem { id: treasure.item_id, count: treasure.count, ..CoreItem::default() }),
        "count": treasure.count,
        "startTimeText": readable_time(treasure.start_at),
        "endTimeText": readable_time(treasure.end_at),
        "plunderCount": treasure.plunder_count,
        "maxPlunderCount": treasure.max_plunder_count,
        "previews": treasure.battle_previews.iter().map(|preview| serde_json::json!({
            "challengeId": preview.challenge_item_id.to_string(),
            "challengeName": item_name(preview.challenge_item_id),
            "canStart": preview.can_start,
            "plunderableCount": preview.plunderable_count.to_string(),
        })).collect::<Vec<_>>(),
    })
}

fn charm_rows(
    battle: &PetTreasureHuntBattle,
    bag: Option<&HashMap<i64, i64>>,
    active: bool,
    adult: bool,
) -> serde_json::Value {
    let free_remaining = (free_refresh_limit() - battle.charm_free_refresh_count).max(0);
    let paid_remaining = (paid_refresh_limit() - battle.charm_paid_refresh_count).max(0);
    let needs_choice = !battle.charm_equipped.is_empty() && !battle.charm_pick_used;
    let ticket_ok = costs_ok(&[refresh_cost()], bag, 1);
    serde_json::json!({
        "pool": battle.charm_daily_pool.iter().map(|id| charm_line(*id, battle)).collect::<Vec<_>>(),
        "equipped": battle.charm_equipped.iter().map(|id| charm_line(*id, battle)).collect::<Vec<_>>(),
        "poolIds": battle.charm_daily_pool,
        "equipped": battle.charm_equipped,
        "picked": battle.charm_pick_used,
        "canChoose": active && adult && !battle.charm_pick_used && !battle.charm_daily_pool.is_empty(),
        "freeRefreshRemaining": free_remaining,
        "paidRefreshCount": battle.charm_paid_refresh_count,
        "paidRefreshRemaining": paid_remaining,
        "refreshCost": { "id": refresh_cost().0.to_string(), "count": refresh_cost().1, "name": item_name(refresh_cost().0), "known": bag.is_some() },
        "canRefreshFree": active && adult && !needs_choice && free_remaining > 0,
        "canRefreshPaid": active && adult && !needs_choice && free_remaining == 0 && paid_remaining > 0 && ticket_ok,
        "refreshNote": format!("每日免费 {} 次，之后每次 {} 点券，今日还可付费刷新 {paid_remaining} 次。点券不足时不刷新。", free_refresh_limit(), refresh_cost().1),
    })
}

fn shop_rows(
    data: &PetDiaryActivityData,
    bag: Option<&HashMap<i64, i64>>,
    pet_active: bool,
) -> Vec<serde_json::Value> {
    let mut goods = data.shop.clone().unwrap_or_default().goods;
    goods.sort_by_key(|item| item.order);
    let shop_active = head_active(data);
    goods.into_iter().map(|goods| {
        let remaining = if goods.purchase_limit > 0 {
            Some((goods.purchase_limit - goods.purchased_count).max(0).to_string())
        } else {
            None
        };
        let costs = item_pairs_shop(&goods.cost);
        let safe = !costs.is_empty() && goods.diamond_cost_count == 0 && costs.iter().all(|(id, _)| *id != DIAMOND_ID);
        serde_json::json!({
            "id": goods.id.to_string(),
            "name": goods.name,
            "rewards": goods.item.iter().map(|item| serde_json::json!({"id": item.id.to_string(), "count": item.count, "name": item_name(item.id)})).collect::<Vec<_>>(),
            "costs": cost_rows(&costs, bag),
            "remaining": remaining,
            "safeCosts": safe,
            "exchangeable": pet_active && shop_active && safe && remaining.as_deref() != Some("0") && costs_ok(&costs, bag, 1),
        })
    }).collect()
}

fn solar_rows(reply: &GetSolarTermsReply, start: i64, end: i64) -> Vec<serde_json::Value> {
    reply
        .terms
        .iter()
        .filter(|term| term.end_time >= start && term.begin_time <= end)
        .map(|term| {
            serde_json::json!({
                "id": term.term_id.to_string(),
                "name": String::from_utf8_lossy(&term.name),
                "status": term.status,
                "canClaim": term.status == 2,
                "beginTimeText": readable_time(term.begin_time),
                "endTimeText": readable_time(term.end_time),
            })
        })
        .collect()
}

fn treasure_rule_lines(desc: &str) -> (String, Vec<String>) {
    let tips2 = serde_json::from_str::<serde_json::Value>(desc)
        .ok()
        .and_then(|value| value.get("tips2").cloned())
        .unwrap_or_else(|| serde_json::json!({}));
    let wrapped = serde_json::json!({ "tips": tips2 }).to_string();
    crate::services::activity_text::text_content(wrapped.as_bytes())
}

fn charm_line(id: i32, battle: &PetTreasureHuntBattle) -> serde_json::Value {
    let config = charm_config(id);
    let remaining = battle
        .charm_effect_remaining_count
        .iter()
        .filter(|entry| entry.charm_id == id)
        .map(|entry| entry.remaining_count)
        .collect::<Vec<_>>();
    serde_json::json!({
        "id": id,
        "name": config.map(|item| item.name.clone()).unwrap_or_else(|| format!("锦囊 {id}")),
        "description": config.map(|item| item.desc.clone()).unwrap_or_default(),
        "shortDescription": config.map(|item| if item.short_desc.is_empty() { item.desc.clone() } else { item.short_desc.clone() }).unwrap_or_default(),
        "useLimit": config.map(|item| item.use_limit).unwrap_or(0),
        "icon": config.map(|item| item.icon_path.clone()).unwrap_or_default(),
        "remaining": remaining,
    })
}

fn plant_rows(bag: Option<&HashMap<i64, i64>>) -> Vec<serde_json::Value> {
    PLANT_IDS
        .into_iter()
        .map(|id| {
            serde_json::json!({
                "id": id.to_string(),
                "name": item_name(id),
                "count": bag.and_then(|bag| bag.get(&id).copied()).unwrap_or(0).to_string(),
                "known": bag.is_some(),
            })
        })
        .collect()
}

fn readable_time(secs: i64) -> String {
    if secs <= 0 {
        return "未知".to_owned();
    }
    let Some(utc) = chrono::DateTime::from_timestamp(secs, 0) else {
        return "未知".to_owned();
    };
    use chrono::{Datelike as _, Timelike as _};
    let tz: chrono_tz::Tz =
        crate::config::get_time_zone().parse().unwrap_or(chrono_tz::Asia::Shanghai);
    let local = utc.with_timezone(&tz);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        local.year(),
        local.month(),
        local.day(),
        local.hour(),
        local.minute()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::encryptor::NoopEncryptor;
    use crate::network::gateway::{Gateway, GatewayConfig};
    use crate::proto::generated::corepb::ItemBag;
    use crate::proto::generated::gamepb::activitypb::{
        PetDiaryActivityBodyMegaEvent, PetDiaryActivityBodyShop, PetDiaryActivityGroup,
        PetDiaryActivityHead, PetDiaryMegaEventClaimAllRsp, PetDiaryMegaEventRewardState,
        PetDiaryShopGoodsInfo, PetTreasureHuntFeed, PetTreasureHuntHunt, PetTreasureHuntNurture,
        PetTreasureHuntPlunder, PetTreasureHuntPool, PetTreasureHuntStory,
    };
    use crate::proto::generated::gamepb::itempb::BagReply;

    fn gateway() -> Arc<Gateway> {
        let gate = Arc::new(Gateway::new(
            GatewayConfig {
                server_url: "wss://gate.example.com/ws".into(),
                platform: "qq".into(),
                os: "Windows".into(),
                client_version: "1".into(),
                auth_code: "test".into(),
                headers: std::collections::HashMap::new(),
            },
            Arc::new(NoopEncryptor),
        ));
        gate.mark_online();
        gate
    }

    fn head(id: i64, begin: i64, end: i64) -> PetDiaryActivityHead {
        PetDiaryActivityHead {
            id,
            start_time: begin,
            end_time: end,
            desc: "投喂".into(),
            ..PetDiaryActivityHead::default()
        }
    }

    fn group(stage: i32, feed_count: i32, begin: i64) -> Vec<u8> {
        let end = get_server_time_secs() + 3600;
        PetDiaryGetGroupReply {
            group: Some(PetDiaryActivityGroup {
                head: Some(head(GROUP_ID, begin, end)),
                children: vec![
                    PetDiaryActivityData {
                        head: Some(head(PET_ID, begin, end)),
                        pet_treasure_hunt: Some(ActivityBodyPetTreasureHunt {
                            nurture: Some(PetTreasureHuntNurture {
                                stage,
                                ..PetTreasureHuntNurture::default()
                            }),
                            feed: Some(PetTreasureHuntFeed { feed_count }),
                            hunt: Some(PetTreasureHuntHunt {
                                treasure_cost: vec![CoreItem {
                                    id: feed_pair().0,
                                    count: 10,
                                    ..CoreItem::default()
                                }],
                                can_play_plunder: true,
                                ..PetTreasureHuntHunt::default()
                            }),
                            story: Some(PetTreasureHuntStory {
                                stories: vec![PetTreasureHuntStoryInfo {
                                    order: 2,
                                    unlocked: true,
                                    ..PetTreasureHuntStoryInfo::default()
                                }],
                            }),
                            battle: Some(PetTreasureHuntBattle {
                                charm_daily_pool: vec![4],
                                ..PetTreasureHuntBattle::default()
                            }),
                            pool: Some(PetTreasureHuntPool {
                                treasures: vec![PetTreasureHuntTreasure {
                                    id: "t1".into(),
                                    status: 3,
                                    item_id: 1,
                                    count: 1,
                                    ..PetTreasureHuntTreasure::default()
                                }],
                            }),
                            plunder: Some(PetTreasureHuntPlunder {
                                plunder_compensation_count: 1,
                                ..PetTreasureHuntPlunder::default()
                            }),
                            ..ActivityBodyPetTreasureHunt::default()
                        }),
                        ..PetDiaryActivityData::default()
                    },
                    PetDiaryActivityData {
                        head: Some(head(SEEDS_ID, begin, end)),
                        mega_event: Some(PetDiaryActivityBodyMegaEvent {
                            rewards: vec![PetDiaryMegaEventRewardState {
                                unlock_day: 1,
                                claimable: true,
                                reward: vec![CoreItem { id: 9, count: 2, ..CoreItem::default() }],
                                ..PetDiaryMegaEventRewardState::default()
                            }],
                            ..PetDiaryActivityBodyMegaEvent::default()
                        }),
                        ..PetDiaryActivityData::default()
                    },
                ],
            }),
        }
        .encode_to_vec()
    }

    fn bag_bytes() -> Vec<u8> {
        BagReply {
            item_bag: Some(ItemBag {
                items: vec![
                    CoreItem { id: feed_pair().0, count: 800, ..CoreItem::default() },
                    CoreItem {
                        id: refresh_cost().0,
                        count: refresh_cost().1,
                        ..CoreItem::default()
                    },
                    CoreItem { id: 80101, count: 1, ..CoreItem::default() },
                ],
                ..ItemBag::default()
            }),
        }
        .encode_to_vec()
    }

    #[test]
    fn the_pet_window_requires_a_positive_start_and_includes_the_end() {
        let request = with_field(PET_ID, INITIALIZE, |request| {
            request.pet_treasure_hunt_finish_cg = Some(PetTreasureHuntFinishCgReq {});
        });
        assert_eq!(hex::encode(request.encode_to_vec()), "08f5dc8ec607101bf20700");
        assert!(!pet_window_is_open(0, 0, 10));
        assert!(!pet_window_is_open(0, 20, 10));
        assert!(pet_window_is_open(5, 10, 10));
        assert!(!pet_window_is_open(5, 9, 10));
    }

    #[tokio::test]
    async fn a_closed_window_and_an_unknown_bag_do_not_send() {
        let _data = crate::config::paths::TestDataDir::enter();
        let writes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&writes);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            Ok(group(BABY_STAGE, 0, 0))
        });
        assert!(PetDiaryService::new(gate).feed().await.is_err());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 0);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, _body| {
            if method == "Bag" {
                return Err(crate::network::error::NetworkError::Phase("bag".into()));
            }
            Ok(group(BABY_STAGE, 0, 1))
        });
        assert!(PetDiaryService::new(gate).feed().await.is_err());
    }

    #[tokio::test]
    async fn feed_seeds_and_exchange_decode_their_fields_and_a_bad_reply_is_not_a_reward() {
        let _data = crate::config::paths::TestDataDir::enter();
        let bodies = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
        let log = std::sync::Arc::clone(&bodies);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            if method == "GetSolarTerms" {
                return Ok(GetSolarTermsReply::default().encode_to_vec());
            }
            if method == "Operate" {
                log.lock().expect("log").push(body.to_vec());
                let request = PetDiaryOperateRequest::decode(body).unwrap();
                if request.operate_type == FEED {
                    return Ok(PetDiaryOperateReply {
                        activity_id: 1,
                        operate_type: 1,
                        ..PetDiaryOperateReply::default()
                    }
                    .encode_to_vec());
                }
                return Ok(PetDiaryOperateReply {
                    activity_id: request.activity_id,
                    operate_type: request.operate_type,
                    data: Some(PetDiaryActivityData {
                        head: Some(head(SHOP_ID, 1, get_server_time_secs() + 100)),
                        shop: Some(PetDiaryActivityBodyShop {
                            goods: vec![
                                PetDiaryShopGoodsInfo {
                                    id: 8,
                                    item: vec![PetDiaryShopItemInfo { id: 9, count: 1 }],
                                    cost: vec![PetDiaryShopItemInfo { id: feed_pair().0, count: 2 }],
                                    purchase_limit: 3,
                                    name: "种子".into(),
                                    ..PetDiaryShopGoodsInfo::default()
                                },
                                PetDiaryShopGoodsInfo {
                                    id: 9,
                                    diamond_cost_count: 1,
                                    name: "钻石包".into(),
                                    ..PetDiaryShopGoodsInfo::default()
                                },
                            ],
                        }),
                        ..PetDiaryActivityData::default()
                    }),
                    mega_event_claim_all: (request.operate_type == SEEDS).then(|| {
                        PetDiaryMegaEventClaimAllRsp {
                            awards: vec![CoreItem { id: 9, count: 2, ..CoreItem::default() }],
                            ..PetDiaryMegaEventClaimAllRsp::default()
                        }
                    }),
                    shop_buy: (request.operate_type == EXCHANGE).then(|| {
                        crate::proto::generated::gamepb::activitypb::PetDiaryShopBuyRsp {
                            awards: vec![CoreItem { id: 9, count: 1, ..CoreItem::default() }],
                            ..crate::proto::generated::gamepb::activitypb::PetDiaryShopBuyRsp::default()
                        }
                    }),
                    ..PetDiaryOperateReply::default()
                }
                .encode_to_vec());
            }
            Ok(group(BABY_STAGE, 0, 1))
        });
        let service = PetDiaryService::new(gate);
        let error = service.feed().await.unwrap_err();
        assert!(error.to_string().contains("不匹配"), "{error}");
        let claimed = service.claim_seeds_if(|| true).await.unwrap();
        assert_eq!(claimed["rewards"][0]["count"], 2);
        let bought = service.exchange_if(8, 2, || true).await.unwrap();
        assert_eq!(bought["message"], "操作成功");
        assert!(service.exchange_if(9, 1, || true).await.is_err());
        let seen = bodies.lock().expect("log");
        let feed = PetDiaryOperateRequest::decode(seen[0].as_slice()).unwrap();
        assert_eq!(feed.operate_type, FEED);
        assert!(feed.pet_treasure_hunt_feed.is_some());
        let seeds = PetDiaryOperateRequest::decode(seen[1].as_slice()).unwrap();
        assert_eq!((seeds.activity_id, seeds.operate_type), (SEEDS_ID, SEEDS));
        assert!(seeds.mega_event_claim_all.is_some());
        let kinds = seen
            .iter()
            .map(|body| {
                let request = PetDiaryOperateRequest::decode(body.as_slice()).unwrap();
                (request.activity_id, request.operate_type, request.shop_buy.map(|item| item.count))
            })
            .collect::<Vec<_>>();
        assert_eq!(kinds.iter().filter(|item| item.1 == EXCHANGE).count(), 1, "{kinds:?}");
        assert_eq!(kinds.iter().find(|item| item.1 == EXCHANGE).unwrap().2, Some(2), "{kinds:?}");
    }

    fn operate_log() -> (
        PetDiaryService,
        std::sync::Arc<std::sync::Mutex<Vec<PetDiaryOperateRequest>>>,
        std::sync::Arc<std::sync::atomic::AtomicI32>,
        std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
        let stage = std::sync::Arc::new(AtomicI32::new(BABY_STAGE));
        let paid = std::sync::Arc::new(AtomicBool::new(false));
        let seen_stage = std::sync::Arc::clone(&stage);
        let seen_paid = std::sync::Arc::clone(&paid);
        let bodies =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::<PetDiaryOperateRequest>::new()));
        let log = std::sync::Arc::clone(&bodies);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            if method == "GetSolarTerms" {
                return Ok(GetSolarTermsReply {
                    terms: vec![crate::proto::generated::gamepb::solartermspb::SolarTermInfo {
                        term_id: 7,
                        status: 2,
                        begin_time: 1,
                        end_time: get_server_time_secs() + 100,
                        name: "白露".into(),
                        ..Default::default()
                    }],
                    ..GetSolarTermsReply::default()
                }
                .encode_to_vec());
            }
            if method == "ClaimSolarTerms" {
                let request = ClaimSolarTermsRequest::decode(body).unwrap();
                return Ok(ClaimSolarTermsReply {
                    rewards: vec![crate::proto::generated::gamepb::solartermspb::SolarTermReward {
                        item_id: 9,
                        count: 4,
                    }],
                    term: Some(crate::proto::generated::gamepb::solartermspb::SolarTermInfo {
                        term_id: request.term_id,
                        status: 3,
                        ..Default::default()
                    }),
                    ..ClaimSolarTermsReply::default()
                }
                .encode_to_vec());
            }
            if method == "Operate" {
                let request = PetDiaryOperateRequest::decode(body).unwrap();
                log.lock().expect("log").push(request.clone());
                return Ok(scripted_reply(&request).encode_to_vec());
            }
            let mut reply = PetDiaryGetGroupReply::decode(
                group(seen_stage.load(Ordering::SeqCst), 0, 1).as_slice(),
            )
            .unwrap();
            if seen_paid.load(Ordering::SeqCst) {
                reply.group.as_mut().unwrap().children[0]
                    .pet_treasure_hunt
                    .as_mut()
                    .unwrap()
                    .battle
                    .as_mut()
                    .unwrap()
                    .charm_free_refresh_count = free_refresh_limit();
            }
            Ok(reply.encode_to_vec())
        });
        (PetDiaryService::new(gate), bodies, stage, paid)
    }

    fn scripted_reply(request: &PetDiaryOperateRequest) -> PetDiaryOperateReply {
        use crate::proto::generated::gamepb::activitypb::{
            PetTreasureHuntBattlePreview, PetTreasureHuntClaimDogRsp,
            PetTreasureHuntClaimPlunderCompensationRsp, PetTreasureHuntDrawRsp,
            PetTreasureHuntEquipCharmsRsp, PetTreasureHuntFeedRsp,
            PetTreasureHuntGetFriendActivityInfoRsp, PetTreasureHuntGetLogRsp,
            PetTreasureHuntGetPlunderedLogRsp, PetTreasureHuntLogEntry,
            PetTreasureHuntOpenTreasureRsp, PetTreasureHuntPlunderedLogEntry,
            PetTreasureHuntRefreshCharmPoolRsp, PetTreasureHuntStartBattleRsp,
        };
        let mut reply = PetDiaryOperateReply {
            activity_id: request.activity_id,
            operate_type: request.operate_type,
            ..PetDiaryOperateReply::default()
        };
        let reward = CoreItem { id: 9, count: 2, ..CoreItem::default() };
        match request.operate_type {
            FEED => reply.pet_treasure_hunt_feed = Some(PetTreasureHuntFeedRsp::default()),
            DRAW => {
                reply.pet_treasure_hunt_draw = Some(PetTreasureHuntDrawRsp {
                    rewards: vec![reward],
                    ..PetTreasureHuntDrawRsp::default()
                })
            }
            INITIALIZE => {
                reply.pet_treasure_hunt_finish_cg =
                    Some(crate::proto::generated::gamepb::activitypb::PetTreasureHuntFinishCgRsp {})
            }
            STORY => {
                reply.pet_treasure_hunt_claim_story = Some(
                    crate::proto::generated::gamepb::activitypb::PetTreasureHuntClaimStoryRsp {
                        awards: vec![reward],
                        ..Default::default()
                    },
                )
            }
            REFRESH_CHARM => {
                reply.pet_treasure_hunt_refresh_charm_pool =
                    Some(PetTreasureHuntRefreshCharmPoolRsp::default())
            }
            EQUIP_CHARM => {
                reply.pet_treasure_hunt_equip_charms =
                    Some(PetTreasureHuntEquipCharmsRsp::default())
            }
            BATTLE => {
                reply.pet_treasure_hunt_start_battle = Some(PetTreasureHuntStartBattleRsp {
                    won: true,
                    rewards: vec![reward],
                    ..PetTreasureHuntStartBattleRsp::default()
                })
            }
            OPEN_TREASURE => {
                reply.pet_treasure_hunt_open_treasure = Some(PetTreasureHuntOpenTreasureRsp {
                    rewards: vec![reward],
                    ..Default::default()
                })
            }
            COMPENSATION => {
                reply.pet_treasure_hunt_claim_plunder_compensation =
                    Some(PetTreasureHuntClaimPlunderCompensationRsp {
                        rewards: vec![reward],
                        ..Default::default()
                    })
            }
            CLAIM_DOG => {
                reply.pet_treasure_hunt_claim_dog = Some(PetTreasureHuntClaimDogRsp::default())
            }
            SEEDS => {
                reply.mega_event_claim_all = Some(PetDiaryMegaEventClaimAllRsp {
                    awards: vec![reward],
                    ..PetDiaryMegaEventClaimAllRsp::default()
                })
            }
            EXCHANGE => {
                reply.shop_buy =
                    Some(crate::proto::generated::gamepb::activitypb::PetDiaryShopBuyRsp {
                        awards: vec![reward],
                        ..Default::default()
                    })
            }
            INTERACT_LOG => {
                reply.pet_treasure_hunt_get_log = Some(PetTreasureHuntGetLogRsp {
                    logs: vec![PetTreasureHuntLogEntry {
                        ts: 1_700_000_000,
                        r#type: 3,
                        costs: vec![CoreItem {
                            id: feed_pair().0,
                            count: 1,
                            ..CoreItem::default()
                        }],
                        rewards: vec![reward],
                        ..PetTreasureHuntLogEntry::default()
                    }],
                })
            }
            PLUNDER_LOG => {
                reply.pet_treasure_hunt_get_plundered_log =
                    Some(PetTreasureHuntGetPlunderedLogRsp {
                        logs: vec![PetTreasureHuntPlunderedLogEntry {
                            ts: 1_700_000_100,
                            attacker_name: "甲".into(),
                            attacker_won: true,
                            challenge_item_id: 80102,
                            lost_items: vec![CoreItem { id: 9, count: 1, ..CoreItem::default() }],
                            injected_items: vec![CoreItem {
                                id: 8,
                                count: 2,
                                ..CoreItem::default()
                            }],
                            ..PetTreasureHuntPlunderedLogEntry::default()
                        }],
                    })
            }
            FRIEND_INFO => {
                let gid = request
                    .pet_treasure_hunt_get_friend_activity_info
                    .as_ref()
                    .map(|item| item.friend_gid)
                    .unwrap_or(0);
                reply.pet_treasure_hunt_get_friend_activity_info =
                    Some(PetTreasureHuntGetFriendActivityInfoRsp {
                        gid,
                        info: Some(crate::proto::generated::gamepb::activitypb::PetTreasureHuntFriendInfo {
                            treasures: vec![PetTreasureHuntTreasure {
                                id: "box".into(),
                                status: 2,
                                item_id: 9,
                                count: 3,
                                battle_previews: vec![PetTreasureHuntBattlePreview {
                                    challenge_item_id: 80101,
                                    can_start: true,
                                    ..PetTreasureHuntBattlePreview::default()
                                }],
                                ..PetTreasureHuntTreasure::default()
                            }],
                            ..Default::default()
                        }),
                    })
            }
            SHOP_QUERY => {
                reply.data = Some(PetDiaryActivityData {
                    head: Some(head(SHOP_ID, 1, get_server_time_secs() + 100)),
                    shop: Some(PetDiaryActivityBodyShop {
                        goods: vec![PetDiaryShopGoodsInfo {
                            id: 8,
                            item: vec![PetDiaryShopItemInfo { id: 9, count: 1 }],
                            cost: vec![PetDiaryShopItemInfo { id: feed_pair().0, count: 2 }],
                            purchase_limit: 3,
                            name: "种子".into(),
                            ..PetDiaryShopGoodsInfo::default()
                        }],
                    }),
                    ..PetDiaryActivityData::default()
                })
            }
            _ => {}
        }
        reply
    }

    #[test]
    fn the_catalog_limits_are_read_from_the_fixed_source_file() {
        let raw: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/pet-diary-2026090101.json")).unwrap();
        let base = &raw["ActivityPetTreasureHuntBase"][0];
        let fight = &raw["ActivityPetTreasureHuntFight"][0];
        let refresh = &raw["ActivityPetTreasureCharmRefresh"][0];
        assert_eq!(
            format!("{}:{}", feed_pair().0, feed_pair().1),
            base["feed_items"].as_str().unwrap()
        );
        assert_eq!(feed_limit() as i64, base["daily_feed_limit"].as_i64().unwrap());
        assert_eq!(treasure_limit() as i64, base["daily_treasure_limit"].as_i64().unwrap());
        assert_eq!(adult_growth(), base["growth_adult_threshold"].as_i64().unwrap());
        assert_eq!(battle_limit() as i64, fight["daily_battle_limit"].as_i64().unwrap());
        assert_eq!(free_refresh_limit(), refresh["free_refresh_daily_limit"].as_i64().unwrap());
        assert_eq!(paid_refresh_limit(), refresh["manual_refresh_daily_limit"].as_i64().unwrap());
        assert_eq!(refresh_cost().0, refresh["manual_refresh_cost_id"].as_i64().unwrap());
        assert_eq!(refresh_cost().1, refresh["manual_refresh_cost_count"].as_i64().unwrap());
        assert_eq!(base["feed_items"], "1028:700");
        assert!(charm_config(101).is_some_and(|charm| charm.name == "惜糕探宝"));
    }

    #[tokio::test]
    async fn both_rule_sets_treasures_charms_and_plants_reach_the_snapshot() {
        let _data = crate::config::paths::TestDataDir::enter();
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            if method == "GetSolarTerms" {
                return Ok(GetSolarTermsReply::default().encode_to_vec());
            }
            if method == "Operate" {
                let request = PetDiaryOperateRequest::decode(body).unwrap();
                return Ok(scripted_reply(&request).encode_to_vec());
            }
            let mut reply = PetDiaryGetGroupReply::decode(group(BABY_STAGE, 0, 1).as_slice()).unwrap();
            let child = &mut reply.group.as_mut().unwrap().children[0];
            child.head.as_mut().unwrap().desc = r#"{"tips":{"title":"主规则","txt":["主文"]},"tips2":{"title":"护送规则","txt":["护送文"]}}"#.into();
            let hunt = child.pet_treasure_hunt.as_mut().unwrap();
            hunt.hunt.as_mut().unwrap().lucky_star_gained_total = 12;
            hunt.battle.as_mut().unwrap().is_skip_battle_cg = true;
            hunt.battle.as_mut().unwrap().charm_daily_pool = vec![101];
            hunt.story.as_mut().unwrap().stories[0].selected_desc =
                r#"{"photo":"gui/pet/photo","note":"爪印"}"#.into();
            let treasure = &mut hunt.pool.as_mut().unwrap().treasures[0];
            treasure.count = 4;
            treasure.start_at = 10;
            treasure.end_at = 20;
            treasure.plunder_count = 1;
            treasure.max_plunder_count = 3;
            Ok(reply.encode_to_vec())
        });
        let snapshot = PetDiaryService::new(gate).snapshot().await.unwrap();
        assert_eq!(snapshot["rulesTitle"], "主规则");
        assert_eq!(snapshot["rules"][0], "主文");
        assert_eq!(snapshot["treasureRulesTitle"], "护送规则");
        assert_eq!(snapshot["treasureRules"][0], "护送文");
        assert!(!snapshot["rules"].to_string().contains("护送文"));
        assert_eq!(snapshot["hunt"]["luckyStarTotal"], "12");
        assert_eq!(snapshot["skipBattle"], true);
        assert_eq!(snapshot["stories"][0]["photo"], "gui/pet/photo");
        assert_eq!(snapshot["stories"][0]["note"], "爪印");
        assert_eq!(snapshot["charms"]["pool"][0]["name"], "惜糕探宝");
        assert_eq!(snapshot["treasures"][0]["count"], 4);
        assert_eq!(snapshot["treasures"][0]["plunderCount"], 1);
        assert!(snapshot["plants"].as_array().unwrap().len() == 6);
    }

    #[tokio::test]
    async fn initialize_accepts_an_empty_message_and_rejects_a_missing_selector() {
        let _data = crate::config::paths::TestDataDir::enter();
        let (service, log, _stage, _paid) = operate_log();
        let adopted = service.initialize_if(|| true).await.unwrap();
        assert_eq!(adopted["message"], "操作成功");
        let request = log.lock().expect("log")[0].clone();
        assert_eq!(hex::encode(request.encode_to_vec()), "08f5dc8ec607101bf20700");
        assert!(request.pet_treasure_hunt_finish_cg.is_some());
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            if method == "Operate" {
                let request = PetDiaryOperateRequest::decode(body).unwrap();
                return Ok(PetDiaryOperateReply {
                    activity_id: request.activity_id,
                    operate_type: request.operate_type,
                    ..PetDiaryOperateReply::default()
                }
                .encode_to_vec());
            }
            Ok(group(BABY_STAGE, 0, 1))
        });
        let error = PetDiaryService::new(gate).feed().await.unwrap_err();
        assert!(error.to_string().contains("活动响应缺少操作结果"), "{error}");
    }

    #[tokio::test]
    async fn feed_draw_story_seeds_open_and_compensation_send_their_own_fields() {
        let _data = crate::config::paths::TestDataDir::enter();
        let (service, log, stage, _paid) = operate_log();
        let fed = service.feed_if(|| true).await.unwrap();
        assert_eq!(fed["message"], "操作成功");
        stage.store(ADULT_STAGE, std::sync::atomic::Ordering::SeqCst);
        let drawn = service.draw_if(|| true).await.unwrap();
        assert_eq!(drawn["rewards"][0]["count"], 2);
        let story = service.claim_story_if(2, || true).await.unwrap();
        assert_eq!(story["rewards"][0]["id"], "9");
        assert!(service
            .claim_story_if(9, || true)
            .await
            .unwrap_err()
            .to_string()
            .contains("尚未解锁"));
        let seeds = service.claim_seeds_if(|| true).await.unwrap();
        assert_eq!(seeds["rewards"][0]["count"], 2);
        let opened = service.open_treasure_if(|| true).await.unwrap();
        assert_eq!(opened["rewards"][0]["count"], 2);
        let paid_back = service.claim_compensation_if(|| true).await.unwrap();
        assert_eq!(paid_back["message"], "操作成功");
        let seen = log.lock().expect("log");
        let feed = seen.iter().find(|item| item.operate_type == FEED).unwrap();
        assert!(feed.pet_treasure_hunt_feed.is_some());
        let draw = seen.iter().find(|item| item.operate_type == DRAW).unwrap();
        assert!(draw.pet_treasure_hunt_draw.is_some());
        let story_request = seen.iter().find(|item| item.operate_type == STORY).unwrap();
        assert_eq!(story_request.pet_treasure_hunt_claim_story.as_ref().unwrap().order, 2);
        let seeds_request = seen.iter().find(|item| item.operate_type == SEEDS).unwrap();
        assert_eq!(seeds_request.activity_id, SEEDS_ID);
        assert!(seeds_request.mega_event_claim_all.is_some());
        assert!(seen.iter().any(|item| item.operate_type == OPEN_TREASURE
            && item.pet_treasure_hunt_open_treasure.is_some()));
        assert!(seen.iter().any(|item| item.operate_type == COMPENSATION
            && item.pet_treasure_hunt_claim_plunder_compensation.is_some()));
        assert!(seen.iter().all(|item| item.operate_type != STORY
            || item.pet_treasure_hunt_claim_story.as_ref().is_some_and(|story| story.order == 2)));
    }

    #[tokio::test]
    async fn charm_refresh_uses_free_then_tickets_and_never_diamonds() {
        let _data = crate::config::paths::TestDataDir::enter();
        let (service, log, stage, paid) = operate_log();
        stage.store(ADULT_STAGE, std::sync::atomic::Ordering::SeqCst);
        assert!(service
            .refresh_charm_if("diamond", None, || true)
            .await
            .unwrap_err()
            .to_string()
            .contains("不支持使用钻石"));
        assert!(log.lock().expect("log").is_empty());
        let free = service.refresh_charm_if("free", None, || true).await.unwrap();
        assert_eq!(free["message"], "操作成功");
        let wrong = service.refresh_charm_if("tickets", Some(0), || true).await.unwrap_err();
        assert!(wrong.to_string().contains("刷新次数已变化"), "{wrong}");
        paid.store(true, std::sync::atomic::Ordering::SeqCst);
        let tickets = service.refresh_charm_if("tickets", Some(0), || true).await.unwrap();
        assert_eq!(tickets["message"], "操作成功");
        let seen = log.lock().expect("log");
        let refreshes =
            seen.iter().filter(|item| item.operate_type == REFRESH_CHARM).collect::<Vec<_>>();
        assert_eq!(refreshes.len(), 2);
        assert!(refreshes.iter().all(|item| item.pet_treasure_hunt_refresh_charm_pool.is_some()));
        assert!(seen
            .iter()
            .all(|item| item.operate_type == REFRESH_CHARM || item.operate_type == SHOP_QUERY));
    }

    #[tokio::test]
    async fn equip_dog_and_exchange_quantity_follow_the_source_limits() {
        let _data = crate::config::paths::TestDataDir::enter();
        let (service, log, stage, _paid) = operate_log();
        stage.store(ADULT_STAGE, std::sync::atomic::Ordering::SeqCst);
        let dog = service.claim_dog_if(|| true).await.unwrap();
        assert_eq!(dog["message"], "操作成功");
        let equipped = service.equip_charm_if(4, || true).await.unwrap();
        assert_eq!(equipped["message"], "操作成功");
        let bought = service.exchange_if(8, 2, || true).await.unwrap();
        assert_eq!(bought["message"], "操作成功");
        let over = service.exchange_if(8, 4, || true).await.unwrap_err();
        assert!(over.to_string().contains("超过剩余限购"), "{over}");
        let seen = log.lock().expect("log");
        let exchange = seen.iter().find(|item| item.operate_type == EXCHANGE).unwrap();
        assert_eq!(exchange.shop_buy.as_ref().unwrap().count, 2);
        assert_eq!(exchange.shop_buy.as_ref().unwrap().goods_id, 8);
        assert!(seen.iter().filter(|item| item.operate_type == EXCHANGE).count() == 1);
        assert!(seen.iter().any(
            |item| item.operate_type == CLAIM_DOG && item.pet_treasure_hunt_claim_dog.is_some()
        ));
        assert_eq!(
            seen.iter()
                .find(|item| item.operate_type == EQUIP_CHARM)
                .unwrap()
                .pet_treasure_hunt_equip_charms
                .as_ref()
                .unwrap()
                .charm_ids,
            vec![4]
        );
    }

    #[tokio::test]
    async fn friend_query_must_match_before_battle_and_a_wrong_treasure_does_not_send_43() {
        let _data = crate::config::paths::TestDataDir::enter();
        let (service, log, _stage, _paid) = operate_log();
        assert!(service.battle_if(0, "box", 80101, || true).await.is_err());
        assert!(log.lock().expect("log").is_empty());
        let missed = service.battle_if(9, "missing", 80101, || true).await.unwrap_err();
        assert!(missed.to_string().contains("好友宝藏状态已变化"), "{missed}");
        let kinds =
            log.lock().expect("log").iter().map(|item| item.operate_type).collect::<Vec<_>>();
        assert_eq!(kinds, vec![FRIEND_INFO]);
        let won = service.battle_if(9, "box", 80101, || true).await.unwrap();
        assert_eq!(won["message"], "夺宝成功");
        let seen = log.lock().expect("log");
        let friend = seen.iter().find(|item| item.operate_type == FRIEND_INFO).unwrap();
        assert_eq!(
            friend.pet_treasure_hunt_get_friend_activity_info.as_ref().unwrap().friend_gid,
            9
        );
        let battle = seen.iter().find(|item| item.operate_type == BATTLE).unwrap();
        let body = battle.pet_treasure_hunt_start_battle.as_ref().unwrap();
        assert_eq!(
            (body.defender_gid, body.treasure_id.as_str(), body.challenge_item_id),
            (9, "box", 80101)
        );
        assert!(service
            .battle_if(9, "box", 80000, || true)
            .await
            .unwrap_err()
            .to_string()
            .contains("挑战书无效"));
    }

    #[tokio::test]
    async fn interact_and_plunder_logs_keep_the_source_fields() {
        let _data = crate::config::paths::TestDataDir::enter();
        let (service, log, _stage, _paid) = operate_log();
        let interact = service.interact_log_if(|| true).await.unwrap();
        assert_eq!(interact["logs"][0]["type"], 3);
        assert_eq!(interact["logs"][0]["costs"][0]["count"], 1);
        let plunder = service.plunder_log_if(|| true).await.unwrap();
        assert_eq!(plunder["plunderLogs"][0]["won"], true);
        assert_eq!(plunder["plunderLogs"][0]["challengeId"], "80102");
        assert_eq!(plunder["plunderLogs"][0]["lost"][0]["count"], 1);
        assert_eq!(plunder["plunderLogs"][0]["injected"][0]["count"], 2);
        let seen = log.lock().expect("log");
        assert!(seen
            .iter()
            .any(|item| item.operate_type == INTERACT_LOG
                && item.pet_treasure_hunt_get_log.is_some()));
        assert!(seen.iter().any(|item| item.operate_type == PLUNDER_LOG
            && item.pet_treasure_hunt_get_plundered_log.is_some()));
    }

    #[tokio::test]
    async fn solar_claim_uses_v1_and_checks_the_returned_term() {
        let _data = crate::config::paths::TestDataDir::enter();
        let (service, _log, _stage, _paid) = operate_log();
        let claimed = service.claim_solar_if(7, || true).await.unwrap();
        assert_eq!(claimed["message"], "节令好礼领取成功");
        assert_eq!(claimed["rewards"][0]["count"], 4);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, _body| {
            if method == "GetSolarTerms" {
                return Ok(GetSolarTermsReply {
                    terms: vec![crate::proto::generated::gamepb::solartermspb::SolarTermInfo {
                        term_id: 7,
                        status: 2,
                        begin_time: 1,
                        end_time: get_server_time_secs() + 100,
                        ..Default::default()
                    }],
                    ..GetSolarTermsReply::default()
                }
                .encode_to_vec());
            }
            if method == "ClaimSolarTerms" {
                return Ok(ClaimSolarTermsReply {
                    term: Some(crate::proto::generated::gamepb::solartermspb::SolarTermInfo {
                        term_id: 7,
                        status: 2,
                        ..Default::default()
                    }),
                    ..ClaimSolarTermsReply::default()
                }
                .encode_to_vec());
            }
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            Ok(group(BABY_STAGE, 0, 1))
        });
        let error = PetDiaryService::new(gate).claim_solar_if(7, || true).await.unwrap_err();
        assert!(error.to_string().contains("节令回包不匹配"), "{error}");
    }

    #[tokio::test]
    async fn a_timeout_is_not_replayed_and_refresh_failure_keeps_the_result() {
        let _data = crate::config::paths::TestDataDir::enter();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&calls);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, _body| {
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Err(crate::network::error::NetworkError::Phase("timeout".into()));
            }
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            Ok(group(BABY_STAGE, 0, 1))
        });
        assert!(PetDiaryService::new(gate).feed().await.is_err());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&calls);
        let gate = gateway();
        gate.install_reply_script(move |_service, method, body| {
            if method == "Operate" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let request = PetDiaryOperateRequest::decode(body).unwrap();
                return Ok(scripted_reply(&request).encode_to_vec());
            }
            if method == "Bag" {
                return Ok(bag_bytes());
            }
            if method == "GetSolarTerms" {
                return Ok(GetSolarTermsReply::default().encode_to_vec());
            }
            if count.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                return Err(crate::network::error::NetworkError::Phase("token=abc".into()));
            }
            Ok(group(BABY_STAGE, 0, 1))
        });
        let fed = PetDiaryService::new(gate).feed().await.unwrap();
        assert_eq!(fed["message"], "操作成功");
        assert_eq!(fed["refreshRequired"], true);
        let refresh = fed["refreshError"].as_str().unwrap();
        assert!(refresh.contains("操作已成功，刷新失败"), "{refresh}");
        assert!(refresh.contains("内部错误"), "{refresh}");
        assert!(!refresh.to_ascii_lowercase().contains("token"), "{refresh}");
        assert!(!refresh.contains("abc"), "{refresh}");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn captured_vectors_decode_strictly_and_the_empty_error_reply_stays_empty() {
        let diary = serde_json::from_str::<Vec<serde_json::Value>>(include_str!(
            "fixtures/pet-diary-capture.json"
        ))
        .unwrap();
        let charms = serde_json::from_str::<Vec<serde_json::Value>>(include_str!(
            "fixtures/pet-charm-refresh-capture.json"
        ))
        .unwrap();
        let treasures = serde_json::from_str::<Vec<serde_json::Value>>(include_str!(
            "fixtures/pet-treasure-capture.json"
        ))
        .unwrap();
        let official = serde_json::from_str::<Vec<serde_json::Value>>(include_str!(
            "fixtures/pet-diary-official-vectors.json"
        ))
        .unwrap();
        assert_eq!((diary.len(), charms.len(), treasures.len(), official.len()), (16, 4, 6, 17));
        let mut missing = Vec::new();
        let mut bad = Vec::new();
        for row in diary.iter().chain(charms.iter()).chain(treasures.iter()) {
            let file = row["file"].as_str().unwrap_or("unknown");
            let hex = row["hex"].as_str().unwrap_or("");
            let error = row["error"].as_str().unwrap_or("0");
            if file == "000310-recv.bin" {
                assert_eq!(error, "1000019");
                assert!(hex.is_empty(), "{file}");
                missing.push(file.to_owned());
                continue;
            }
            let bytes = hex::decode(hex).unwrap_or_default();
            if bytes.is_empty() {
                bad.push(format!("{file}: hex 为空"));
                continue;
            }
            let decoded = if row["direction"] == "send" {
                PetDiaryOperateRequest::decode(bytes.as_slice())
                    .map(|request| request.encode_to_vec())
            } else if error == "0" {
                PetDiaryOperateReply::decode(bytes.as_slice()).map(|reply| reply.encode_to_vec())
            } else {
                bad.push(format!("{file}: error {error}"));
                continue;
            };
            match decoded {
                Ok(again) if again == bytes => {}
                Ok(_) => bad.push(format!("{file}: 重编码与抓包字节不一致")),
                Err(err) => bad.push(format!("{file}: 解码失败 {err}")),
            }
        }
        assert_eq!(missing, vec!["000310-recv.bin".to_owned()]);
        let mut synthetic = Vec::new();
        for row in &official {
            let name = row["name"].as_str().unwrap_or("synthetic");
            let bytes = hex::decode(row["hex"].as_str().unwrap_or("")).unwrap_or_default();
            let decoded = if row["direction"] == "recv" {
                PetDiaryOperateReply::decode(bytes.as_slice()).map(|reply| reply.encode_to_vec())
            } else {
                PetDiaryOperateRequest::decode(bytes.as_slice())
                    .map(|request| request.encode_to_vec())
            };
            match decoded {
                Ok(again) if again == bytes => {}
                Ok(_) => synthetic.push(format!("{name}: 合成向量重编码不一致")),
                Err(err) => synthetic.push(format!("{name}: 合成向量解码失败 {err}")),
            }
        }
        let solar = serde_json::from_str::<serde_json::Value>(include_str!(
            "fixtures/pet-solar-claim-capture.json"
        ))
        .unwrap();
        let solar_bytes = hex::decode(solar["hex"].as_str().unwrap()).unwrap();
        let solar_reply = ClaimSolarTermsReply::decode(solar_bytes.as_slice()).expect("节令抓包");
        assert_eq!(solar_reply.encode_to_vec(), solar_bytes);
        let mall = serde_json::from_str::<serde_json::Value>(include_str!(
            "fixtures/pet-mall-capture.json"
        ))
        .unwrap();
        let mall_bytes = hex::decode(mall["hex"].as_str().unwrap()).unwrap();
        let mall_reply =
            crate::proto::generated::gamepb::mallpb::GetMallListBySlotTypeResponse::decode(
                mall_bytes.as_slice(),
            );
        if let Err(err) = mall_reply {
            bad.push(format!("pet-mall-capture.json: 解码失败 {err}"));
        }
        assert!(bad.is_empty(), "坏向量:\n{}", bad.join("\n"));
        assert!(synthetic.is_empty(), "合成编码参考（不是实机抓包）:\n{}", synthetic.join("\n"));
    }
}
