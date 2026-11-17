//! 好友牧场：进入、偷取、抚摸、放蚊子、清理，然后离开。

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock};

use prost::Message;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::network::error::NetworkError;
use crate::network::gateway::Gateway;
use crate::proto::generated::corepb::FacilityOpResult as CoreOpResult;
use crate::proto::generated::gamepb::animalpb::{
    AnimalDetail, AnimalRef, CleanAllRanchReply, CleanAllRanchRequest, GetRanchDetailReply,
    GetRanchDetailRequest, GetRanchOpTimesReply, GetRanchOpTimesRequest, ProductLotDetail,
    PutHazardReply, PutHazardRequest, RanchDetail, RanchHazardView, ShelfDetail,
    StealProduceRequest, StrokeAnimalReply, StrokeAnimalRequest,
};
use crate::proto::generated::gamepb::ranchpb::{
    EnterHomesteadReply, EnterHomesteadRequest, FacilityOpResult as StealOpResult,
    LeaveHomesteadReply, LeaveHomesteadRequest, StealProduceReply,
};

use super::ranch::{self, animal_name};

const RANCH_SCENE: i32 = 2;
const ENTER_FRIEND: i32 = 2;
const NEST: i32 = 1;
const HAZARD_MOSQUITO: i32 = 1;
const STEAL_KIND: i32 = 2;
const STROKE_KIND: i32 = 1;
const CLEAN_KIND: i32 = 3;
const STROKE_OP: i64 = 11009;
const MOSQUITO_OP: i64 = 11003;
const NEST_LIMIT: i64 = 6001001;
const SHED_LIMIT: i64 = 6002001;
const LOCKED_REASON: &str = "牧场尚未解锁，请先在游戏中完成开启条件";
const FRIEND_VISITS_PER_CYCLE: usize = 3;
const VISIT_SERVICE: &str = "gamepb.visitpb.VisitService";
const ANIMAL_SERVICE: &str = "gamepb.animalpb.AnimalService";
const RANCH_SERVICE: &str = "gamepb.animalpb.RanchService";

#[derive(Clone, Copy)]
struct InteractionTimes {
    stroke: i64,
    mosquito: i64,
}

/// 一轮巡查要拜访的好友。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RanchFriend {
    pub gid: i64,
    pub name: String,
}

/// 好友牧场自动操作开关。
#[derive(Clone, Copy)]
pub struct FriendFlags {
    pub steal: bool,
    pub stroke: bool,
    pub clean: bool,
    pub mosquito: bool,
}

impl FriendFlags {
    pub fn any(self) -> bool {
        self.steal || self.stroke || self.clean || self.mosquito
    }
}

struct StealShelf {
    ranch_id: i64,
    slot: i32,
    item_ids: Vec<i64>,
}

/// 好友牧场当前可偷、可抚摸、可清理和可放蚊子的状态。
pub async fn friend_ranch(
    gateway: &Arc<Gateway>,
    account_id: &str,
    host: i64,
    self_gid: i64,
) -> Result<Value> {
    let owned = Arc::clone(gateway);
    visit(gateway, account_id, host, self_gid, move |ranches, entry| async move {
        let times = interaction_times(&owned).await?;
        Ok(friend_json(&entry, &ranches, self_gid, &times))
    })
    .await
}

/// 偷取好友当前可偷的货架。服务端按空目标自行筛选。
pub async fn steal_friend(
    gateway: &Arc<Gateway>,
    account_id: &str,
    host: i64,
    self_gid: i64,
) -> Result<()> {
    let owned = Arc::clone(gateway);
    visit(gateway, account_id, host, self_gid, move |ranches, _entry| async move {
        let shelves = steal_shelves(&ranches, self_gid);
        if shelves.is_empty() {
            return Err(Error::Business("当前没有可偷取的产物".to_owned()));
        }
        let request = StealProduceRequest { host_gid: host, targets: Vec::new() };
        let bytes = owned.request(ANIMAL_SERVICE, "StealProduce", &request.encode_to_vec()).await?;
        let reply = StealProduceReply::decode(bytes.as_slice())?;
        confirm_steal(&reply.op_results, &shelves)
    })
    .await
}

/// 抚摸一只还能抚摸的动物。
pub async fn stroke_friend(
    gateway: &Arc<Gateway>,
    account_id: &str,
    host: i64,
    self_gid: i64,
    ranch_id: i64,
    animal_inst_id: i32,
) -> Result<()> {
    let owned = Arc::clone(gateway);
    visit(gateway, account_id, host, self_gid, move |ranches, _entry| async move {
        let times = interaction_times(&owned).await?;
        if times.stroke <= 0 {
            return Err(Error::Business("今日互动次数已用完".to_owned()));
        }
        let Some(animal) = find_animal(&ranches, ranch_id, animal_inst_id) else {
            return Err(Error::Business("好友栏舍已变化，请刷新".to_owned()));
        };
        if !can_stroke(animal, self_gid) {
            return Err(Error::Business("这只动物已抚摸或当前不能抚摸".to_owned()));
        }
        let animal_id = animal.animal_id;
        let request = StrokeAnimalRequest {
            host_gid: host,
            target: Some(AnimalRef { ranch_inst_id: ranch_id, animal_inst_id }),
        };
        let bytes = owned.request(RANCH_SERVICE, "StrokeAnimal", &request.encode_to_vec()).await?;
        let reply = StrokeAnimalReply::decode(bytes.as_slice())?;
        if reply.remaining_times != times.stroke - 1 {
            return Err(Error::Protocol("抚摸结果未能确认，请刷新核对，勿重复提交".to_owned()));
        }
        confirm_stroke(&reply, ranch_id, animal_inst_id, animal_id, self_gid)
    })
    .await
}

/// 向一个还有空位的栏舍放一只蚊子。
pub async fn mosquito_friend(
    gateway: &Arc<Gateway>,
    account_id: &str,
    host: i64,
    self_gid: i64,
    ranch_id: i64,
) -> Result<()> {
    let owned = Arc::clone(gateway);
    visit(gateway, account_id, host, self_gid, move |ranches, _entry| async move {
        let times = interaction_times(&owned).await?;
        if times.mosquito <= 0 {
            return Err(Error::Business("今日互动次数已用完".to_owned()));
        }
        let Some(ranch) = ranches.iter().find(|ranch| ranch.ranch_inst_id == ranch_id) else {
            return Err(Error::Business("好友栏舍已变化，请刷新".to_owned()));
        };
        let limit = mosquito_limit(ranch.husbandry_type);
        if limit == 0 {
            return Err(Error::Business("该栏舍的蚊子上限尚未确认".to_owned()));
        }
        let room = limit - mosquito_count(ranch);
        if room <= 0 {
            return Err(Error::Business(format!("该栏舍已有 {limit} 只蚊子")));
        }
        let previous = ranch.hazards.iter().map(|hazard| hazard.hazard_id).collect::<Vec<_>>();
        let request = PutHazardRequest {
            ranch_inst_id: ranch_id,
            host_gid: host,
            op_id: MOSQUITO_OP,
            count: 1,
        };
        let bytes = owned.request(RANCH_SERVICE, "PutHazard", &request.encode_to_vec()).await?;
        let reply = PutHazardReply::decode(bytes.as_slice())?;
        if reply.remaining_times != times.mosquito - 1 {
            return Err(Error::Protocol("放蚊子结果未能确认，请刷新核对，勿重复提交".to_owned()));
        }
        confirm_mosquito(&reply, &previous, limit, self_gid)
    })
    .await
}

/// 清理好友栏舍里不是自己放的障碍。
pub async fn clean_friend(
    gateway: &Arc<Gateway>,
    account_id: &str,
    host: i64,
    self_gid: i64,
) -> Result<()> {
    let owned = Arc::clone(gateway);
    visit(gateway, account_id, host, self_gid, move |ranches, _entry| async move {
        let pens = cleanable_pens(&ranches, self_gid);
        if pens.is_empty() {
            return Err(Error::Business("当前已无可清理项目".to_owned()));
        }
        let request = CleanAllRanchRequest { host_gid: host, ranch_inst_ids: pens.clone() };
        let bytes = owned.request(RANCH_SERVICE, "CleanAllRanch", &request.encode_to_vec()).await?;
        let reply = CleanAllRanchReply::decode(bytes.as_slice())?;
        confirm_clean(&reply.op_results, &pens)
    })
    .await
}

/// 一轮好友拜访的进度。让路时 `pending` 是还没做完的好友。
pub struct FriendRound {
    pub visits: u8,
    pub pending: i64,
    pub yielded: bool,
}

enum Halt {
    Done,
    Yielded,
}

/// 已经在自家牧场时，按开关拜访好友。每轮最多三位，下次从下一位继续。时间到了就先离开当前好友，下次还从这位继续。
pub async fn visit_friends<P>(
    gateway: &Gateway,
    account_id: &str,
    self_gid: i64,
    friends: &[RanchFriend],
    flags: FriendFlags,
    mut visits: u8,
    mut pending: i64,
    mut pause: P,
) -> Result<FriendRound>
where
    P: FnMut(bool) -> bool,
{
    let friends = friends
        .iter()
        .filter(|friend| friend.gid > 0 && friend.gid != self_gid)
        .cloned()
        .collect::<Vec<_>>();
    if !flags.any() || friends.is_empty() || self_gid <= 0 {
        return Ok(FriendRound { visits, pending: 0, yielded: false });
    }
    let mut times = if flags.stroke || flags.mosquito {
        interaction_times(gateway).await?
    } else {
        InteractionTimes { stroke: 0, mosquito: 0 }
    };
    let mut index = friend_cursor().lock().get(account_id).copied().unwrap_or(0);
    let limit = u8::try_from(FRIEND_VISITS_PER_CYCLE.min(friends.len())).unwrap_or(u8::MAX);
    while visits < limit && friend_work_left(flags, times) {
        if pause(false) {
            friend_cursor().lock().insert(account_id.to_owned(), index);
            return Ok(FriendRound { visits, pending, yielded: true });
        }
        let friend = if pending > 0 {
            friends
                .iter()
                .find(|friend| friend.gid == pending)
                .cloned()
                .unwrap_or_else(|| friends[index % friends.len()].clone())
        } else {
            friends[index % friends.len()].clone()
        };
        match visit_one(gateway, account_id, self_gid, &friend, flags, &mut times, &mut pause).await
        {
            Ok(Halt::Done) => {
                if pending == friend.gid {
                    pending = 0;
                }
                index += 1;
                visits = visits.saturating_add(1);
            }
            Ok(Halt::Yielded) => {
                pending = friend.gid;
                friend_cursor().lock().insert(account_id.to_owned(), index);
                return Ok(FriendRound { visits, pending, yielded: true });
            }
            Err(error) if enter_rejected(&error) => {
                tracing::warn!(
                    account_id,
                    gid = friend.gid,
                    name = friend.name,
                    %error,
                    "牧场巡查暂不能拜访这位好友"
                );
                if pending == friend.gid {
                    pending = 0;
                }
                index += 1;
                visits = visits.saturating_add(1);
            }
            Err(error) => {
                friend_cursor().lock().insert(account_id.to_owned(), index);
                return Err(error);
            }
        }
    }
    friend_cursor().lock().insert(account_id.to_owned(), index);
    Ok(FriendRound { visits, pending: 0, yielded: false })
}

fn friend_cursor() -> &'static parking_lot::Mutex<HashMap<String, usize>> {
    static CURSOR: OnceLock<parking_lot::Mutex<HashMap<String, usize>>> = OnceLock::new();
    CURSOR.get_or_init(|| parking_lot::Mutex::new(HashMap::new()))
}

fn friend_work_left(flags: FriendFlags, times: InteractionTimes) -> bool {
    flags.steal
        || flags.clean
        || (flags.stroke && times.stroke > 0)
        || (flags.mosquito && times.mosquito > 0)
}

fn enter_rejected(error: &Error) -> bool {
    matches!(
        error,
        Error::Network(NetworkError::Gateway { method_name, .. }) if method_name == "EnterHomestead"
    )
}

#[cfg(test)]
fn visit_indexes(friend_count: usize, start: usize, limit: usize) -> Vec<usize> {
    if friend_count == 0 || limit == 0 {
        return Vec::new();
    }
    let count = limit.min(friend_count);
    (0..count).map(|offset| start.saturating_add(offset) % friend_count).collect()
}

/// 好友列表 JSON 里的一位好友。
#[must_use]
pub fn friend_from_list_row(row: &Value) -> Option<RanchFriend> {
    let gid = row.get("gid")?.as_i64()?;
    if gid <= 0 {
        return None;
    }
    let name = row.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
    Some(RanchFriend { gid, name })
}

async fn visit_one<P>(
    gateway: &Gateway,
    account_id: &str,
    self_gid: i64,
    friend: &RanchFriend,
    flags: FriendFlags,
    times: &mut InteractionTimes,
    pause: &mut P,
) -> Result<Halt>
where
    P: FnMut(bool) -> bool,
{
    let host = friend.gid;
    let mut entered = false;
    let outcome = async {
        let entry = enter_friend(gateway, host).await?;
        entered = true;
        if entry.basic.as_ref().map(|basic| basic.gid) != Some(host)
            || entry.homestead_cells.is_none()
        {
            return Err(Error::Protocol("好友牧场回包与目标不匹配".to_owned()));
        }
        let mut ranches = friend_ranches(gateway, host).await?;
        if pause(false) {
            return Ok(Halt::Yielded);
        }
        if flags.clean {
            match clean_visit(gateway, host, self_gid, &ranches).await {
                Ok(true) => {
                    tracing::info!(
                        account_id,
                        gid = host,
                        name = friend.name,
                        "牧场巡查已清理好友牧场"
                    );
                    ranches = friend_ranches(gateway, host).await?;
                    if pause(true) {
                        return Ok(Halt::Yielded);
                    }
                }
                Ok(false) => {}
                Err(Error::Business(message)) => {
                    tracing::warn!(account_id, gid = host, %message, "牧场巡查跳过清理好友牧场");
                }
                Err(error) => return Err(error),
            }
        }
        if pause(false) {
            return Ok(Halt::Yielded);
        }
        if flags.steal {
            match steal_visit(gateway, host, self_gid, &ranches).await {
                Ok(true) => {
                    tracing::info!(
                        account_id,
                        gid = host,
                        name = friend.name,
                        "牧场巡查已偷取好友产物"
                    );
                    if pause(true) {
                        return Ok(Halt::Yielded);
                    }
                }
                Ok(false) => {}
                Err(Error::Business(message)) => {
                    tracing::warn!(account_id, gid = host, %message, "牧场巡查跳过偷取好友产物");
                }
                Err(error) => return Err(error),
            }
        }
        for ranch_index in 0..ranches.len() {
            if flags.stroke && times.stroke > 0 {
                let targets = stroke_targets(&ranches[ranch_index], self_gid);
                for target in targets {
                    if times.stroke <= 0 {
                        break;
                    }
                    if pause(false) {
                        return Ok(Halt::Yielded);
                    }
                    match stroke_visit(gateway, host, self_gid, &target, times.stroke).await {
                        Ok(remaining) => {
                            times.stroke = remaining;
                            tracing::info!(
                                account_id,
                                gid = host,
                                name = friend.name,
                                animal_id = target.animal_id,
                                "牧场巡查已抚摸好友动物"
                            );
                            if pause(true) {
                                return Ok(Halt::Yielded);
                            }
                        }
                        Err(Error::Business(message)) => {
                            tracing::warn!(account_id, gid = host, %message, "牧场巡查跳过抚摸");
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            if flags.mosquito && times.mosquito > 0 {
                let ranch_id = ranches[ranch_index].ranch_inst_id;
                loop {
                    let Some(ranch) = ranches.iter().find(|ranch| ranch.ranch_inst_id == ranch_id)
                    else {
                        break;
                    };
                    let limit = mosquito_limit(ranch.husbandry_type);
                    let room = (limit - mosquito_count(ranch)).max(0);
                    if limit == 0 || room <= 0 || times.mosquito <= 0 {
                        break;
                    }
                    if pause(false) {
                        return Ok(Halt::Yielded);
                    }
                    let previous =
                        ranch.hazards.iter().map(|hazard| hazard.hazard_id).collect::<Vec<_>>();
                    match mosquito_visit(
                        gateway,
                        host,
                        self_gid,
                        ranch_id,
                        &previous,
                        limit,
                        times.mosquito,
                    )
                    .await
                    {
                        Ok(remaining) => {
                            times.mosquito = remaining;
                            tracing::info!(
                                account_id,
                                gid = host,
                                name = friend.name,
                                ranch_id,
                                "牧场巡查已向好友栏舍放蚊子"
                            );
                            if pause(true) {
                                return Ok(Halt::Yielded);
                            }
                            let latest = friend_ranches(gateway, host).await?;
                            let Some(pen) =
                                latest.into_iter().find(|ranch| ranch.ranch_inst_id == ranch_id)
                            else {
                                break;
                            };
                            if let Some(slot) =
                                ranches.iter_mut().find(|ranch| ranch.ranch_inst_id == ranch_id)
                            {
                                slot.hazards = pen.hazards;
                            }
                        }
                        Err(Error::Business(message)) => {
                            tracing::warn!(account_id, gid = host, %message, "牧场巡查跳过放蚊子");
                            break;
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        }
        Ok(Halt::Done)
    }
    .await;
    if entered {
        if let Err(error) = leave_friend(gateway, host).await {
            tracing::error!(%error, "离开好友牧场失败");
            if outcome.is_ok() {
                return Err(Error::Protocol("好友牧场离场未确认，请核对操作结果".to_owned()));
            }
        }
    }
    outcome
}

struct StrokeTarget {
    ranch_id: i64,
    animal_inst_id: i32,
    animal_id: i64,
}

fn stroke_targets(ranch: &RanchDetail, self_gid: i64) -> Vec<StrokeTarget> {
    ranch
        .animals
        .iter()
        .filter(|animal| can_stroke(animal, self_gid))
        .map(|animal| StrokeTarget {
            ranch_id: ranch.ranch_inst_id,
            animal_inst_id: animal.animal_inst_id,
            animal_id: animal.animal_id,
        })
        .collect()
}

async fn clean_visit(
    gateway: &Gateway,
    host: i64,
    self_gid: i64,
    ranches: &[RanchDetail],
) -> Result<bool> {
    let pens = cleanable_pens(ranches, self_gid);
    if pens.is_empty() {
        return Ok(false);
    }
    let request = CleanAllRanchRequest { host_gid: host, ranch_inst_ids: pens.clone() };
    let bytes = gateway.request(RANCH_SERVICE, "CleanAllRanch", &request.encode_to_vec()).await?;
    let reply = CleanAllRanchReply::decode(bytes.as_slice())?;
    if reply.op_results.is_empty() {
        return Ok(false);
    }
    confirm_clean(&reply.op_results, &pens)?;
    Ok(true)
}

async fn steal_visit(
    gateway: &Gateway,
    host: i64,
    self_gid: i64,
    ranches: &[RanchDetail],
) -> Result<bool> {
    let shelves = steal_shelves(ranches, self_gid);
    if shelves.is_empty() {
        return Ok(false);
    }
    let request = StealProduceRequest { host_gid: host, targets: Vec::new() };
    let bytes = gateway.request(ANIMAL_SERVICE, "StealProduce", &request.encode_to_vec()).await?;
    let reply = StealProduceReply::decode(bytes.as_slice())?;
    confirm_steal(&reply.op_results, &shelves)?;
    Ok(!reply.op_results.is_empty())
}

async fn stroke_visit(
    gateway: &Gateway,
    host: i64,
    self_gid: i64,
    target: &StrokeTarget,
    remaining: i64,
) -> Result<i64> {
    if remaining <= 0 {
        return Err(Error::Business("今日互动次数已用完".to_owned()));
    }
    let request = StrokeAnimalRequest {
        host_gid: host,
        target: Some(AnimalRef {
            ranch_inst_id: target.ranch_id,
            animal_inst_id: target.animal_inst_id,
        }),
    };
    let bytes = gateway.request(RANCH_SERVICE, "StrokeAnimal", &request.encode_to_vec()).await?;
    let reply = StrokeAnimalReply::decode(bytes.as_slice())?;
    if reply.remaining_times != remaining - 1 {
        return Err(Error::Protocol("抚摸结果未能确认，请刷新核对，勿重复提交".to_owned()));
    }
    confirm_stroke(&reply, target.ranch_id, target.animal_inst_id, target.animal_id, self_gid)?;
    Ok(reply.remaining_times)
}

async fn mosquito_visit(
    gateway: &Gateway,
    host: i64,
    self_gid: i64,
    ranch_id: i64,
    previous: &[i64],
    limit: i64,
    remaining: i64,
) -> Result<i64> {
    if remaining <= 0 {
        return Err(Error::Business("今日互动次数已用完".to_owned()));
    }
    let request =
        PutHazardRequest { ranch_inst_id: ranch_id, host_gid: host, op_id: MOSQUITO_OP, count: 1 };
    let bytes = gateway.request(RANCH_SERVICE, "PutHazard", &request.encode_to_vec()).await?;
    let reply = PutHazardReply::decode(bytes.as_slice())?;
    if reply.remaining_times != remaining - 1 {
        return Err(Error::Protocol("放蚊子结果未能确认，请刷新核对，勿重复提交".to_owned()));
    }
    confirm_mosquito(&reply, previous, limit, self_gid)?;
    Ok(reply.remaining_times)
}

async fn visit<T, F, Fut>(
    gateway: &Arc<Gateway>,
    account_id: &str,
    host: i64,
    self_gid: i64,
    work: F,
) -> Result<T>
where
    F: FnOnce(Vec<RanchDetail>, EnterHomesteadReply) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T>> + Send,
    T: Send + 'static,
{
    require_friend(host, self_gid)?;
    if !gateway.scene_unlocked(RANCH_SCENE) {
        return Err(Error::Business(LOCKED_REASON.to_owned()));
    }
    let owned = Arc::clone(gateway);
    ranch::in_ranch(gateway, account_id, move |_cells, _bag| async move {
        let mut entered = false;
        let outcome = async {
            let entry = enter_friend(&owned, host).await?;
            entered = true;
            if entry.basic.as_ref().map(|basic| basic.gid) != Some(host)
                || entry.homestead_cells.is_none()
            {
                return Err(Error::Protocol("好友牧场回包与目标不匹配".to_owned()));
            }
            let ranches = friend_ranches(&owned, host).await?;
            work(ranches, entry).await
        }
        .await;
        if entered {
            if let Err(error) = leave_friend(&owned, host).await {
                tracing::error!(%error, "离开好友牧场失败");
                if outcome.is_ok() {
                    return Err(Error::Protocol("好友牧场离场未确认，请核对操作结果".to_owned()));
                }
            }
        }
        outcome
    })
    .await
}

fn require_friend(host: i64, self_gid: i64) -> Result<()> {
    if host <= 0 || host == self_gid {
        Err(Error::Business("请选择好友牧场".to_owned()))
    } else {
        Ok(())
    }
}

async fn enter_friend(gateway: &Gateway, host: i64) -> Result<EnterHomesteadReply> {
    let request = EnterHomesteadRequest { host_gid: host, reason: ENTER_FRIEND };
    let bytes = gateway.request(VISIT_SERVICE, "EnterHomestead", &request.encode_to_vec()).await?;
    Ok(EnterHomesteadReply::decode(bytes.as_slice())?)
}

async fn leave_friend(gateway: &Gateway, host: i64) -> Result<()> {
    let request = LeaveHomesteadRequest { host_gid: host };
    let bytes = gateway.request(VISIT_SERVICE, "LeaveHomestead", &request.encode_to_vec()).await?;
    LeaveHomesteadReply::decode(bytes.as_slice())?;
    Ok(())
}

async fn friend_ranches(gateway: &Gateway, host: i64) -> Result<Vec<RanchDetail>> {
    let request = GetRanchDetailRequest { host_gid: host, ranch_inst_ids: Vec::new() };
    let bytes = gateway.request(ANIMAL_SERVICE, "GetRanchDetail", &request.encode_to_vec()).await?;
    Ok(GetRanchDetailReply::decode(bytes.as_slice())?.ranches)
}

async fn interaction_times(gateway: &Gateway) -> Result<InteractionTimes> {
    let request = GetRanchOpTimesRequest { op_ids: Vec::new() };
    let bytes = gateway.request(RANCH_SERVICE, "GetRanchOpTimes", &request.encode_to_vec()).await?;
    let reply = GetRanchOpTimesReply::decode(bytes.as_slice())?;
    Ok(InteractionTimes {
        stroke: op_remaining(&reply, STROKE_OP)?,
        mosquito: op_remaining(&reply, MOSQUITO_OP)?,
    })
}

fn op_remaining(reply: &GetRanchOpTimesReply, op_id: i64) -> Result<i64> {
    let Some(op) = reply.ops.iter().find(|op| op.op_id == op_id) else {
        return Err(Error::Protocol("牧场互动剩余次数未返回".to_owned()));
    };
    if op.remaining_times < 0 {
        return Err(Error::Protocol("牧场互动剩余次数无效".to_owned()));
    }
    Ok(op.remaining_times)
}

fn steal_shelves(ranches: &[RanchDetail], self_gid: i64) -> Vec<StealShelf> {
    let mut shelves = Vec::new();
    for ranch in ranches {
        for shelf in &ranch.shelves {
            if stealable_shelf(shelf, self_gid) {
                shelves.push(StealShelf {
                    ranch_id: ranch.ranch_inst_id,
                    slot: shelf.slot_index,
                    item_ids: stealable_items(shelf, self_gid),
                });
            }
        }
    }
    shelves
}

fn stealable_shelf(shelf: &ShelfDetail, self_gid: i64) -> bool {
    shelf.can_steal
        && shelf.lots.iter().any(|lot| stealable_lot(lot, self_gid) && lot.remain_num > 0)
}

fn stealable_items(shelf: &ShelfDetail, self_gid: i64) -> Vec<i64> {
    shelf.lots.iter().filter(|lot| stealable_lot(lot, self_gid)).map(|lot| lot.item_id).collect()
}

fn stealable_lot(lot: &ProductLotDetail, self_gid: i64) -> bool {
    lot.can_steal && !lot.stealers.contains(&self_gid)
}

fn can_stroke(animal: &AnimalDetail, self_gid: i64) -> bool {
    animal.mutation.as_ref().is_some_and(|mutation| mutation.strokable)
        && !stroked(animal, self_gid)
}

fn stroked(animal: &AnimalDetail, self_gid: i64) -> bool {
    animal.mutation.as_ref().is_some_and(|mutation| mutation.stroked_gids.contains(&self_gid))
}

fn mosquito_limit(husbandry: i32) -> i64 {
    let facility = if husbandry == NEST { NEST_LIMIT } else { SHED_LIMIT };
    if husbandry != NEST && husbandry != NEST + 1 {
        return 0;
    }
    mosquito_limits().get(&facility).copied().unwrap_or(0)
}

fn mosquito_count(ranch: &RanchDetail) -> i64 {
    ranch.hazards.iter().filter(|hazard| hazard.hazard_type == HAZARD_MOSQUITO).count() as i64
}

fn cleanable_hazards(ranch: &RanchDetail, self_gid: i64) -> Vec<&RanchHazardView> {
    ranch
        .hazards
        .iter()
        .filter(|hazard| hazard.hazard_type != HAZARD_MOSQUITO || hazard.owner != self_gid)
        .collect()
}

fn cleanable_pens(ranches: &[RanchDetail], self_gid: i64) -> Vec<i64> {
    ranches
        .iter()
        .filter(|ranch| !cleanable_hazards(ranch, self_gid).is_empty())
        .map(|ranch| ranch.ranch_inst_id)
        .collect()
}

fn find_animal(
    ranches: &[RanchDetail],
    ranch_id: i64,
    animal_inst_id: i32,
) -> Option<&AnimalDetail> {
    ranches.iter().find(|ranch| ranch.ranch_inst_id == ranch_id).and_then(|ranch| {
        ranch.animals.iter().find(|animal| animal.animal_inst_id == animal_inst_id)
    })
}

fn confirm_steal(results: &[StealOpResult], shelves: &[StealShelf]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for row in results {
        let Some(shelf) = shelves.iter().find(|shelf| {
            shelf.ranch_id == row.facility_inst_id && i64::from(shelf.slot) == row.target_id
        }) else {
            return Err(Error::Protocol("偷取回包未能确认，请刷新核对，勿重复提交".to_owned()));
        };
        let key = (row.facility_inst_id, row.target_id);
        if row.kind != STEAL_KIND
            || !seen.insert(key)
            || !row.costs.is_empty()
            || row.rewards.is_empty()
            || row.rewards.iter().any(|item| !shelf.item_ids.contains(&item.id) || item.count <= 0)
        {
            return Err(Error::Protocol("偷取回包未能确认，请刷新核对，勿重复提交".to_owned()));
        }
    }
    Ok(())
}

fn confirm_stroke(
    reply: &StrokeAnimalReply,
    ranch_id: i64,
    animal_inst_id: i32,
    animal_id: i64,
    self_gid: i64,
) -> Result<()> {
    let Some(result) = reply.op_result.as_ref() else {
        return Err(Error::Protocol("抚摸结果未能确认，请刷新核对，勿重复提交".to_owned()));
    };
    let animal = find_animal(&reply.ranches, ranch_id, animal_inst_id);
    let matched = result.kind == STROKE_KIND
        && result.facility_inst_id == ranch_id
        && result.target_id == i64::from(animal_inst_id)
        && result.costs.is_empty()
        && animal.is_some_and(|animal| {
            animal.animal_id == animal_id
                && stroked(animal, self_gid)
                && (!reply.triggered
                    || animal.mutation.as_ref().is_some_and(|mutation| mutation.mutation_type != 0))
        })
        && result.rewards.iter().all(|item| item.count > 0);
    if matched {
        Ok(())
    } else {
        Err(Error::Protocol("抚摸结果未能确认，请刷新核对，勿重复提交".to_owned()))
    }
}

fn confirm_mosquito(
    reply: &PutHazardReply,
    previous: &[i64],
    limit: i64,
    self_gid: i64,
) -> Result<()> {
    let mosquitoes = reply.hazards.iter().filter(|hazard| hazard.hazard_type == HAZARD_MOSQUITO);
    let fresh = mosquitoes
        .filter(|hazard| hazard.owner == self_gid && !previous.contains(&hazard.hazard_id))
        .count();
    let unique = reply.hazards.iter().map(|hazard| hazard.hazard_id).collect::<BTreeSet<_>>().len()
        == reply.hazards.len();
    let mosquito_total =
        reply.hazards.iter().filter(|hazard| hazard.hazard_type == HAZARD_MOSQUITO).count() as i64;
    if reply.actual_count == 1
        && fresh == 1
        && mosquito_total <= limit
        && unique
        && reply.rewards.iter().all(|item| item.count > 0)
    {
        Ok(())
    } else {
        Err(Error::Protocol("放蚊子结果未能确认，请刷新核对，勿重复提交".to_owned()))
    }
}

fn confirm_clean(results: &[CoreOpResult], pens: &[i64]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for row in results {
        let key = (row.facility_inst_id, row.target_id);
        if row.kind != CLEAN_KIND
            || !pens.contains(&row.facility_inst_id)
            || row.target_id <= 0
            || !row.costs.is_empty()
            || !seen.insert(key)
            || row.rewards.iter().any(|item| item.count <= 0)
        {
            return Err(Error::Protocol(
                "清理好友牧场结果未能确认，请刷新核对，勿重复提交".to_owned(),
            ));
        }
    }
    Ok(())
}

fn friend_json(
    entry: &EnterHomesteadReply,
    ranches: &[RanchDetail],
    self_gid: i64,
    times: &InteractionTimes,
) -> Value {
    let basic = entry.basic.as_ref();
    json!({
        "name": basic.map(|basic| basic.name.clone()).unwrap_or_default(),
        "level": basic.map(|basic| basic.big_farm_level).unwrap_or(0),
        "canSteal": !steal_shelves(ranches, self_gid).is_empty(),
        "cleanable": cleanable_pens(ranches, self_gid).len(),
        "strokeRemaining": times.stroke,
        "mosquitoRemaining": times.mosquito,
        "pens": ranches.iter().map(|ranch| pen_json(ranch, self_gid, times.mosquito)).collect::<Vec<_>>(),
    })
}

fn pen_json(ranch: &RanchDetail, self_gid: i64, mosquito_remaining: i64) -> Value {
    let limit = mosquito_limit(ranch.husbandry_type);
    let room = (limit - mosquito_count(ranch)).max(0);
    json!({
        "id": ranch.ranch_inst_id.to_string(),
        "name": if ranch.husbandry_type == NEST { "窝" } else { "棚" },
        "canMosquito": mosquito_remaining > 0 && room > 0 && limit > 0,
        "animals": ranch.animals.iter().filter(|animal| can_stroke(animal, self_gid)).map(|animal| json!({
            "id": animal.animal_inst_id,
            "name": animal_name(animal.animal_id),
        })).collect::<Vec<_>>(),
    })
}

fn mosquito_limits() -> &'static HashMap<i64, i64> {
    static LIMITS: OnceLock<HashMap<i64, i64>> = OnceLock::new();
    LIMITS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Row {
            id: i64,
            mosquito_max_count: i64,
        }
        serde_json::from_str::<Vec<Row>>(include_str!(
            "../../assets/ranch/MCRanchFacilityConfig.json"
        ))
        .expect("牧场栏舍蚊子上限无法解析")
        .into_iter()
        .map(|row| (row.id, row.mosquito_max_count))
        .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::animalpb::AnimalMutationView;
    use crate::proto::generated::gamepb::ranchpb::Item as RanchItem;

    fn lot(item_id: i64, remain: i64, stealers: Vec<i64>) -> ProductLotDetail {
        ProductLotDetail {
            item_id,
            remain_num: remain,
            can_steal: true,
            stealers,
            ..ProductLotDetail::default()
        }
    }

    fn ranch_with_shelf(stealers: Vec<i64>) -> RanchDetail {
        RanchDetail {
            ranch_inst_id: 9,
            husbandry_type: NEST,
            shelves: vec![ShelfDetail {
                slot_index: 1,
                can_steal: true,
                lots: vec![lot(2060001, 2, stealers)],
                ..ShelfDetail::default()
            }],
            ..RanchDetail::default()
        }
    }

    #[test]
    fn steal_skips_a_shelf_already_taken_by_me() {
        assert!(require_friend(7, 7).is_err());
        let open = steal_shelves(&[ranch_with_shelf(vec![4])], 8);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].item_ids, vec![2060001]);
        assert!(steal_shelves(&[ranch_with_shelf(vec![8])], 8).is_empty());
        let result = StealOpResult {
            facility_inst_id: 9,
            kind: STEAL_KIND,
            target_id: 1,
            rewards: vec![RanchItem { id: 2060001, count: 1, ..RanchItem::default() }],
            ..StealOpResult::default()
        };
        assert!(confirm_steal(&[result], &open).is_ok());
        let wrong = StealOpResult {
            facility_inst_id: 9,
            kind: 1,
            target_id: 1,
            ..StealOpResult::default()
        };
        assert!(confirm_steal(&[wrong], &open).is_err());
    }

    #[test]
    fn stroke_mosquito_and_clean_follow_the_pen_state() {
        let strokable = AnimalDetail {
            animal_inst_id: 3,
            animal_id: 60001,
            mutation: Some(AnimalMutationView { strokable: true, ..AnimalMutationView::default() }),
            ..AnimalDetail::default()
        };
        let ranch = RanchDetail {
            ranch_inst_id: 9,
            husbandry_type: NEST,
            animals: vec![strokable],
            hazards: vec![
                RanchHazardView { hazard_id: 1, owner: 8, hazard_type: HAZARD_MOSQUITO },
                RanchHazardView { hazard_id: 2, owner: 4, hazard_type: 2 },
            ],
            ..RanchDetail::default()
        };
        let ranches = [ranch.clone()];
        let animal = find_animal(&ranches, 9, 3).unwrap();
        assert!(can_stroke(animal, 8));
        assert_eq!(mosquito_limit(NEST), 3);
        assert_eq!(mosquito_count(&ranch), 1);
        assert_eq!(cleanable_pens(&[ranch], 8), vec![9]);
        let reply = PutHazardReply {
            actual_count: 1,
            hazards: vec![
                RanchHazardView { hazard_id: 1, owner: 8, hazard_type: HAZARD_MOSQUITO },
                RanchHazardView { hazard_id: 5, owner: 8, hazard_type: HAZARD_MOSQUITO },
            ],
            ..PutHazardReply::default()
        };
        assert!(confirm_mosquito(&reply, &[1, 2], 3, 8).is_ok());
        assert!(confirm_mosquito(&reply, &[1, 2, 5], 3, 8).is_err());
    }

    #[test]
    fn friend_visits_rotate_and_stop_when_interaction_times_run_out() {
        assert_eq!(visit_indexes(5, 3, FRIEND_VISITS_PER_CYCLE), vec![3, 4, 0]);
        assert_eq!(visit_indexes(2, 1, FRIEND_VISITS_PER_CYCLE), vec![1, 0]);
        assert!(visit_indexes(0, 0, FRIEND_VISITS_PER_CYCLE).is_empty());
        let times = InteractionTimes { stroke: 0, mosquito: 0 };
        let stroke = FriendFlags { steal: false, stroke: true, clean: false, mosquito: false };
        assert!(!friend_work_left(stroke, times));
        let steal = FriendFlags { steal: true, stroke: false, clean: false, mosquito: false };
        assert!(friend_work_left(steal, times));
        let row = json!({ "gid": 9, "name": "邻居" });
        assert_eq!(friend_from_list_row(&row).expect("friend").name, "邻居");
        assert!(friend_from_list_row(&json!({ "gid": 0, "name": "空" })).is_none());
        let rejected = Error::Network(NetworkError::Gateway {
            code: 1,
            service_name: VISIT_SERVICE.to_owned(),
            method_name: "EnterHomestead".to_owned(),
            error_message: "不能进入".to_owned(),
            client_seq: 1,
        });
        assert!(enter_rejected(&rejected));
        assert!(!enter_rejected(&Error::Business("余额不足".to_owned())));
    }
}
