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
const TICKET_ID: i64 = 1002;
const FEED_ITEM: i64 = 1028;
const FEED_COST: i64 = 700;
const FEED_LIMIT: i32 = 16;
const TREASURE_LIMIT: i32 = 10;
const BATTLE_LIMIT: i32 = 20;
const ADULT_GROWTH: i64 = 7000;
const ADULT_STAGE: i32 = 2;
const BABY_STAGE: i32 = 1;
const FREE_REFRESH_LIMIT: i64 = 1;
const PAID_REFRESH_LIMIT: i64 = 3;
const REFRESH_COST: i64 = 30;
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
            if nurture.stage != BABY_STAGE || feed.feed_count >= FEED_LIMIT {
                return Err(Error::Business("当前不可投喂".into()));
            }
            if !costs_ok(&[(FEED_ITEM, FEED_COST)], bag, 1) {
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
            if nurture.stage != ADULT_STAGE || hunt.treasure_count >= TREASURE_LIMIT {
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
            let free = battle.charm_free_refresh_count < FREE_REFRESH_LIMIT;
            if free {
                if payment != "free" {
                    return Err(Error::Business("刷新次数已变化，请刷新状态后重试".into()));
                }
            } else {
                if payment != "tickets" {
                    return Err(Error::Business("免费刷新已用完，请确认点券费用后再操作".into()));
                }
                if battle.charm_paid_refresh_count >= PAID_REFRESH_LIMIT {
                    return Err(Error::Business("今日付费刷新次数已用完".into()));
                }
                if expected_paid != Some(battle.charm_paid_refresh_count) {
                    return Err(Error::Business("刷新次数已变化，请刷新状态后重试".into()));
                }
                if !costs_ok(&[(TICKET_ID, REFRESH_COST)], bag, 1) {
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
        if !hunt.can_play_plunder || battle.battle_count >= BATTLE_LIMIT {
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
        self.attach_snapshot(&mut payload).await;
        Ok(payload)
    }

    async fn attach_snapshot(&self, payload: &mut serde_json::Value) {
        match self.snapshot().await {
            Ok(snapshot) => payload["snapshot"] = snapshot,
            Err(_) => {
                payload["refreshRequired"] = serde_json::json!(true);
                payload["refreshError"] = serde_json::json!("操作已成功，刷新失败");
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
        let feed_costs = vec![(FEED_ITEM, FEED_COST)];
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
            "warnings": warnings,
            "balances": balance_rows(bag),
            "nurture": {
                "initialized": nurture.cg_played,
                "adult": adult,
                "growth": nurture.growth,
                "adultGrowth": ADULT_GROWTH,
                "dogGranted": nurture.dog_granted,
                "feedCount": feed.feed_count,
                "feedLimit": FEED_LIMIT,
                "feedCosts": cost_rows(&feed_costs, bag),
                "canFeed": active && !adult && nurture.stage == BABY_STAGE && feed.feed_count < FEED_LIMIT && costs_ok(&feed_costs, bag, 1),
            },
            "hunt": {
                "count": hunt.treasure_count,
                "limit": TREASURE_LIMIT,
                "total": hunt.treasure_total.to_string(),
                "costs": cost_rows(&draw_costs, bag),
                "canDraw": active && adult && hunt.treasure_count < TREASURE_LIMIT && costs_ok(&draw_costs, bag, 1),
                "canPlunder": active && hunt.can_play_plunder && battle.battle_count < BATTLE_LIMIT,
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
            "battleLimit": BATTLE_LIMIT,
            "skipBattle": battle.is_skip_battle_cg,
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
    serde_json::json!({
        "order": story.order,
        "unlocked": story.unlocked,
        "claimed": story.claimed,
        "animated": story.animated,
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
        "endTimeText": readable_time(treasure.end_at),
    })
}

fn charm_rows(
    battle: &PetTreasureHuntBattle,
    bag: Option<&HashMap<i64, i64>>,
    active: bool,
    adult: bool,
) -> serde_json::Value {
    let free_remaining = (FREE_REFRESH_LIMIT - battle.charm_free_refresh_count).max(0);
    let paid_remaining = (PAID_REFRESH_LIMIT - battle.charm_paid_refresh_count).max(0);
    let needs_choice = !battle.charm_equipped.is_empty() && !battle.charm_pick_used;
    let ticket_ok = costs_ok(&[(TICKET_ID, REFRESH_COST)], bag, 1);
    serde_json::json!({
        "pool": battle.charm_daily_pool,
        "equipped": battle.charm_equipped,
        "picked": battle.charm_pick_used,
        "canChoose": active && adult && !battle.charm_pick_used && !battle.charm_daily_pool.is_empty(),
        "freeRefreshRemaining": free_remaining,
        "paidRefreshCount": battle.charm_paid_refresh_count,
        "paidRefreshRemaining": paid_remaining,
        "refreshCost": { "id": TICKET_ID.to_string(), "count": REFRESH_COST, "name": item_name(TICKET_ID), "known": bag.is_some() },
        "canRefreshFree": active && adult && !needs_choice && free_remaining > 0,
        "canRefreshPaid": active && adult && !needs_choice && free_remaining == 0 && paid_remaining > 0 && ticket_ok,
        "refreshNote": format!("每日免费 {FREE_REFRESH_LIMIT} 次，之后每次 {REFRESH_COST} 点券，今日还可付费刷新 {paid_remaining} 次。点券不足时不刷新。"),
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
                                    id: FEED_ITEM,
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
                    CoreItem { id: FEED_ITEM, count: 800, ..CoreItem::default() },
                    CoreItem { id: TICKET_ID, count: 30, ..CoreItem::default() },
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
                                    cost: vec![PetDiaryShopItemInfo { id: FEED_ITEM, count: 2 }],
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
                    mega_event_claim_all: Some(PetDiaryMegaEventClaimAllRsp {
                        awards: vec![CoreItem { id: 9, count: 2, ..CoreItem::default() }],
                        ..PetDiaryMegaEventClaimAllRsp::default()
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
}
