//! 牧场巡查。进入牧场后按开关收获、生产、清理、出售、升级、放养幼崽、补充牧草和罐头，并喂养，然后拜访好友。做满大约 5 秒，或农场、好友巡查已经到期，就先离开。购买、补草、升级和好友拜访会在中途停下，稍后再从剩下的步骤继续。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;
use prost::Message;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::Item as CoreItem;
use crate::proto::generated::gamepb::animalpb::{
    CleanAllRanchReply, FeedAnimalReply, HarvestRanchReply, PlaceAnimalReply, TriggerProduceReply,
};
use crate::proto::generated::gamepb::homesteadpb::{
    AddFeedReply, AllCellsReply, CompleteBuildingUpgradeReply, CompleteBuildingUpgradeRequest,
    FacilityInfo, GetFacilityDetailReply, GetFacilityDetailRequest, UpgradeBuildingReply,
    UpgradeBuildingRequest,
};
use crate::proto::generated::gamepb::ranchpb::{
    GetBagReply, Item as RanchItem, SellReply, UseReply, UseRequest,
};
use crate::proto::generated::gamepb::shoppb::{BuyGoodsReply, BuyGoodsRequest};

const CHECK_MIN_MINUTES: i64 = 1;
const CHECK_MAX_MINUTES: i64 = 60;
const CHECK_DEFAULT_MINUTES: i64 = 5;
const RETRY_FLOOR_MS: i64 = 10 * 60 * 1000;
const GRASS_THRESHOLD_MIN: i64 = 1;
const GRASS_THRESHOLD_MAX: i64 = 100;
const GRASS_THRESHOLD_DEFAULT: i64 = 20;
const GRASS_PRICE_ID: i64 = 1007;
const PATROL_SLICE_MS: i64 = 5_000;
const SCENE_REENTRY_MS: i64 = 20_000;
const CHANGED_RECHECK_MS: i64 = 30_000;
const POOP_EXCHANGE_FLOOR: i64 = 3;
const MARKET_CHECK_MS: i64 = 6 * 60 * 60 * 1000;

#[derive(Clone, Copy)]
struct PatrolFlags {
    harvest: bool,
    produce: bool,
    clean: bool,
    sell: bool,
    market_organic: bool,
    upgrade: bool,
    buy: bool,
    grass: bool,
    grass_buy: bool,
    fill_cans: bool,
    feed_normal: bool,
    feed_organic: bool,
    friends: crate::services::ranch_friend::FriendFlags,
}

impl PatrolFlags {
    fn any(self) -> bool {
        self.harvest
            || self.produce
            || self.clean
            || self.sell
            || self.market_organic
            || self.upgrade
            || self.buy
            || self.grass
            || self.fill_cans
            || self.feed_normal
            || self.feed_organic
            || self.friends.any()
    }
}

fn next_checks() -> &'static Mutex<HashMap<String, i64>> {
    static NEXT: std::sync::OnceLock<Mutex<HashMap<String, i64>>> = std::sync::OnceLock::new();
    NEXT.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 到了巡查时间就做一轮自家牧场操作。开关全关、牧场未解锁或还没到时间时直接返回。
pub async fn patrol_if_due<F, Fut>(
    gateway: &Arc<Gateway>,
    account_id: &str,
    self_gid: i64,
    farm_deadline_ms: i64,
    load_friends: F,
) -> Result<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Vec<crate::services::ranch_friend::RanchFriend>> + Send,
{
    let automation = crate::models::store::account_config::get_automation(Some(account_id));
    let flags = PatrolFlags {
        harvest: automation.ranch_harvest,
        produce: automation.ranch_produce,
        clean: automation.ranch_clean,
        sell: automation.ranch_sell,
        market_organic: automation.ranch_market_organic,
        upgrade: automation.ranch_upgrade,
        buy: automation.ranch_buy,
        grass: automation.ranch_grass,
        grass_buy: automation.ranch_grass && automation.ranch_grass_buy,
        fill_cans: automation.ranch_fill_cans,
        feed_normal: automation.ranch_feed_normal,
        feed_organic: automation.ranch_feed_organic,
        friends: crate::services::ranch_friend::FriendFlags {
            steal: automation.ranch_friend_steal,
            stroke: automation.ranch_stroke,
            clean: automation.ranch_friend_clean,
            mosquito: automation.ranch_friend_mosquito,
        },
    };
    let now = crate::utils::time::now_ms();
    let unlocked = gateway.scene_unlocked(super::RANCH_SCENE);
    if !flags.any() || !unlocked {
        clear_cycle(account_id);
        return Ok(());
    }
    let next_check_at = next_checks().lock().get(account_id).copied().unwrap_or(0);
    if !should_patrol(true, true, next_check_at, now) {
        return Ok(());
    }
    let delay = check_delay_ms(automation.ranch_check_minutes);
    set_next(account_id, now.saturating_add(delay));
    if let Err(error) = patrol(
        gateway,
        account_id,
        self_gid,
        flags,
        cub_plan(&automation),
        automation.ranch_grass_threshold_percent,
        farm_deadline_ms,
        load_friends,
    )
    .await
    {
        clear_cycle(account_id);
        set_next(account_id, now.saturating_add(delay.max(RETRY_FLOOR_MS)));
        if matches!(error, Error::Protocol(_) | Error::Protobuf(_)) {
            tracing::error!(account_id, error = %error, "牧场巡查结果未能确认");
        } else {
            tracing::warn!(account_id, error = %error, "牧场巡查失败，稍后重试");
        }
        return Err(error);
    }
    match finish_visit(account_id) {
        VisitEnd::Yielded => {
            set_next(account_id, crate::utils::time::now_ms().saturating_add(SCENE_REENTRY_MS));
            tracing::info!(account_id, "牧场巡查先让路，稍后再继续");
            crate::services::panel_log::log(
                account_id,
                "牧场",
                "牧场巡查先让路，稍后再继续",
                crate::constants::PanelEvent::RanchAutomation,
                Some(serde_json::json!({ "module": "ranch" })),
            );
        }
        VisitEnd::Finished { started, changed, next_action } => {
            set_next(
                account_id,
                follow_up_at(crate::utils::time::now_ms(), started, delay, next_action, changed),
            );
        }
    }
    Ok(())
}

fn set_next(account_id: &str, at: i64) {
    next_checks().lock().insert(account_id.to_owned(), at);
}

fn should_patrol(enabled: bool, unlocked: bool, next_check_at: i64, now: i64) -> bool {
    enabled && unlocked && now >= next_check_at
}

#[derive(Clone, Copy)]
struct PhaseDone {
    harvest: bool,
    produce: bool,
    clean: bool,
    sell: bool,
    market_organic: bool,
    upgrade: bool,
    buy: bool,
    grass: bool,
    cans: bool,
    feed_normal: bool,
    feed_organic: bool,
    friends: bool,
}

struct Cycle {
    done: PhaseDone,
    yielded: bool,
    acted: bool,
    started_ms: i64,
    deadline_ms: i64,
    friend_visits: u8,
    pending_friend: i64,
    cycle_started_ms: i64,
    changed: bool,
    next_action_ms: i64,
}

enum VisitEnd {
    Yielded,
    Finished { started: i64, changed: bool, next_action: i64 },
}

fn cycles() -> &'static Mutex<HashMap<String, Cycle>> {
    static CYCLES: std::sync::OnceLock<Mutex<HashMap<String, Cycle>>> = std::sync::OnceLock::new();
    CYCLES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn initial_done(flags: PatrolFlags) -> PhaseDone {
    PhaseDone {
        harvest: !flags.harvest,
        produce: !flags.produce,
        clean: !flags.clean,
        sell: !flags.sell,
        market_organic: !flags.market_organic,
        upgrade: !flags.upgrade,
        buy: !flags.buy,
        grass: !flags.grass,
        cans: !flags.fill_cans,
        feed_normal: !flags.feed_normal,
        feed_organic: !flags.feed_organic,
        friends: !flags.friends.any(),
    }
}

fn begin_visit(account_id: &str, flags: PatrolFlags, deadline_ms: i64) -> PhaseDone {
    let now = crate::utils::time::now_ms();
    let mut cycles = cycles().lock();
    if let Some(cycle) = cycles.get_mut(account_id) {
        cycle.started_ms = now;
        cycle.deadline_ms = deadline_ms;
        cycle.acted = false;
        cycle.yielded = false;
        return cycle.done;
    }
    let done = initial_done(flags);
    cycles.insert(
        account_id.to_owned(),
        Cycle {
            done,
            yielded: false,
            acted: false,
            started_ms: now,
            deadline_ms,
            friend_visits: 0,
            pending_friend: 0,
            cycle_started_ms: now,
            changed: false,
            next_action_ms: i64::MAX,
        },
    );
    done
}

fn current_done(account_id: &str, fallback: PhaseDone) -> PhaseDone {
    cycles().lock().get(account_id).map(|cycle| cycle.done).unwrap_or(fallback)
}

fn mark_done(account_id: &str, done: PhaseDone) {
    if let Some(cycle) = cycles().lock().get_mut(account_id) {
        cycle.done = done;
    }
}

fn note_acted(account_id: &str, changed: bool) {
    if let Some(cycle) = cycles().lock().get_mut(account_id) {
        cycle.acted = true;
        cycle.changed |= changed;
    }
}

fn hold(account_id: &str) -> bool {
    let now = crate::utils::time::now_ms();
    let mut cycles = cycles().lock();
    let Some(cycle) = cycles.get_mut(account_id) else {
        return false;
    };
    if should_yield(cycle.acted, cycle.started_ms, now, cycle.deadline_ms) {
        cycle.yielded = true;
        true
    } else {
        false
    }
}

fn friend_progress(account_id: &str) -> (u8, i64) {
    cycles()
        .lock()
        .get(account_id)
        .map(|cycle| (cycle.friend_visits, cycle.pending_friend))
        .unwrap_or((0, 0))
}

fn remember_friends(account_id: &str, visits: u8, pending: i64) {
    if let Some(cycle) = cycles().lock().get_mut(account_id) {
        cycle.friend_visits = visits;
        cycle.pending_friend = pending;
    }
}

fn finish_visit(account_id: &str) -> VisitEnd {
    let mut cycles = cycles().lock();
    let Some(cycle) = cycles.get_mut(account_id) else {
        return VisitEnd::Finished {
            started: crate::utils::time::now_ms(),
            changed: false,
            next_action: i64::MAX,
        };
    };
    if cycle.yielded {
        cycle.yielded = false;
        VisitEnd::Yielded
    } else {
        let end = VisitEnd::Finished {
            started: cycle.cycle_started_ms,
            changed: cycle.changed,
            next_action: cycle.next_action_ms,
        };
        cycles.remove(account_id);
        end
    }
}

fn clear_cycle(account_id: &str) {
    cycles().lock().remove(account_id);
}

/// 人工核对扣费记录后，丢掉本轮进度并允许下一次定时巡查马上开始。
pub fn reset_patrol_schedule(account_id: &str) {
    clear_cycle(account_id);
    set_next(account_id, 0);
}

fn should_yield(acted_this_visit: bool, started_ms: i64, now_ms: i64, deadline_ms: i64) -> bool {
    acted_this_visit && now_ms >= started_ms.saturating_add(PATROL_SLICE_MS).min(deadline_ms)
}

fn follow_up_at(
    now: i64,
    cycle_started: i64,
    check_delay: i64,
    next_action: i64,
    changed: bool,
) -> i64 {
    let cycle_end = cycle_started.saturating_add(check_delay);
    let soon = if changed { now.saturating_add(CHANGED_RECHECK_MS) } else { i64::MAX };
    now.saturating_add(SCENE_REENTRY_MS).max(cycle_end.min(next_action).min(soon))
}

fn produce_ready(cells: &AllCellsReply) -> bool {
    cells
        .facilities
        .iter()
        .filter_map(|facility| facility.ranch.as_ref())
        .any(|ranch| ranch.animals.iter().any(|animal| animal.can_deliver))
}

fn sync_board(account_id: &str, flags: PatrolFlags, cells: &AllCellsReply) {
    if let Some(cycle) = cycles().lock().get_mut(account_id) {
        if flags.harvest && super::harvest_ready(cells) {
            cycle.done.harvest = false;
        }
        if flags.produce && produce_ready(cells) {
            cycle.done.produce = false;
        }
        cycle.next_action_ms = next_ready_at(cells, flags);
    }
}

fn next_ready_at(cells: &AllCellsReply, flags: PatrolFlags) -> i64 {
    let server_now = crate::utils::time::get_server_time_secs().saturating_mul(1000);
    let local_now = crate::utils::time::now_ms();
    let mut soonest = i64::MAX;
    for facility in &cells.facilities {
        if flags.upgrade && facility.upgrade_state != 0 && facility.upgrade_end_time > 0 {
            let end = super::upgrade_end_ms(facility.upgrade_end_time);
            if end > server_now {
                soonest = soonest.min(local_now.saturating_add(end - server_now));
            }
        }
        if !(flags.harvest || flags.produce) {
            continue;
        }
        let Some(ranch) = facility.ranch.as_ref() else {
            continue;
        };
        for animal in &ranch.animals {
            for at in [
                animal.infant_ready_at_ms,
                animal.grow_ready_at_ms,
                animal.produce_ready_at_ms,
                animal.mature_ready_at_ms,
            ] {
                if at > server_now {
                    soonest = soonest.min(local_now.saturating_add(at - server_now));
                }
            }
        }
    }
    soonest
}

#[derive(Clone, Copy)]
struct CubPlan {
    bag_first: bool,
    prefer: bool,
    coop: i64,
    shed: i64,
}

fn cub_plan(automation: &crate::models::types::AutomationConfig) -> CubPlan {
    use crate::models::types::RanchStrategy;
    let prefer_fallback = matches!(automation.ranch_bag_fallback, RanchStrategy::Preferred);
    let (bag_first, prefer) = match automation.ranch_strategy {
        RanchStrategy::BagPriority => (true, prefer_fallback),
        RanchStrategy::Preferred => (false, true),
        RanchStrategy::Level => (false, false),
    };
    CubPlan {
        bag_first,
        prefer,
        coop: automation.ranch_coop_animal,
        shed: automation.ranch_shed_animal,
    }
}

fn grass_threshold(percent: i64) -> i64 {
    if (GRASS_THRESHOLD_MIN..=GRASS_THRESHOLD_MAX).contains(&percent) {
        percent
    } else {
        GRASS_THRESHOLD_DEFAULT
    }
}

fn affordable_count(
    shortfall: i64,
    balance: i64,
    unit_price: i64,
    max_count: i64,
    remaining: Option<i64>,
) -> i64 {
    if shortfall <= 0 || unit_price <= 0 || max_count <= 0 || balance < unit_price {
        return 0;
    }
    let mut count = shortfall.min(max_count).min(balance / unit_price);
    if let Some(remaining) = remaining {
        count = count.min(remaining.max(0));
    }
    count.max(0)
}

fn grass_room(facility: &FacilityInfo, threshold_percent: i64) -> Option<i64> {
    if facility.module_type != super::GRASS_MACHINE || facility.state != super::FACILITY_READY {
        return None;
    }
    let machine = facility.feed_machine.as_ref()?;
    let capacity = i64::from(facility.capacity);
    let filled = i64::from(machine.feed_count);
    let room = capacity - filled;
    if room <= 0 || filled.saturating_mul(100) >= capacity.saturating_mul(threshold_percent) {
        None
    } else {
        Some(room)
    }
}

fn grass_top_up(facility: &FacilityInfo, stock: i64, threshold_percent: i64) -> Option<i32> {
    let count = grass_room(facility, threshold_percent)?.min(stock);
    i32::try_from(count).ok().filter(|count| *count > 0)
}

fn check_delay_ms(minutes: i64) -> i64 {
    let minutes = if (CHECK_MIN_MINUTES..=CHECK_MAX_MINUTES).contains(&minutes) {
        minutes
    } else {
        CHECK_DEFAULT_MINUTES
    };
    minutes.saturating_mul(60_000)
}

async fn patrol<F, Fut>(
    gateway: &Arc<Gateway>,
    account_id: &str,
    self_gid: i64,
    flags: PatrolFlags,
    cubs: CubPlan,
    grass_threshold_percent: i64,
    farm_deadline_ms: i64,
    load_friends: F,
) -> Result<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Vec<crate::services::ranch_friend::RanchFriend>> + Send,
{
    if !gateway.scene_unlocked(super::RANCH_SCENE) {
        return Err(Error::Business(super::LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    let account = account_id.to_owned();
    let threshold = grass_threshold(grass_threshold_percent);
    super::in_ranch(gateway, account_id, move |mut cells, mut bag| async move {
        let fallback = begin_visit(&account, flags, farm_deadline_ms);
        sync_board(&account, flags, &cells);
        let mut done = current_done(&account, fallback);
        if !done.harvest {
            if hold(&account) {
                return Ok(());
            }
            let changed = acted(harvest_phase(&owned, &account, flags.harvest, &cells).await)?;
            if changed {
                (cells, bag) = reload(&owned).await?;
                sync_board(&account, flags, &cells);
            }
            note_acted(&account, changed);
            done = current_done(&account, done);
            done.harvest = true;
            mark_done(&account, done);
        }
        if !done.produce {
            if hold(&account) {
                return Ok(());
            }
            let changed = acted(produce_phase(&owned, &account, flags.produce, &cells).await)?;
            if changed {
                (cells, bag) = reload(&owned).await?;
                sync_board(&account, flags, &cells);
            }
            note_acted(&account, changed);
            done = current_done(&account, done);
            done.produce = true;
            mark_done(&account, done);
        }
        if !done.clean {
            if hold(&account) {
                return Ok(());
            }
            let changed = acted(clean_phase(&owned, &account, flags.clean, &cells).await)?;
            if changed {
                (cells, bag) = reload(&owned).await?;
                sync_board(&account, flags, &cells);
            }
            note_acted(&account, changed);
            done = current_done(&account, done);
            done.clean = true;
            mark_done(&account, done);
        }
        if !done.sell {
            if hold(&account) {
                return Ok(());
            }
            let changed = acted(sell_phase(&owned, &account, flags.sell, &bag).await)?;
            if changed {
                (cells, bag) = reload(&owned).await?;
                sync_board(&account, flags, &cells);
            }
            note_acted(&account, changed);
            done = current_done(&account, done);
            done.sell = true;
            mark_done(&account, done);
        }
        if !done.market_organic {
            if hold(&account) {
                return Ok(());
            }
            let changed = acted(market_phase(&owned, &account, flags.market_organic, &bag).await)?;
            if changed {
                (cells, bag) = reload(&owned).await?;
                sync_board(&account, flags, &cells);
            }
            note_acted(&account, changed);
            done = current_done(&account, done);
            done.market_organic = true;
            mark_done(&account, done);
        }
        if !done.upgrade {
            if hold(&account) {
                return Ok(());
            }
            let carried = upgrade_phase(&owned, &account, flags, cells, bag).await?;
            cells = carried.0;
            bag = carried.1;
            if carried.2 {
                return Ok(());
            }
            done = current_done(&account, done);
            done.upgrade = true;
            mark_done(&account, done);
        }
        if !done.buy {
            if hold(&account) {
                return Ok(());
            }
            let carried = buy_phase(&owned, &account, flags, cubs, cells, bag).await?;
            cells = carried.0;
            bag = carried.1;
            if carried.2 {
                return Ok(());
            }
            done = current_done(&account, done);
            done.buy = true;
            mark_done(&account, done);
        }
        if !done.grass {
            if hold(&account) {
                return Ok(());
            }
            let carried = grass_phase(&owned, &account, flags, threshold, cells, bag).await?;
            cells = carried.0;
            bag = carried.1;
            if carried.2 {
                return Ok(());
            }
            done = current_done(&account, done);
            done.grass = true;
            mark_done(&account, done);
        }
        if !done.cans {
            if hold(&account) {
                return Ok(());
            }
            if flags.fill_cans {
                let filled = fill_cans(&owned, &account, bag).await?;
                bag = filled.0;
                if filled.1 {
                    return Ok(());
                }
            }
            done = current_done(&account, done);
            done.cans = true;
            mark_done(&account, done);
            note_acted(&account, false);
        }
        if !done.feed_normal {
            if hold(&account) {
                return Ok(());
            }
            let changed = acted(
                feed_phase(&owned, &account, flags.feed_normal, super::BUCKET_NORMAL, &cells, &bag)
                    .await,
            )?;
            if changed {
                (cells, bag) = reload(&owned).await?;
                sync_board(&account, flags, &cells);
            }
            note_acted(&account, changed);
            done = current_done(&account, done);
            done.feed_normal = true;
            mark_done(&account, done);
        }
        if !done.feed_organic {
            if hold(&account) {
                return Ok(());
            }
            let changed = acted(
                feed_phase(
                    &owned,
                    &account,
                    flags.feed_organic,
                    super::BUCKET_ORGANIC,
                    &cells,
                    &bag,
                )
                .await,
            )?;
            if changed {
                sync_board(&account, flags, &cells);
            }
            note_acted(&account, changed);
            done = current_done(&account, done);
            done.feed_organic = true;
            mark_done(&account, done);
        }
        if !done.friends {
            if hold(&account) {
                return Ok(());
            }
            let friends = load_friends().await;
            let (visits, pending) = friend_progress(&account);
            let round = crate::services::ranch_friend::visit_friends(
                &owned,
                &account,
                self_gid,
                &friends,
                flags.friends,
                visits,
                pending,
                |acted_now| {
                    if acted_now {
                        note_acted(&account, true);
                    }
                    hold(&account)
                },
            )
            .await?;
            remember_friends(&account, round.visits, round.pending);
            if round.yielded {
                return Ok(());
            }
            done = current_done(&account, done);
            done.friends = true;
            mark_done(&account, done);
        }
        let _ = (cells, bag);
        Ok(())
    })
    .await
}

fn acted(result: Result<bool>) -> Result<bool> {
    match result {
        Ok(acted) => Ok(acted),
        Err(Error::Business(message)) => {
            tracing::warn!(%message, "牧场巡查跳过这一步");
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

async fn reload(gateway: &Gateway) -> Result<(AllCellsReply, GetBagReply)> {
    let cells = match super::all_cells(gateway).await {
        Ok(cells) => cells,
        Err(error) => {
            tracing::error!(%error, "牧场巡查后未能重新读取栏舍");
            return Err(Error::Protocol("操作后的牧场状态未能读取".to_owned()));
        }
    };
    match super::ranch_bag(gateway).await {
        Ok(bag) => Ok((cells, bag)),
        Err(error) => {
            tracing::error!(%error, "牧场巡查后未能重新读取背包");
            Err(Error::Protocol("操作后的牧场状态未能读取".to_owned()))
        }
    }
}

async fn harvest_phase(
    gateway: &Gateway,
    account_id: &str,
    enabled: bool,
    cells: &AllCellsReply,
) -> Result<bool> {
    if !enabled || !super::harvest_ready(cells) {
        return Ok(false);
    }
    let request = super::harvest_request(cells, None, None)?;
    let bytes =
        gateway.request(super::ANIMAL_SERVICE, "HarvestRanch", &request.encode_to_vec()).await?;
    HarvestRanchReply::decode(bytes.as_slice())?;
    tracing::info!(account_id, "牧场巡查已收获");
    Ok(true)
}

async fn produce_phase(
    gateway: &Gateway,
    account_id: &str,
    enabled: bool,
    cells: &AllCellsReply,
) -> Result<bool> {
    if !enabled {
        return Ok(false);
    }
    let Some(request) = super::produce_request(cells, None, None)? else {
        return Ok(false);
    };
    let bytes =
        gateway.request(super::ANIMAL_SERVICE, "TriggerProduce", &request.encode_to_vec()).await?;
    TriggerProduceReply::decode(bytes.as_slice())?;
    tracing::info!(account_id, "牧场巡查已生产");
    Ok(true)
}

async fn clean_phase(
    gateway: &Gateway,
    account_id: &str,
    enabled: bool,
    cells: &AllCellsReply,
) -> Result<bool> {
    if !enabled {
        return Ok(false);
    }
    let Some(request) = super::clean_request(cells) else {
        return Ok(false);
    };
    let bytes =
        gateway.request(super::RANCH_SERVICE, "CleanAllRanch", &request.encode_to_vec()).await?;
    CleanAllRanchReply::decode(bytes.as_slice())?;
    tracing::info!(account_id, "牧场巡查已清理");
    Ok(true)
}

async fn sell_phase(
    gateway: &Gateway,
    account_id: &str,
    enabled: bool,
    bag: &GetBagReply,
) -> Result<bool> {
    if !enabled {
        return Ok(false);
    }
    let Some(sale) = super::sale_plan(bag, super::SellChoice::All)? else {
        return Ok(false);
    };
    let plan = super::money_plan(
        super::sell_target(super::SellChoice::All),
        "出售牧场产物".to_owned(),
        bag,
        &super::sale_costs(&sale),
    );
    let sold = sale.sold.clone();
    let reward = sale.reward.clone();
    let request = sale.request;
    crate::services::ranch_ops::spend(account_id, plan, || async move {
        let bytes = gateway.request(super::ITEM_SERVICE, "Sell", &request.encode_to_vec()).await?;
        let reply = SellReply::decode(bytes.as_slice())?;
        if super::sold_signature(&reply.sell_items) != sold
            || super::id_count_signature(reply.get_items.iter().map(|item| (item.id, item.count)))
                != reward
        {
            return Err(Error::Protocol("出售物品或收益不匹配".to_owned()));
        }
        Ok(())
    })
    .await?;
    tracing::info!(account_id, "牧场巡查已出售产物");
    Ok(true)
}

async fn upgrade_phase(
    gateway: &Gateway,
    account_id: &str,
    flags: PatrolFlags,
    mut cells: AllCellsReply,
    mut bag: GetBagReply,
) -> Result<(AllCellsReply, GetBagReply, bool)> {
    if !flags.upgrade {
        return Ok((cells, bag, false));
    }
    let ids = cells
        .facilities
        .iter()
        .filter(|facility| super::RANCH_MODULES.contains(&facility.module_type))
        .map(|facility| facility.instance_id)
        .collect::<Vec<_>>();
    for facility_id in ids {
        if hold(account_id) {
            return Ok((cells, bag, true));
        }
        let level = super::ranch_level(super::item_count(&bag, super::RANCH_EXP));
        let now_ms = crate::utils::time::get_server_time_secs().saturating_mul(1000);
        let action = cells
            .facilities
            .iter()
            .find(|facility| facility.instance_id == facility_id)
            .map(|facility| super::upgrade_action(facility, level, now_ms))
            .unwrap_or("");
        let step = if action == "finish" {
            finish_upgrade(gateway, facility_id, &cells, now_ms).await
        } else if action == "start" {
            start_upgrade(gateway, account_id, facility_id, &cells, &bag, level).await
        } else {
            Ok(false)
        };
        match step {
            Ok(true) => {
                tracing::info!(account_id, facility_id, "牧场巡查已处理升级");
                note_acted(account_id, true);
                (cells, bag) = reload(gateway).await?;
                sync_board(account_id, flags, &cells);
                if hold(account_id) {
                    return Ok((cells, bag, true));
                }
            }
            Ok(false) => {}
            Err(Error::Business(message)) => {
                tracing::warn!(account_id, facility_id, %message, "牧场巡查跳过这座设施的升级");
                note_acted(account_id, false);
            }
            Err(error) => return Err(error),
        }
    }
    Ok((cells, bag, false))
}

async fn finish_upgrade(
    gateway: &Gateway,
    facility_id: i64,
    cells: &AllCellsReply,
    now_ms: i64,
) -> Result<bool> {
    let Some(target) = super::finish_target(cells, facility_id, now_ms)? else {
        return Ok(false);
    };
    let request = CompleteBuildingUpgradeRequest { inst_id: facility_id };
    let bytes = gateway
        .request(super::HOMESTEAD_SERVICE, "CompleteBuildingUpgrade", &request.encode_to_vec())
        .await?;
    let reply = CompleteBuildingUpgradeReply::decode(bytes.as_slice())?;
    let facility = reply.facility.ok_or_else(|| Error::Protocol("建筑等级不匹配".to_owned()))?;
    if facility.instance_id != facility_id
        || facility.level != target.next_level
        || facility.facility_id != target.next_facility_id
    {
        return Err(Error::Protocol("建筑等级不匹配".to_owned()));
    }
    Ok(true)
}

async fn start_upgrade(
    gateway: &Gateway,
    account_id: &str,
    facility_id: i64,
    cells: &AllCellsReply,
    bag: &GetBagReply,
    level: i64,
) -> Result<bool> {
    let target = super::start_target(cells, facility_id, level)?;
    let request = GetFacilityDetailRequest { instance_id: facility_id, host_gid: 0 };
    let bytes = gateway
        .request(super::HOMESTEAD_SERVICE, "GetFacilityDetail", &request.encode_to_vec())
        .await?;
    let detail = GetFacilityDetailReply::decode(bytes.as_slice())?;
    let info = detail.info.ok_or_else(|| Error::Business("建筑等级已变化".to_owned()))?;
    if info.facility_id != target.facility_id || info.level != target.level {
        return Err(Error::Business("建筑等级已变化".to_owned()));
    }
    if info.upgrade_cost.is_empty()
        || info.upgrade_cost.iter().any(|cost| super::unlocked_count(bag, cost.id) < cost.count)
    {
        return Err(Error::Business(if info.upgrade_cost.is_empty() {
            "升级费用尚未确认".to_owned()
        } else {
            "余额不足".to_owned()
        }));
    }
    let costs = info.upgrade_cost.iter().map(|cost| (cost.id, cost.count)).collect::<Vec<_>>();
    let plan = super::money_plan(
        format!("ranch:upgrade:{facility_id}"),
        "升级牧场设施".to_owned(),
        bag,
        &costs,
    );
    crate::services::ranch_ops::spend(account_id, plan, || async move {
        let request = UpgradeBuildingRequest { inst_id: facility_id };
        let bytes = gateway
            .request(super::HOMESTEAD_SERVICE, "UpgradeBuilding", &request.encode_to_vec())
            .await?;
        let reply = UpgradeBuildingReply::decode(bytes.as_slice())?;
        let upgrade =
            reply.upgrade.ok_or_else(|| Error::Protocol("建筑升级目标不匹配".to_owned()))?;
        if upgrade.inst_id != facility_id
            || upgrade.target_level != target.next_level
            || upgrade.target_facility_id != target.next_facility_id
        {
            return Err(Error::Protocol("建筑升级目标不匹配".to_owned()));
        }
        Ok(())
    })
    .await?;
    Ok(true)
}

async fn buy_phase(
    gateway: &Gateway,
    account_id: &str,
    flags: PatrolFlags,
    plan: CubPlan,
    mut cells: AllCellsReply,
    mut bag: GetBagReply,
) -> Result<(AllCellsReply, GetBagReply, bool)> {
    if !flags.buy {
        return Ok((cells, bag, false));
    }
    let facility_ids = cells
        .facilities
        .iter()
        .filter(|facility| stall_room(facility) > 0)
        .map(|facility| facility.instance_id)
        .collect::<Vec<_>>();
    if facility_ids.is_empty() {
        return Ok((cells, bag, false));
    }
    let saved = crate::models::store::account_config::get_automation(Some(account_id));
    let mut blocked = HashSet::new();
    if plan.bag_first {
        for facility_id in &facility_ids {
            if hold(account_id) {
                return Ok((cells, bag, true));
            }
            let Some(kind) = cells
                .facilities
                .iter()
                .find(|facility| facility.instance_id == *facility_id)
                .map(stall_kind)
            else {
                continue;
            };
            let priority = if kind == super::NEST {
                saved.ranch_coop_cub_priority.as_slice()
            } else {
                saved.ranch_shed_cub_priority.as_slice()
            };
            let finished = place_available(
                gateway,
                account_id,
                flags,
                *facility_id,
                kind,
                None,
                priority,
                &mut cells,
                &mut bag,
            )
            .await?;
            if hold(account_id) {
                return Ok((cells, bag, true));
            }
            if !finished {
                blocked.insert(*facility_id);
            }
        }
    }
    let room_left = facility_ids.iter().any(|facility_id| {
        !blocked.contains(facility_id)
            && cells
                .facilities
                .iter()
                .find(|facility| facility.instance_id == *facility_id)
                .is_some_and(|facility| stall_room(facility) > 0)
    });
    if !room_left {
        return Ok((cells, bag, false));
    }
    let now = crate::utils::time::get_server_time_secs();
    let info = super::shop_info(gateway, super::SHOP_CUBS).await?;
    let mut offers = super::shop_offers(&info.goods_list, super::SHOP_CUBS, now);
    for facility_id in facility_ids {
        if blocked.contains(&facility_id) {
            continue;
        }
        let Some(kind) = cells
            .facilities
            .iter()
            .find(|facility| facility.instance_id == facility_id)
            .map(stall_kind)
        else {
            continue;
        };
        let preferred = if kind == super::NEST { plan.coop } else { plan.shed };
        let choice = pick_cub(&cub_candidates(&offers), kind, plan.prefer, preferred);
        if !plan.bag_first {
            let Some(animal_id) = choice.as_ref().map(|choice| choice.animal_id) else {
                continue;
            };
            let finished = place_available(
                gateway,
                account_id,
                flags,
                facility_id,
                kind,
                Some(animal_id),
                &[],
                &mut cells,
                &mut bag,
            )
            .await?;
            if hold(account_id) {
                return Ok((cells, bag, true));
            }
            if !finished {
                continue;
            }
        }
        let Some(choice) = choice else {
            continue;
        };
        let room = cells
            .facilities
            .iter()
            .find(|facility| facility.instance_id == facility_id)
            .map(stall_room)
            .unwrap_or(0);
        let Some(offer) = offers.get(choice.index) else {
            continue;
        };
        let count = affordable_count(
            room,
            super::unlocked_count(&bag, offer.price_id),
            offer.unit_price,
            offer.max_count,
            offer.remaining,
        );
        if count <= 0 {
            continue;
        }
        if hold(account_id) {
            return Ok((cells, bag, true));
        }
        let offer = offer.clone();
        match buy_cub(gateway, account_id, &bag, &cells, &offer, count).await {
            Ok(()) => {
                if let Some(left) = offers[choice.index].remaining.as_mut() {
                    *left = left.saturating_sub(count);
                }
                if offers[choice.index].remaining.is_some_and(|left| left <= 0) {
                    offers[choice.index].available = false;
                }
                tracing::info!(
                    account_id,
                    facility_id,
                    goods_id = offer.id,
                    count,
                    "牧场巡查已购买幼崽"
                );
                note_acted(account_id, true);
                (cells, bag) = reload(gateway).await?;
                sync_board(account_id, flags, &cells);
                if hold(account_id) {
                    return Ok((cells, bag, true));
                }
            }
            Err(Error::Business(message)) => {
                tracing::warn!(account_id, facility_id, %message, "牧场巡查跳过购买幼崽");
                note_acted(account_id, false);
            }
            Err(error) => return Err(error),
        }
    }
    Ok((cells, bag, false))
}

struct CubCandidate {
    animal_id: i64,
    required_level: i64,
    kind: i32,
    available: bool,
}

struct CubChoice {
    index: usize,
    animal_id: i64,
}

fn cub_candidates(offers: &[super::ShopOffer]) -> Vec<CubCandidate> {
    offers
        .iter()
        .map(|offer| CubCandidate {
            animal_id: offer.animal_id.unwrap_or(0),
            required_level: offer.required_level,
            kind: offer.husbandry_kind.unwrap_or(0),
            available: offer.available
                && offer.item_count == 1
                && offer.price_id == GRASS_PRICE_ID
                && offer.unit_price > 0
                && offer.animal_id.is_some_and(super::animal_enabled),
        })
        .collect()
}

fn pick_cub(
    candidates: &[CubCandidate],
    kind: i32,
    prefer: bool,
    preferred_id: i64,
) -> Option<CubChoice> {
    let mut matched = candidates.iter().enumerate().filter(|(_, candidate)| {
        candidate.available && candidate.kind == kind && candidate.animal_id > 0
    });
    if prefer {
        return matched
            .find(|(_, candidate)| candidate.animal_id == preferred_id)
            .map(|(index, candidate)| CubChoice { index, animal_id: candidate.animal_id });
    }
    matched
        .max_by(|(_, left), (_, right)| {
            left.required_level
                .cmp(&right.required_level)
                .then(left.animal_id.cmp(&right.animal_id))
        })
        .map(|(index, candidate)| CubChoice { index, animal_id: candidate.animal_id })
}

fn bag_cubs(
    bag: &GetBagReply,
    kind: i32,
    only_animal: Option<i64>,
    priority: &[i64],
) -> Vec<(i64, i64)> {
    let mut cubs = super::cub_catalog()
        .iter()
        .filter_map(|(item_id, spec)| {
            if spec.kind != kind || only_animal.is_some_and(|animal_id| spec.animal_id != animal_id)
            {
                return None;
            }
            let stock = super::unlocked_count(bag, *item_id);
            (stock > 0).then_some((*item_id, spec.animal_id, stock))
        })
        .collect::<Vec<_>>();
    cubs.sort_by(|left, right| {
        let left_rank = priority.iter().position(|id| *id == left.0).unwrap_or(priority.len());
        let right_rank = priority.iter().position(|id| *id == right.0).unwrap_or(priority.len());
        left_rank.cmp(&right_rank).then(right.1.cmp(&left.1)).then(left.0.cmp(&right.0))
    });
    cubs.into_iter().map(|(item_id, _, stock)| (item_id, stock)).collect()
}

async fn place_available(
    gateway: &Gateway,
    account_id: &str,
    flags: PatrolFlags,
    facility_id: i64,
    kind: i32,
    only_animal: Option<i64>,
    priority: &[i64],
    cells: &mut AllCellsReply,
    bag: &mut GetBagReply,
) -> Result<bool> {
    let mut skipped = HashSet::new();
    loop {
        if hold(account_id) {
            return Ok(false);
        }
        let room = cells
            .facilities
            .iter()
            .find(|facility| facility.instance_id == facility_id)
            .map(stall_room)
            .unwrap_or(0);
        if room <= 0 {
            return Ok(true);
        }
        let Some((item_id, stock)) = bag_cubs(bag, kind, only_animal, priority)
            .into_iter()
            .find(|(item_id, _)| !skipped.contains(item_id))
        else {
            return Ok(true);
        };
        let Some(count) = i32::try_from(room.min(stock)).ok().filter(|count| *count > 0) else {
            return Ok(true);
        };
        match place_cub(gateway, cells, bag, facility_id, item_id, i64::from(count)).await {
            Ok(()) => {
                tracing::info!(account_id, facility_id, item_id, count, "牧场巡查已放养背包幼崽");
                note_acted(account_id, true);
                (*cells, *bag) = reload(gateway).await?;
                sync_board(account_id, flags, cells);
            }
            Err(Error::Business(message)) => {
                tracing::warn!(account_id, facility_id, item_id, %message, "牧场巡查跳过放养幼崽");
                note_acted(account_id, false);
                skipped.insert(item_id);
                return Ok(false);
            }
            Err(error) => return Err(error),
        }
    }
}

fn stall_room(facility: &FacilityInfo) -> i64 {
    if facility.state != super::FACILITY_READY {
        return 0;
    }
    facility
        .ranch
        .as_ref()
        .map(|ranch| (i64::from(ranch.capacity) - i64::from(ranch.animal_count)).max(0))
        .unwrap_or(0)
}

fn stall_kind(facility: &FacilityInfo) -> i32 {
    facility.ranch.as_ref().map(|ranch| ranch.husbandry_type).unwrap_or(super::SHED)
}

async fn place_cub(
    gateway: &Gateway,
    cells: &AllCellsReply,
    bag: &GetBagReply,
    ranch_id: i64,
    item_id: i64,
    count: i64,
) -> Result<()> {
    let request = super::place_request(cells, bag, ranch_id, item_id, count)?;
    let before = super::item_count(bag, item_id);
    let bytes =
        gateway.request(super::ANIMAL_SERVICE, "PlaceAnimal", &request.encode_to_vec()).await?;
    let reply = PlaceAnimalReply::decode(bytes.as_slice())?;
    if i64::try_from(reply.op_results.len()).ok() != Some(count) {
        return Err(Error::Protocol("放养数量不匹配".to_owned()));
    }
    let after = match super::ranch_bag(gateway).await {
        Ok(bag) => bag,
        Err(error) => {
            tracing::error!(%error, "放养后未能重新读取背包");
            return Err(Error::Protocol("操作后的库存未能读取".to_owned()));
        }
    };
    if before.checked_sub(super::item_count(&after, item_id)) != Some(count) {
        return Err(Error::Protocol("库存扣减与费用不匹配".to_owned()));
    }
    Ok(())
}

async fn buy_cub(
    gateway: &Gateway,
    account_id: &str,
    bag: &GetBagReply,
    cells: &AllCellsReply,
    offer: &super::ShopOffer,
    count: i64,
) -> Result<()> {
    let Some(animal_id) = super::placement_target(offer, cells, count, true)? else {
        return Err(Error::Business("只有幼崽可以自动放养".to_owned()));
    };
    let cost = offer.unit_price.saturating_mul(count);
    let goods_id = offer.id;
    let price_id = offer.price_id;
    let unit_price = offer.unit_price;
    let label = super::purchase_label(offer.item_id, offer.item_count.saturating_mul(count));
    let plan = super::money_plan(format!("ranch:buy:{goods_id}"), label, bag, &[(price_id, cost)]);
    crate::services::ranch_ops::spend(account_id, plan, || async move {
        let request = BuyGoodsRequest { goods_id, num: count, price: unit_price, auto_add: true };
        let bytes =
            gateway.request(super::SHOP_SERVICE, "BuyGoods", &request.encode_to_vec()).await?;
        let bought = BuyGoodsReply::decode(bytes.as_slice())?;
        let expected_cost =
            super::item_signature(&[CoreItem { id: price_id, count: cost, ..CoreItem::default() }]);
        if bought.goods.as_ref().map(|goods| goods.id) != Some(goods_id)
            || super::item_signature(&bought.cost_items) != expected_cost
        {
            return Err(Error::Protocol("商品或扣费不匹配".to_owned()));
        }
        super::confirm_placement(gateway, cells, animal_id, count).await
    })
    .await
}

async fn grass_phase(
    gateway: &Gateway,
    account_id: &str,
    flags: PatrolFlags,
    threshold_percent: i64,
    mut cells: AllCellsReply,
    mut bag: GetBagReply,
) -> Result<(AllCellsReply, GetBagReply, bool)> {
    if !flags.grass {
        return Ok((cells, bag, false));
    }
    let ids = cells
        .facilities
        .iter()
        .filter(|facility| facility.module_type == super::GRASS_MACHINE)
        .map(|facility| facility.instance_id)
        .collect::<Vec<_>>();
    for facility_id in ids {
        if hold(account_id) {
            return Ok((cells, bag, true));
        }
        let room = cells
            .facilities
            .iter()
            .find(|facility| facility.instance_id == facility_id)
            .and_then(|facility| grass_room(facility, threshold_percent));
        let Some(room) = room else {
            continue;
        };
        if flags.grass_buy && super::item_count(&bag, super::GRASS_ITEM) < room {
            let bought = buy_grass(gateway, account_id, bag, room).await?;
            bag = bought.0;
            if bought.1 {
                return Ok((cells, bag, true));
            }
        }
        let stock = super::item_count(&bag, super::GRASS_ITEM);
        let Some(count) = cells
            .facilities
            .iter()
            .find(|facility| facility.instance_id == facility_id)
            .and_then(|facility| grass_top_up(facility, stock, threshold_percent))
        else {
            continue;
        };
        match add_grass(gateway, &cells, &bag, facility_id, count).await {
            Ok(()) => {
                tracing::info!(account_id, facility_id, count, "牧场巡查已补充牧草");
                note_acted(account_id, true);
                (cells, bag) = reload(gateway).await?;
                sync_board(account_id, flags, &cells);
                if hold(account_id) {
                    return Ok((cells, bag, true));
                }
            }
            Err(Error::Business(message)) => {
                tracing::warn!(account_id, facility_id, %message, "牧场巡查跳过补充牧草");
                note_acted(account_id, false);
            }
            Err(error) => return Err(error),
        }
    }
    Ok((cells, bag, false))
}

async fn buy_grass(
    gateway: &Gateway,
    account_id: &str,
    mut bag: GetBagReply,
    room: i64,
) -> Result<(GetBagReply, bool)> {
    let now = crate::utils::time::get_server_time_secs();
    let info = super::shop_info(gateway, super::SHOP_FEED).await?;
    let Some(offer) =
        super::shop_offers(&info.goods_list, super::SHOP_FEED, now).into_iter().find(|offer| {
            offer.available
                && offer.item_id == super::GRASS_ITEM
                && offer.item_count == 1
                && offer.price_id == GRASS_PRICE_ID
                && offer.unit_price > 0
        })
    else {
        return Ok((bag, false));
    };
    let mut remaining = offer.remaining;
    while super::item_count(&bag, super::GRASS_ITEM) < room {
        if hold(account_id) {
            return Ok((bag, true));
        }
        let shortfall = room - super::item_count(&bag, super::GRASS_ITEM);
        let count = affordable_count(
            shortfall,
            super::unlocked_count(&bag, offer.price_id),
            offer.unit_price,
            offer.max_count,
            remaining,
        );
        if count <= 0 {
            break;
        }
        match buy_feed_goods(gateway, account_id, &bag, &offer, count).await {
            Ok(()) => {
                if let Some(left) = remaining.as_mut() {
                    *left = left.saturating_sub(count);
                }
                bag = match super::ranch_bag(gateway).await {
                    Ok(bag) => bag,
                    Err(error) => {
                        tracing::error!(%error, "购买牧草后未能重新读取背包");
                        return Err(Error::Protocol("操作后的库存未能读取".to_owned()));
                    }
                };
                tracing::info!(account_id, count, "牧场巡查已购买牧草");
                note_acted(account_id, true);
                if hold(account_id) {
                    return Ok((bag, true));
                }
            }
            Err(Error::Business(message)) => {
                tracing::warn!(account_id, %message, "牧场巡查跳过购买牧草");
                note_acted(account_id, false);
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok((bag, false))
}

async fn buy_feed_goods(
    gateway: &Gateway,
    account_id: &str,
    bag: &GetBagReply,
    offer: &super::ShopOffer,
    count: i64,
) -> Result<()> {
    let cost = offer.unit_price.saturating_mul(count);
    let goods_id = offer.id;
    let price_id = offer.price_id;
    let unit_price = offer.unit_price;
    let item_id = offer.item_id;
    let item_count = offer.item_count;
    let plan = super::money_plan(
        format!("ranch:buy:{goods_id}"),
        super::purchase_label(item_id, item_count.saturating_mul(count)),
        bag,
        &[(price_id, cost)],
    );
    crate::services::ranch_ops::spend(account_id, plan, || async move {
        let request = BuyGoodsRequest { goods_id, num: count, price: unit_price, auto_add: false };
        let bytes =
            gateway.request(super::SHOP_SERVICE, "BuyGoods", &request.encode_to_vec()).await?;
        let bought = BuyGoodsReply::decode(bytes.as_slice())?;
        let expected_cost =
            super::item_signature(&[CoreItem { id: price_id, count: cost, ..CoreItem::default() }]);
        let expected_reward = super::item_signature(&[CoreItem {
            id: item_id,
            count: item_count.saturating_mul(count),
            ..CoreItem::default()
        }]);
        if bought.goods.as_ref().map(|goods| goods.id) != Some(goods_id)
            || super::item_signature(&bought.cost_items) != expected_cost
            || super::item_signature(&bought.get_items) != expected_reward
        {
            return Err(Error::Protocol("商品或扣费不匹配".to_owned()));
        }
        Ok(())
    })
    .await
}

async fn add_grass(
    gateway: &Gateway,
    cells: &AllCellsReply,
    bag: &GetBagReply,
    facility_id: i64,
    count: i32,
) -> Result<()> {
    let request = super::grass_request(cells, bag, facility_id, count)?;
    let bytes =
        gateway.request(super::FEED_MACHINE_SERVICE, "AddFeed", &request.encode_to_vec()).await?;
    AddFeedReply::decode(bytes.as_slice())?;
    Ok(())
}

async fn fill_cans(
    gateway: &Gateway,
    account_id: &str,
    mut bag: GetBagReply,
) -> Result<(GetBagReply, bool)> {
    let uids = bag.physical_items.iter().map(|item| item.uid).collect::<Vec<_>>();
    for uid in uids {
        if hold(account_id) {
            return Ok((bag, true));
        }
        let Some(item) = bag.physical_items.iter().find(|item| item.uid == uid) else {
            continue;
        };
        if item.is_locked || item.count <= 0 {
            continue;
        }
        let count = item.count;
        let plan = match super::can_plan(&bag, uid, count) {
            Ok(plan) => plan,
            Err(Error::Business(_)) => continue,
            Err(error) => return Err(error),
        };
        let before_item = super::item_count(&bag, plan.item_id);
        let before_bucket = super::item_count(&bag, plan.bucket_id);
        let request = UseRequest {
            item: Some(RanchItem { id: plan.item_id, uid, count, ..RanchItem::default() }),
        };
        let bytes = gateway.request(super::ITEM_SERVICE, "Use", &request.encode_to_vec()).await?;
        UseReply::decode(bytes.as_slice())?;
        bag = match super::ranch_bag(gateway).await {
            Ok(bag) => bag,
            Err(error) => {
                tracing::error!(%error, "添加罐头后未能重新读取背包");
                return Err(Error::Protocol("操作后的库存未能读取".to_owned()));
            }
        };
        if before_item.checked_sub(super::item_count(&bag, plan.item_id)) != Some(count)
            || super::item_count(&bag, plan.bucket_id).checked_sub(before_bucket)
                != Some(plan.seconds)
        {
            return Err(Error::Protocol("罐头桶增加时长不匹配".to_owned()));
        }
        tracing::info!(account_id, "牧场巡查已补充罐头");
        note_acted(account_id, true);
        if hold(account_id) {
            return Ok((bag, true));
        }
    }
    Ok((bag, false))
}

async fn feed_phase(
    gateway: &Gateway,
    account_id: &str,
    enabled: bool,
    bucket_type: i32,
    cells: &AllCellsReply,
    bag: &GetBagReply,
) -> Result<bool> {
    if !enabled {
        return Ok(false);
    }
    let now_secs = crate::utils::time::get_server_time_secs();
    let Some(request) = super::feed_request(cells, bag, None, None, bucket_type, now_secs)? else {
        return Ok(false);
    };
    let bytes =
        gateway.request(super::ANIMAL_SERVICE, "FeedAnimal", &request.encode_to_vec()).await?;
    FeedAnimalReply::decode(bytes.as_slice())?;
    tracing::info!(account_id, bucket_type, "牧场巡查已喂养");
    Ok(true)
}

struct MarketClock {
    next_check_at: i64,
    last_poop: i64,
}

fn market_clocks() -> &'static Mutex<HashMap<String, MarketClock>> {
    static CLOCKS: std::sync::OnceLock<Mutex<HashMap<String, MarketClock>>> =
        std::sync::OnceLock::new();
    CLOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn wake_market_exchange(account_id: &str) {
    if !crate::models::store::account_config::get_automation(Some(account_id)).ranch_market_organic
    {
        return;
    }
    market_clocks().lock().remove(account_id);
    if let Some(cycle) = cycles().lock().get_mut(account_id) {
        cycle.done.market_organic = false;
    }
    let now = crate::utils::time::now_ms();
    let soon = now.saturating_add(SCENE_REENTRY_MS);
    let current = next_checks().lock().get(account_id).copied().unwrap_or(0);
    set_next(account_id, if current <= 0 { 0 } else { current.min(soon) });
}

fn market_should_run(account_id: &str, poop: i64, now: i64) -> bool {
    let clocks = market_clocks().lock();
    let Some(clock) = clocks.get(account_id) else {
        return true;
    };
    now >= clock.next_check_at || (poop >= POOP_EXCHANGE_FLOOR && poop > clock.last_poop)
}

async fn market_phase(
    gateway: &Arc<Gateway>,
    account_id: &str,
    enabled: bool,
    bag: &GetBagReply,
) -> Result<bool> {
    if !enabled {
        return Ok(false);
    }
    let now = crate::utils::time::now_ms();
    if !market_should_run(account_id, super::unlocked_count(bag, super::POOP_ID), now) {
        return Ok(false);
    }
    let mut bag = bag.clone();
    let bought = super::exchange_poop_organic(gateway, account_id, &mut bag).await?;
    market_clocks().lock().insert(
        account_id.to_owned(),
        MarketClock {
            next_check_at: now.saturating_add(MARKET_CHECK_MS),
            last_poop: super::unlocked_count(&bag, super::POOP_ID),
        },
    );
    Ok(bought)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrol_runs_only_when_a_switch_is_due() {
        let now = 1_000_000;
        assert!(!should_patrol(false, true, 0, now));
        assert!(!should_patrol(true, false, 0, now));
        assert!(!should_patrol(true, true, now + 1, now));
        assert!(should_patrol(true, true, now, now));
        assert!(!should_yield(false, 0, PATROL_SLICE_MS, i64::MAX));
        assert!(!should_yield(true, 0, PATROL_SLICE_MS - 1, i64::MAX));
        assert!(should_yield(true, 0, PATROL_SLICE_MS, i64::MAX));
        assert!(should_yield(true, 1_000, 2_000, 2_000));
        assert!(!should_yield(false, 1_000, 9_000, 1_000));
        assert_eq!(follow_up_at(1_000_000, 1_000_000, 300_000, i64::MAX, true), 1_030_000);
        assert_eq!(follow_up_at(1_000_000, 1_000_000, 300_000, i64::MAX, false), 1_300_000);
        assert_eq!(follow_up_at(1_000_000, 1_000_000, 300_000, 1_025_000, true), 1_025_000);
        assert_eq!(follow_up_at(1_000_000, 1_000_000, 300_000, 1_010_000, false), 1_020_000);
        let flags = PatrolFlags {
            harvest: false,
            produce: true,
            clean: false,
            sell: false,
            market_organic: false,
            upgrade: false,
            buy: false,
            grass: false,
            grass_buy: false,
            fill_cans: false,
            feed_normal: false,
            feed_organic: false,
            friends: crate::services::ranch_friend::FriendFlags {
                steal: false,
                stroke: false,
                clean: false,
                mosquito: false,
            },
        };
        let done = initial_done(flags);
        assert!(done.harvest);
        assert!(!done.produce);
        assert_eq!(check_delay_ms(5), 300_000);
        assert_eq!(check_delay_ms(0), 300_000);
        assert_eq!(check_delay_ms(5).max(RETRY_FLOOR_MS), RETRY_FLOOR_MS);
        assert_eq!(grass_threshold(20), 20);
        assert_eq!(grass_threshold(0), GRASS_THRESHOLD_DEFAULT);
        let low = grass_machine(10, 100);
        assert_eq!(grass_top_up(&low, 5, 20), Some(5));
        assert_eq!(grass_top_up(&low, 0, 20), None);
        assert_eq!(grass_top_up(&grass_machine(80, 100), 5, 20), None);
        assert_eq!(grass_top_up(&grass_machine(0, 0), 5, 20), None);
        assert_eq!(affordable_count(10, 25, 2, 99, None), 10);
        assert_eq!(affordable_count(10, 5, 2, 99, None), 2);
        assert_eq!(affordable_count(10, 100, 2, 99, Some(1)), 1);
        assert_eq!(affordable_count(10, 100, 2, 3, None), 3);
        assert_eq!(affordable_count(0, 100, 2, 99, None), 0);
        assert_eq!(affordable_count(10, 100, 0, 99, None), 0);
        let nest = [
            CubCandidate { animal_id: 60001, required_level: 1, kind: 1, available: true },
            CubCandidate { animal_id: 60002, required_level: 3, kind: 1, available: true },
            CubCandidate { animal_id: 60011, required_level: 9, kind: 2, available: true },
        ];
        assert_eq!(pick_cub(&nest, 1, false, 0).expect("level").animal_id, 60002);
        assert_eq!(pick_cub(&nest, 1, true, 60001).expect("preferred").animal_id, 60001);
        assert!(pick_cub(&nest, 1, true, 0).is_none());

        let account = "ranch-patrol-routine-test";
        set_next(account, now + 300_000);
        assert_eq!(next_checks().lock().get(account).copied(), Some(now + 300_000));
    }

    #[test]
    fn ready_animal_reopens_finished_harvest() {
        use crate::proto::generated::gamepb::animalpb::{AnimalDetail, RanchDetail};
        let flags = PatrolFlags {
            harvest: true,
            produce: true,
            clean: false,
            sell: false,
            market_organic: false,
            upgrade: false,
            buy: false,
            grass: false,
            grass_buy: false,
            fill_cans: false,
            feed_normal: false,
            feed_organic: false,
            friends: crate::services::ranch_friend::FriendFlags {
                steal: false,
                stroke: false,
                clean: false,
                mosquito: false,
            },
        };
        let account = "ranch-patrol-reopen-test";
        clear_cycle(account);
        begin_visit(account, flags, i64::MAX);
        let mut done = current_done(account, initial_done(flags));
        done.harvest = true;
        done.produce = true;
        mark_done(account, done);
        let server = crate::utils::time::get_server_time_secs().saturating_mul(1000);
        let cells = AllCellsReply {
            facilities: vec![FacilityInfo {
                upgrade_state: 1,
                upgrade_end_time: server.saturating_add(50_000),
                ranch: Some(RanchDetail {
                    animals: vec![AnimalDetail {
                        can_harvest: true,
                        can_deliver: true,
                        produce_ready_at_ms: server.saturating_add(40_000),
                        ..AnimalDetail::default()
                    }],
                    ..RanchDetail::default()
                }),
                ..FacilityInfo::default()
            }],
            ..AllCellsReply::default()
        };
        sync_board(account, flags, &cells);
        let done = current_done(account, initial_done(flags));
        assert!(!done.harvest);
        assert!(!done.produce);
        let next = next_ready_at(&cells, flags);
        let local = crate::utils::time::now_ms();
        assert!(next >= local);
        assert!(next <= local.saturating_add(60_000));
        clear_cycle(account);
    }

    #[test]
    fn acknowledge_restarts_the_patrol() {
        let account = "ranch-patrol-ack-test";
        clear_cycle(account);
        let flags = PatrolFlags {
            harvest: true,
            produce: false,
            clean: false,
            sell: false,
            market_organic: false,
            upgrade: false,
            buy: false,
            grass: false,
            grass_buy: false,
            fill_cans: false,
            feed_normal: false,
            feed_organic: false,
            friends: crate::services::ranch_friend::FriendFlags {
                steal: false,
                stroke: false,
                clean: false,
                mosquito: false,
            },
        };
        begin_visit(account, flags, i64::MAX);
        set_next(account, 9_000_000);
        reset_patrol_schedule(account);
        assert!(cycles().lock().get(account).is_none());
        assert_eq!(next_checks().lock().get(account).copied(), Some(0));
    }

    #[test]
    fn bag_cubs_place_the_saved_order_ahead_of_the_default_order() {
        let nests = super::super::cub_choices()
            .into_iter()
            .filter(|cub| cub.nest)
            .take(2)
            .collect::<Vec<_>>();
        assert!(nests.len() == 2, "窝幼崽目录不足两项");
        let later = nests[0].id;
        let earlier = nests[1].id;
        let bag = GetBagReply {
            physical_items: vec![
                crate::proto::generated::gamepb::ranchpb::Item {
                    id: later,
                    count: 1,
                    ..crate::proto::generated::gamepb::ranchpb::Item::default()
                },
                crate::proto::generated::gamepb::ranchpb::Item {
                    id: earlier,
                    count: 1,
                    ..crate::proto::generated::gamepb::ranchpb::Item::default()
                },
            ],
            ..GetBagReply::default()
        };
        let ordered = bag_cubs(&bag, super::super::NEST, None, &[earlier]);
        assert_eq!(ordered[0].0, earlier);
        assert_eq!(ordered[1].0, later);
        let default_order = bag_cubs(&bag, super::super::NEST, None, &[]);
        let first = if nests[0].animal_id == nests[1].animal_id {
            nests[0].id.min(nests[1].id)
        } else if nests[0].animal_id > nests[1].animal_id {
            nests[0].id
        } else {
            nests[1].id
        };
        assert_eq!(default_order[0].0, first);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn patrol_places_saved_cubs_before_opening_the_shop() {
        let _root = OpsRoot::new();
        let mut seen = std::collections::HashSet::new();
        let nests = super::super::cub_choices()
            .into_iter()
            .filter(|cub| cub.nest && seen.insert(cub.animal_id))
            .take(2)
            .collect::<Vec<_>>();
        assert!(nests.len() == 2, "窝幼崽目录不足两项");
        let account = "ranch-cub-place";
        let mut config =
            crate::models::store::account_config::get_account_config_snapshot(Some(account));
        config.automation.ranch_harvest = false;
        config.automation.ranch_produce = false;
        config.automation.ranch_clean = false;
        config.automation.ranch_sell = false;
        config.automation.ranch_market_organic = false;
        config.automation.ranch_upgrade = false;
        config.automation.ranch_buy = true;
        config.automation.ranch_strategy = crate::models::types::RanchStrategy::BagPriority;
        config.automation.ranch_grass = false;
        config.automation.ranch_grass_buy = false;
        config.automation.ranch_fill_cans = false;
        config.automation.ranch_feed_normal = false;
        config.automation.ranch_feed_organic = false;
        config.automation.ranch_friend_steal = false;
        config.automation.ranch_stroke = false;
        config.automation.ranch_friend_clean = false;
        config.automation.ranch_friend_mosquito = false;
        config.automation.ranch_coop_cub_priority = vec![nests[1].id];
        crate::models::store::account_config::set_account_config_snapshot(
            Some(account),
            config,
            false,
        );
        clear_cycle(account);
        set_next(account, 0);
        let placed = Arc::new(Mutex::new(Vec::<i64>::new()));
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let stocks = Arc::new(Mutex::new(vec![(nests[0].id, 1_i64), (nests[1].id, 1)]));
        let occupied = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let gateway = {
            use crate::network::encryptor::{Encryptor, NoopEncryptor};
            use crate::network::gateway::{Gateway, GatewayConfig};
            use crate::proto::generated::gamepb::animalpb::{
                PlaceAnimalReply, PlaceAnimalRequest, RanchDetail,
            };
            use crate::proto::generated::gamepb::globalscenepb::{
                PlayerSceneLoginReply, PlayerSceneLoginRequest,
            };
            use crate::proto::generated::gamepb::homesteadpb::{AllCellsReply, FacilityInfo};
            use crate::proto::generated::gamepb::ranchpb::{
                GetBagReply, Item as RanchItem, SceneData,
            };
            use prost::Message;
            let cfg = GatewayConfig {
                server_url: "https://example.com".to_string(),
                platform: "qq".to_string(),
                os: "linux".to_string(),
                client_version: "1.0".to_string(),
                auth_code: "code-1".to_string(),
                headers: std::collections::HashMap::new(),
            };
            let gateway =
                Arc::new(Gateway::new(cfg, Arc::new(NoopEncryptor) as Arc<dyn Encryptor>));
            gateway.mark_online();
            gateway.set_unlocked_scenes(vec![2]);
            let placed = Arc::clone(&placed);
            let calls = Arc::clone(&calls);
            let stocks = Arc::clone(&stocks);
            let occupied = Arc::clone(&occupied);
            gateway.install_reply_script(move |_service, method, body| {
                calls.lock().push(method.to_owned());
                let cells = AllCellsReply {
                    facilities: vec![FacilityInfo {
                        instance_id: 9,
                        state: 1,
                        module_type: 6,
                        ranch: Some(RanchDetail {
                            husbandry_type: super::super::NEST,
                            capacity: 2,
                            animal_count: occupied.load(std::sync::atomic::Ordering::Acquire),
                            ..RanchDetail::default()
                        }),
                        ..FacilityInfo::default()
                    }],
                    ..AllCellsReply::default()
                };
                match method {
                    "PlayerSceneLogin" => {
                        let request = PlayerSceneLoginRequest::decode(body).map_err(|error| {
                            crate::network::error::NetworkError::Phase(error.to_string())
                        })?;
                        let scene =
                            SceneData { homestead_cells: Some(cells), ..SceneData::default() };
                        let reply = if request.global_scene_id == 2 {
                            PlayerSceneLoginReply {
                                global_scene_id: 2,
                                big_farm_scene_ext_data: scene.encode_to_vec().into(),
                                ..PlayerSceneLoginReply::default()
                            }
                        } else {
                            PlayerSceneLoginReply {
                                global_scene_id: 1,
                                farm_scene_ext_data: vec![1].into(),
                                ..PlayerSceneLoginReply::default()
                            }
                        };
                        Ok(reply.encode_to_vec())
                    }
                    "AllCells" => Ok(cells.encode_to_vec()),
                    "GetBag" => {
                        let reply = GetBagReply {
                            global_scene_id: 2,
                            physical_items: stocks
                                .lock()
                                .iter()
                                .map(|(id, count)| RanchItem {
                                    id: *id,
                                    count: *count,
                                    ..RanchItem::default()
                                })
                                .collect(),
                            ..GetBagReply::default()
                        };
                        Ok(reply.encode_to_vec())
                    }
                    "PlaceAnimal" => {
                        let request = PlaceAnimalRequest::decode(body).map_err(|error| {
                            crate::network::error::NetworkError::Phase(error.to_string())
                        })?;
                        let animal_id =
                            request.items.first().map(|item| item.animal_id).unwrap_or(0);
                        placed.lock().push(animal_id);
                        occupied.fetch_add(
                            i32::try_from(request.items.len()).unwrap_or(0),
                            std::sync::atomic::Ordering::Release,
                        );
                        if let Some(item) = request.items.first() {
                            let mut stocks = stocks.lock();
                            if let Some(stock) = stocks.iter_mut().find(|(id, _)| {
                                super::super::cub_choices()
                                    .iter()
                                    .any(|cub| cub.id == *id && cub.animal_id == item.animal_id)
                            }) {
                                stock.1 = stock.1.saturating_sub(
                                    i64::try_from(request.items.len()).unwrap_or(0),
                                );
                            }
                        }
                        Ok(PlaceAnimalReply {
                            op_results: vec![Default::default(); request.items.len()],
                            ..PlaceAnimalReply::default()
                        }
                        .encode_to_vec())
                    }
                    _ => {
                        Err(crate::network::error::NetworkError::Phase(format!("未准备 {method}")))
                    }
                }
            });
            gateway
        };
        patrol_if_due(&gateway, account, 1, i64::MAX, || async { Vec::new() })
            .await
            .expect("place");
        assert_eq!(placed.lock().as_slice(), &[nests[1].animal_id, nests[0].animal_id]);
        assert!(calls.lock().iter().any(|call| call == "PlaceAnimal"));
        assert!(calls.lock().iter().all(|call| call != "ShopInfo"));
    }

    fn grass_machine(filled: i32, capacity: i32) -> FacilityInfo {
        use crate::proto::generated::gamepb::homesteadpb::FeedMachineDetail;
        FacilityInfo {
            module_type: 8,
            state: 1,
            capacity,
            feed_machine: Some(FeedMachineDetail {
                feed_count: filled,
                ..FeedMachineDetail::default()
            }),
            ..FacilityInfo::default()
        }
    }

    struct OpsRoot {
        dir: std::path::PathBuf,
    }

    impl OpsRoot {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "qq-farm-market-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&dir).expect("temp dir");
            crate::services::ranch_ops::set_operations_root_for_test(Some(dir.clone()));
            Self { dir }
        }
    }

    impl Drop for OpsRoot {
        fn drop(&mut self) {
            crate::services::ranch_ops::set_operations_root_for_test(None);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    struct MarketScript {
        poop: i64,
        calls: Vec<String>,
        buys: Vec<(i64, i32)>,
        fail_buy: bool,
        opened: bool,
    }

    fn only_market(account: &str, enabled: bool) {
        let mut config =
            crate::models::store::account_config::get_account_config_snapshot(Some(account));
        config.automation.ranch_harvest = false;
        config.automation.ranch_produce = false;
        config.automation.ranch_clean = false;
        config.automation.ranch_sell = false;
        config.automation.ranch_market_organic = enabled;
        config.automation.ranch_upgrade = false;
        config.automation.ranch_buy = false;
        config.automation.ranch_grass = false;
        config.automation.ranch_grass_buy = false;
        config.automation.ranch_fill_cans = false;
        config.automation.ranch_feed_normal = false;
        config.automation.ranch_feed_organic = false;
        config.automation.ranch_friend_steal = false;
        config.automation.ranch_stroke = false;
        config.automation.ranch_friend_clean = false;
        config.automation.ranch_friend_mosquito = false;
        crate::models::store::account_config::set_account_config_snapshot(
            Some(account),
            config,
            false,
        );
    }

    fn scripted_market(state: Arc<Mutex<MarketScript>>) -> Arc<crate::network::gateway::Gateway> {
        use crate::network::encryptor::{Encryptor, NoopEncryptor};
        use crate::network::gateway::{Gateway, GatewayConfig};
        use crate::proto::generated::gamepb::globalscenepb::{
            PlayerSceneLoginReply, PlayerSceneLoginRequest,
        };
        use crate::proto::generated::gamepb::homesteadpb::{AllCellsReply, AllCellsRequest};
        use crate::proto::generated::gamepb::ranchpb::{
            GetBagReply, Item as RanchItem, RotationBuyGoodsReply, RotationBuyGoodsRequest,
            RotationGoodsCard, RotationShopInfoReply, SceneData,
        };
        use prost::Message;

        let cfg = GatewayConfig {
            server_url: "https://example.com".to_string(),
            platform: "qq".to_string(),
            os: "linux".to_string(),
            client_version: "1.0".to_string(),
            auth_code: "code-1".to_string(),
            headers: std::collections::HashMap::new(),
        };
        let enc: Arc<dyn Encryptor> = Arc::new(NoopEncryptor);
        let gateway = Arc::new(Gateway::new(cfg, enc));
        gateway.mark_online();
        gateway.set_unlocked_scenes(vec![2]);
        gateway.install_reply_script(move |_service, method, body| {
            let mut state = state.lock();
            state.calls.push(method.to_owned());
            match method {
                "PlayerSceneLogin" => {
                    let request = PlayerSceneLoginRequest::decode(body).map_err(|error| {
                        crate::network::error::NetworkError::Phase(error.to_string())
                    })?;
                    let scene = SceneData {
                        homestead_cells: Some(AllCellsReply::default()),
                        ..SceneData::default()
                    };
                    let reply = if request.global_scene_id == 2 {
                        PlayerSceneLoginReply {
                            global_scene_id: 2,
                            big_farm_scene_ext_data: scene.encode_to_vec().into(),
                            ..PlayerSceneLoginReply::default()
                        }
                    } else {
                        PlayerSceneLoginReply {
                            global_scene_id: 1,
                            farm_scene_ext_data: vec![1].into(),
                            ..PlayerSceneLoginReply::default()
                        }
                    };
                    Ok(reply.encode_to_vec())
                }
                "AllCells" => {
                    let _ = AllCellsRequest::decode(body);
                    Ok(AllCellsReply::default().encode_to_vec())
                }
                "GetBag" => {
                    let reply = GetBagReply {
                        global_scene_id: 2,
                        physical_items: vec![RanchItem {
                            id: 7002,
                            count: state.poop,
                            ..RanchItem::default()
                        }],
                        ..GetBagReply::default()
                    };
                    Ok(reply.encode_to_vec())
                }
                "RotationShopInfo" => {
                    let four_hour = RotationGoodsCard {
                        goods_id: 11,
                        item_id: 80012,
                        item_count: 1,
                        cost_item_id: 7002,
                        cost_count: 3,
                        limit_count: 1,
                        ..RotationGoodsCard::default()
                    };
                    let one_hour = RotationGoodsCard {
                        goods_id: 12,
                        item_id: 80011,
                        item_count: 1,
                        cost_item_id: 7002,
                        cost_count: 1,
                        ..RotationGoodsCard::default()
                    };
                    let other_currency = RotationGoodsCard {
                        goods_id: 13,
                        item_id: 80012,
                        item_count: 1,
                        cost_item_id: 1007,
                        cost_count: 1,
                        ..RotationGoodsCard::default()
                    };
                    let reply = RotationShopInfoReply {
                        opened: state.opened,
                        present: true,
                        goods: vec![one_hour, other_currency, four_hour],
                        ..RotationShopInfoReply::default()
                    };
                    Ok(reply.encode_to_vec())
                }
                "RotationBuyGoods" => {
                    let request = RotationBuyGoodsRequest::decode(body).map_err(|error| {
                        crate::network::error::NetworkError::Phase(error.to_string())
                    })?;
                    state.buys.push((request.goods_id, request.count));
                    if state.fail_buy {
                        state.fail_buy = false;
                        return Err(crate::network::error::NetworkError::Phase(
                            "集市兑换失败".to_owned(),
                        ));
                    }
                    let price = if request.goods_id == 11 { 3 } else { 1 };
                    state.poop -= i64::from(request.count) * price;
                    let reward = if request.goods_id == 11 { 80012 } else { 80011 };
                    let reply = RotationBuyGoodsReply {
                        card: Some(RotationGoodsCard {
                            goods_id: request.goods_id,
                            ..RotationGoodsCard::default()
                        }),
                        awards: vec![RanchItem {
                            id: reward,
                            count: i64::from(request.count),
                            ..RanchItem::default()
                        }],
                        ..RotationBuyGoodsReply::default()
                    };
                    Ok(reply.encode_to_vec())
                }
                _ => Err(crate::network::error::NetworkError::Phase(format!("未准备 {method}"))),
            }
        });
        gateway
    }

    #[tokio::test(flavor = "current_thread")]
    async fn patrol_exchanges_four_hour_organic_before_one_hour_and_stops_cleanly() {
        let _root = OpsRoot::new();
        let account = "ranch-market-order";
        only_market(account, true);
        clear_cycle(account);
        market_clocks().lock().remove(account);
        set_next(account, 0);
        let state = Arc::new(Mutex::new(MarketScript {
            poop: 7,
            calls: Vec::new(),
            buys: Vec::new(),
            fail_buy: false,
            opened: true,
        }));
        let gateway = scripted_market(Arc::clone(&state));
        patrol_if_due(&gateway, account, 1, i64::MAX, || async { Vec::new() })
            .await
            .expect("exchange");
        assert_eq!(state.lock().buys, vec![(11, 1), (12, 4)]);
        assert!(state.lock().buys.iter().all(|(goods_id, _)| *goods_id != 13));
        let shop_reads =
            state.lock().calls.iter().filter(|call| *call == "RotationShopInfo").count();
        reset_patrol_schedule(account);
        patrol_if_due(&gateway, account, 1, i64::MAX, || async { Vec::new() })
            .await
            .expect("quiet");
        assert_eq!(state.lock().buys, vec![(11, 1), (12, 4)]);
        assert_eq!(
            state.lock().calls.iter().filter(|call| *call == "RotationShopInfo").count(),
            shop_reads
        );

        wake_market_exchange(account);
        reset_patrol_schedule(account);
        patrol_if_due(&gateway, account, 1, i64::MAX, || async { Vec::new() })
            .await
            .expect("woken");
        assert_eq!(state.lock().buys.len(), 2);
        assert!(
            state.lock().calls.iter().filter(|call| *call == "RotationShopInfo").count()
                > shop_reads
        );

        let closed = "ranch-market-closed";
        only_market(closed, false);
        clear_cycle(closed);
        set_next(closed, 0);
        let idle = Arc::new(Mutex::new(MarketScript {
            poop: 9,
            calls: Vec::new(),
            buys: Vec::new(),
            fail_buy: false,
            opened: true,
        }));
        let idle_gateway = scripted_market(Arc::clone(&idle));
        patrol_if_due(&idle_gateway, closed, 1, i64::MAX, || async { Vec::new() })
            .await
            .expect("off");
        assert!(idle.lock().calls.is_empty());

        let short = "ranch-market-short";
        only_market(short, true);
        clear_cycle(short);
        market_clocks().lock().remove(short);
        set_next(short, 0);
        let poor = Arc::new(Mutex::new(MarketScript {
            poop: 2,
            calls: Vec::new(),
            buys: Vec::new(),
            fail_buy: false,
            opened: true,
        }));
        let poor_gateway = scripted_market(Arc::clone(&poor));
        patrol_if_due(&poor_gateway, short, 1, i64::MAX, || async { Vec::new() })
            .await
            .expect("short");
        assert_eq!(poor.lock().buys, vec![(12, 2)]);

        let absent = "ranch-market-absent";
        only_market(absent, true);
        clear_cycle(absent);
        market_clocks().lock().remove(absent);
        set_next(absent, 0);
        let closed_shop = Arc::new(Mutex::new(MarketScript {
            poop: 9,
            calls: Vec::new(),
            buys: Vec::new(),
            fail_buy: false,
            opened: false,
        }));
        let closed_gateway = scripted_market(Arc::clone(&closed_shop));
        patrol_if_due(&closed_gateway, absent, 1, i64::MAX, || async { Vec::new() })
            .await
            .expect("absent");
        assert!(closed_shop.lock().buys.is_empty());
        assert!(closed_shop.lock().calls.iter().any(|call| call == "RotationShopInfo"));

        let broken = "ranch-market-fail";
        only_market(broken, true);
        clear_cycle(broken);
        market_clocks().lock().remove(broken);
        set_next(broken, 0);
        let failing = Arc::new(Mutex::new(MarketScript {
            poop: 6,
            calls: Vec::new(),
            buys: Vec::new(),
            fail_buy: true,
            opened: true,
        }));
        let failing_gateway = scripted_market(Arc::clone(&failing));
        assert!(patrol_if_due(&failing_gateway, broken, 1, i64::MAX, || async { Vec::new() })
            .await
            .is_err());
        assert_eq!(failing.lock().buys, vec![(11, 1)]);
        patrol_if_due(&failing_gateway, broken, 1, i64::MAX, || async { Vec::new() })
            .await
            .expect("backoff");
        assert_eq!(failing.lock().buys, vec![(11, 1)]);
    }
}
