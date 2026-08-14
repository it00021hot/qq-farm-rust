//! 底层好友 API —— protobuf 请求/响应。
//!
//! 对应原 `core/src/services/friend/api.ts`（307 行）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use prost::Message as _;
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::friendpb::{
    GameFriend, GetAllReply, GetAllRequest, GetGameFriendsReply, GetGameFriendsRequest, SyncAllReply,
    SyncAllRequest,
};
use crate::proto::generated::gamepb::plantpb::{LandInfo, OperationLimit};
use crate::proto::generated::gamepb::visitpb::{EnterReply, EnterRequest, LeaveRequest};

const DEFAULT_TIMEOUT_MS: u64 = 20_000;
const QQ_FRIEND_LIST_BATCH_SIZE: usize = 35;
/// 同一时刻 Vue `/api/friends` 与巡查都会打 GetAll，合并 800ms 内的成功结果。
const FRIEND_LIST_COALESCE_MS: u64 = 800;

/// 好友 API 客户端
#[derive(Clone)]
pub struct FriendApi {
    gateway: Arc<Gateway>,
    account_id: Arc<parking_lot::Mutex<String>>,
    on_operation_limits_update: Arc<parking_lot::Mutex<Option<crate::services::farm::api::OperationLimitsCallback>>>,
    /// 捣乱日限门控（对齐 TS schedulerRef）
    bad_is_limit_reached: Arc<parking_lot::Mutex<Option<Arc<dyn Fn() -> bool + Send + Sync>>>>,
    bad_remaining: Arc<parking_lot::Mutex<Option<Arc<dyn Fn() -> i64 + Send + Sync>>>>,
    bad_mark_limit: Arc<parking_lot::Mutex<Option<Arc<dyn Fn(&str) + Send + Sync>>>>,
    /// FriendService 串行（对齐 TS rate-limiter maxConcurrent=1）
    rpc_gate: Arc<AsyncMutex<()>>,
    last_list: Arc<parking_lot::Mutex<Option<(Instant, Vec<GameFriend>)>>>,
}

impl FriendApi {
    /// 创建
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            account_id: Arc::new(parking_lot::Mutex::new(String::new())),
            on_operation_limits_update: Arc::new(parking_lot::Mutex::new(None)),
            bad_is_limit_reached: Arc::new(parking_lot::Mutex::new(None)),
            bad_remaining: Arc::new(parking_lot::Mutex::new(None)),
            bad_mark_limit: Arc::new(parking_lot::Mutex::new(None)),
            rpc_gate: Arc::new(AsyncMutex::new(())),
            last_list: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    pub fn set_account_id(&self, account_id: &str) {
        *self.account_id.lock() = account_id.to_string();
    }

    /// 清空 GetAll 短缓存（面板「清除好友缓存」）
    pub fn invalidate_list_cache(&self) {
        *self.last_list.lock() = None;
    }

    /// 设置操作限制更新回调（对齐 TS `schedulerRef().updateOperationLimits`）
    pub fn set_operation_limits_callback(
        &self,
        cb: crate::services::farm::api::OperationLimitsCallback,
    ) {
        *self.on_operation_limits_update.lock() = Some(cb);
    }

    /// 设置捣乱日限门控（对齐 TS `isBadOperationLimitReached` / remaining / mark）
    pub fn set_bad_gate(
        &self,
        is_reached: Arc<dyn Fn() -> bool + Send + Sync>,
        remaining: Arc<dyn Fn() -> i64 + Send + Sync>,
        mark: Arc<dyn Fn(&str) + Send + Sync>,
    ) {
        *self.bad_is_limit_reached.lock() = Some(is_reached);
        *self.bad_remaining.lock() = Some(remaining);
        *self.bad_mark_limit.lock() = Some(mark);
    }

    fn bad_limit_reached(&self) -> bool {
        self.bad_is_limit_reached
            .lock()
            .as_ref()
            .map(|f| f())
            .unwrap_or(false)
    }

    /// 对外：剩余捣乱次数（无门控时返回较大值）
    #[must_use]
    pub fn remaining_bad_times(&self) -> i64 {
        self.bad_remaining
            .lock()
            .as_ref()
            .map(|f| f())
            .unwrap_or(999)
    }

    fn mark_bad_limit(&self, method: &str) {
        if let Some(f) = self.bad_mark_limit.lock().as_ref() {
            f(method);
        }
    }

    fn fire_operation_limits(&self, limits: Vec<OperationLimit>) {
        if limits.is_empty() {
            return;
        }
        if let Some(cb) = self.on_operation_limits_update.lock().as_ref() {
            cb(limits);
        }
    }

    /// 获取好友列表 GID（内部巡访用）
    pub async fn get_friends_list(&self) -> Result<Vec<i64>> {
        Ok(self
            .get_all_game_friends()
            .await?
            .into_iter()
            .map(|f| f.gid)
            .collect())
    }

    /// 完整 GameFriend 列表。WX 走 GetAll；QQ 走 GetGameFriends + 已知 GID。
    pub async fn get_all_game_friends(&self) -> Result<Vec<GameFriend>> {
        let _gate = self.rpc_gate.lock().await;
        if let Some((at, friends)) = self.last_list.lock().as_ref() {
            if at.elapsed() < Duration::from_millis(FRIEND_LIST_COALESCE_MS) {
                return Ok(friends.clone());
            }
        }
        let platform = self.gateway.platform();
        let friends = if platform.eq_ignore_ascii_case("qq") {
            self.fetch_qq_friends().await?
        } else {
            self.fetch_wx_friends().await?
        };
        *self.last_list.lock() = Some((Instant::now(), friends.clone()));
        Ok(friends)
    }

    async fn fetch_wx_friends(&self) -> Result<Vec<GameFriend>> {
        let body = GetAllRequest {}.encode_to_vec();
        match self
            .gateway
            .request(
                "gamepb.friendpb.FriendService",
                "GetAll",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await
        {
            Ok(resp) => Ok(decode_get_all_friends(&resp)),
            Err(e) => {
                // 对齐 gid-manager 的 GetAll 失败兜底：空 open_ids 的 SyncAll
                let fallback = SyncAllRequest {
                    open_ids: Vec::new(),
                }
                .encode_to_vec();
                match self
                    .gateway
                    .request(
                        "gamepb.friendpb.FriendService",
                        "SyncAll",
                        &fallback,
                        DEFAULT_TIMEOUT_MS,
                    )
                    .await
                {
                    Ok(resp) => {
                        if let Ok(reply) = SyncAllReply::decode(&*resp) {
                            Ok(dedupe_friends_by_gid(reply.game_friends))
                        } else {
                            Ok(Vec::new())
                        }
                    }
                    Err(_) => Err(e.into()),
                }
            }
        }
    }

    async fn fetch_qq_friends(&self) -> Result<Vec<GameFriend>> {
        let account_id = self.account_id.lock().clone();
        let known = crate::models::store::account_config::get_known_friend_gids(Some(&account_id));
        let mut all = Vec::new();
        for chunk in known.chunks(QQ_FRIEND_LIST_BATCH_SIZE) {
            let body = GetGameFriendsRequest {
                gids: chunk.to_vec(),
            }
            .encode_to_vec();
            match self
                .gateway
                .request(
                    "gamepb.friendpb.FriendService",
                    "GetGameFriends",
                    &body,
                    DEFAULT_TIMEOUT_MS,
                )
                .await
            {
                Ok(resp) => {
                    if let Ok(reply) = GetGameFriendsReply::decode(&*resp) {
                        all.extend(reply.game_friends);
                    }
                }
                Err(e) => {
                    crate::services::panel_log::log_warn(
                        &account_id,
                        "好友",
                        format!("QQ 新好友接口分批请求失败: {e}"),
                        Some(serde_json::json!({
                            "module": "friend",
                            "event": "好友列表接口",
                            "method": "GetGameFriends",
                        })),
                    );
                }
            }
        }
        all = dedupe_friends_by_gid(all);
        if !all.is_empty() {
            return Ok(all);
        }

        let body = SyncAllRequest {
            open_ids: Vec::new(),
        }
        .encode_to_vec();
        match self
            .gateway
            .request(
                "gamepb.friendpb.FriendService",
                "SyncAll",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await
        {
            Ok(resp) => {
                if let Ok(reply) = SyncAllReply::decode(&*resp) {
                    all = dedupe_friends_by_gid(reply.game_friends);
                }
            }
            Err(e) => {
                if known.is_empty() {
                    return Err(Error::Business(format!(
                        "QQ 好友列表获取失败，请先在好友页维护已知好友 GID 列表。{e}"
                    )));
                }
            }
        }
        if all.is_empty() && known.is_empty() {
            crate::services::panel_log::log_warn(
                &account_id,
                "好友",
                "QQ 好友列表为空；若近期接口已切到 GetGameFriends，请先在好友页维护已知好友 GID 列表",
                Some(serde_json::json!({
                    "module": "friend",
                    "event": "好友列表接口",
                    "result": "empty",
                })),
            );
        }
        Ok(all)
    }

    /// 拉取待处理好友申请（对齐 TS `getApplications`）
    pub async fn get_applications(&self) -> Result<Vec<(i64, String)>> {
        use crate::proto::generated::gamepb::friendpb::{GetApplicationsReply, GetApplicationsRequest};
        let _gate = self.rpc_gate.lock().await;
        let body = GetApplicationsRequest {}.encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.friendpb.FriendService",
                "GetApplications",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        let reply = GetApplicationsReply::decode(&*resp)?;
        Ok(reply
            .applications
            .into_iter()
            .filter(|a| a.gid > 0)
            .map(|a| (a.gid, a.name))
            .collect())
    }

    /// 接受好友申请（1:1 对齐原 `acceptFriends`，RPC 方法 `AcceptFriends`）
    pub async fn accept_applications(&self, gids: Vec<i64>) -> Result<()> {
        use crate::proto::generated::gamepb::friendpb::AcceptFriendsRequest;
        let _gate = self.rpc_gate.lock().await;
        let body = AcceptFriendsRequest { friend_gids: gids }.encode_to_vec();
        self.gateway
            .request(
                "gamepb.friendpb.FriendService",
                "AcceptFriends",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        Ok(())
    }

    /// 访问好友农场（对齐 enter_farm）
    pub async fn visit_farm(&self, host_gid: i64) -> Result<()> {
        let _ = self.enter_farm(host_gid).await?;
        Ok(())
    }

    /// 进入好友农场（对应原 `enterFriendFarm`）
    ///
    /// 返回 EnterReply（包含 `lands: Vec<LandInfo>` + `basic: BasicInfo`）
    ///
    /// # Errors
    /// - 网关错误
    /// - 1002003（被封）→ `is_enter_farm_banned_error` 可检测
    pub async fn enter_farm(&self, host_gid: i64) -> Result<EnterReply> {
        let body = EnterRequest {
            host_gid,
            reason: 2, // ENTER_REASON_FRIEND
        }
        .encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.visitpb.VisitService",
                "Enter",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        EnterReply::decode(&*resp).map_err(Error::from)
    }

    /// 离开好友农场（对应原 `leaveFriendFarm`）
    ///
    /// 即使失败也 swallow（不影响主流程）
    pub async fn leave_farm(&self, host_gid: i64) -> Result<()> {
        let body = LeaveRequest { host_gid }.encode_to_vec();
        // 原 TS：try { ... } catch { /* 离开失败不影响主流程 */ }
        // Rust 端：调用方自己判断
        self.gateway
            .request(
                "gamepb.visitpb.VisitService",
                "Leave",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        Ok(())
    }

    /// 帮好友锄草（对应原 `helpFarming`）
    ///
    /// FarmingRequest 字段：land_ids + host_gid + field_3(0) + field_4(2=帮)
    pub async fn help_farm(&self, host_gid: i64, land_ids: Vec<i64>) -> Result<Vec<LandInfo>> {
        use crate::proto::generated::gamepb::plantpb::{FarmingReply, FarmingRequest};
        let body = FarmingRequest {
            land_ids,
            host_gid,
            field_3: 0,
            field_4: 2,
        }
        .encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "Farming",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        FarmingReply::decode(&*resp)
            .map(|r| {
                self.fire_operation_limits(r.operation_limits);
                r.land
            })
            .map_err(Error::from)
    }

    /// 帮好友浇水（对应原 `helpWater`）
    pub async fn water_farm(&self, host_gid: i64, land_ids: Vec<i64>) -> Result<()> {
        use crate::proto::generated::gamepb::plantpb::{WaterLandReply, WaterLandRequest};
        let body = WaterLandRequest { land_ids, host_gid }.encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "WaterLand",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        let reply = WaterLandReply::decode(&*resp)?;
        self.fire_operation_limits(reply.operation_limits);
        Ok(())
    }

    /// 偷好友菜（对应原 `stealHarvest`，`is_all: true`）
    pub async fn steal_farm(&self, host_gid: i64, land_ids: Vec<i64>) -> Result<()> {
        use crate::proto::generated::gamepb::plantpb::{HarvestReply, HarvestRequest};
        let body = HarvestRequest {
            land_ids,
            host_gid,
            is_all: true,
        }
        .encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "Harvest",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        let reply = HarvestReply::decode(&*resp)?;
        self.fire_operation_limits(reply.operation_limits);
        Ok(())
    }

    /// 放草（捣乱）—— 对齐 bot `putWeedsDetailed`（按地、日限、1001046）
    pub async fn put_weeds(&self, host_gid: i64, land_ids: Vec<i64>) -> Result<usize> {
        Ok(self
            .put_plant_items_detailed(host_gid, land_ids, BadPutKind::Weeds)
            .await
            .ok)
    }

    /// 放虫（捣乱）—— 对齐 bot `putInsectsDetailed`
    pub async fn put_insects(&self, host_gid: i64, land_ids: Vec<i64>) -> Result<usize> {
        Ok(self
            .put_plant_items_detailed(host_gid, land_ids, BadPutKind::Insects)
            .await
            .ok)
    }

    /// 详细放草/放虫结果
    pub async fn put_weeds_detailed(
        &self,
        host_gid: i64,
        land_ids: Vec<i64>,
    ) -> BadPutResult {
        self.put_plant_items_detailed(host_gid, land_ids, BadPutKind::Weeds)
            .await
    }

    pub async fn put_insects_detailed(
        &self,
        host_gid: i64,
        land_ids: Vec<i64>,
    ) -> BadPutResult {
        self.put_plant_items_detailed(host_gid, land_ids, BadPutKind::Insects)
            .await
    }

    async fn put_plant_items_detailed(
        &self,
        host_gid: i64,
        land_ids: Vec<i64>,
        kind: BadPutKind,
    ) -> BadPutResult {
        use crate::network::error::NetworkError;
        use crate::proto::generated::gamepb::plantpb::{
            PutInsectsReply, PutInsectsRequest, PutWeedsReply, PutWeedsRequest,
        };

        let mut ids: Vec<i64> = land_ids.into_iter().filter(|id| *id > 0).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return BadPutResult::default();
        }
        if self.bad_limit_reached() {
            return BadPutResult {
                ok: 0,
                limit_reached: true,
                failed: ids
                    .iter()
                    .map(|id| BadPutFail {
                        land_id: *id,
                        reason: "今日放虫/放草次数已达上限".into(),
                    })
                    .collect(),
            };
        }

        let method = kind.method();
        let mut ok = 0usize;
        let mut failed = Vec::new();
        for (index, land_id) in ids.iter().copied().enumerate() {
            if self.bad_limit_reached() || self.remaining_bad_times() <= 0 {
                self.mark_bad_limit("operation_limit");
                failed.extend(ids[index..].iter().map(|id| BadPutFail {
                    land_id: *id,
                    reason: "今日放虫/放草次数已达上限".into(),
                }));
                break;
            }

            let body = match kind {
                BadPutKind::Weeds => PutWeedsRequest {
                    host_gid,
                    land_ids: vec![land_id],
                }
                .encode_to_vec(),
                BadPutKind::Insects => PutInsectsRequest {
                    host_gid,
                    land_ids: vec![land_id],
                }
                .encode_to_vec(),
            };

            match self
                .gateway
                .request(
                    "gamepb.plantpb.PlantService",
                    method,
                    &body,
                    DEFAULT_TIMEOUT_MS,
                )
                .await
            {
                Ok(resp) => {
                    let confirmed = match kind {
                        BadPutKind::Weeds => PutWeedsReply::decode(&*resp)
                            .map(|reply| {
                                self.fire_operation_limits(reply.operation_limits);
                                reply.land.iter().any(|l| l.id == land_id)
                            })
                            .unwrap_or(false),
                        BadPutKind::Insects => PutInsectsReply::decode(&*resp)
                            .map(|reply| {
                                self.fire_operation_limits(reply.operation_limits);
                                reply.land.iter().any(|l| l.id == land_id)
                            })
                            .unwrap_or(false),
                    };
                    if confirmed {
                        ok += 1;
                    } else {
                        failed.push(BadPutFail {
                            land_id,
                            reason: "服务端未确认土地状态变化".into(),
                        });
                    }
                }
                Err(e) => {
                    let limit = matches!(
                        &e,
                        NetworkError::Gateway { code: 1_001_046, .. }
                    );
                    if limit {
                        self.mark_bad_limit(method);
                        failed.extend(ids[index..].iter().map(|id| BadPutFail {
                            land_id: *id,
                            reason: "今日放虫/放草次数已达上限".into(),
                        }));
                        break;
                    }
                    failed.push(BadPutFail {
                        land_id,
                        reason: e.to_string(),
                    });
                }
            }

            if index + 1 < ids.len() && !self.bad_limit_reached() {
                let ms = 80 + (index as u64 % 80);
                tokio::time::sleep(Duration::from_millis(ms)).await;
            }
        }

        BadPutResult {
            ok,
            failed,
            limit_reached: self.bad_limit_reached(),
        }
    }

    /// 检查某操作是否可执行（对应原 `checkCanOperate`）
    ///
    /// operation_id: 10001 (water) / 10002 (weed) / 10003 (bug) / 10005 (steal) ...
    /// 返回 (can_operate, can_steal_num)
    pub async fn check_can_operate(&self, host_gid: i64, operation_id: i64) -> Result<(bool, i64)> {
        use crate::proto::generated::gamepb::plantpb::{
            CheckCanOperateReply, CheckCanOperateRequest,
        };
        let body = CheckCanOperateRequest {
            host_gid,
            operation_id,
        }
        .encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "CheckCanOperate",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        let reply = CheckCanOperateReply::decode(&*resp).map_err(Error::from)?;
        Ok((reply.can_operate, reply.can_steal_num))
    }
}

#[derive(Debug, Clone, Copy)]
enum BadPutKind {
    Weeds,
    Insects,
}

impl BadPutKind {
    fn method(self) -> &'static str {
        match self {
            Self::Weeds => "PutWeeds",
            Self::Insects => "PutInsects",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BadPutResult {
    pub ok: usize,
    pub failed: Vec<BadPutFail>,
    pub limit_reached: bool,
}

#[derive(Debug, Clone)]
pub struct BadPutFail {
    pub land_id: i64,
    pub reason: String,
}

fn decode_get_all_friends(resp: &[u8]) -> Vec<GameFriend> {
    if resp.is_empty() {
        return Vec::new();
    }
    GetAllReply::decode(resp)
        .map(|reply| reply.game_friends)
        .unwrap_or_default()
}

fn dedupe_friends_by_gid(friends: Vec<GameFriend>) -> Vec<GameFriend> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for f in friends {
        if f.gid > 0 && seen.insert(f.gid) {
            out.push(f);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friend_api_constructs() {
        let gw = Arc::new(crate::network::gateway::Gateway::new(
            crate::network::gateway::GatewayConfig {
                server_url: "ws://localhost".into(),
                platform: "test".into(),
                os: "linux".into(),
                client_version: "0.1.0".into(),
                auth_code: "x".into(),
                headers: Default::default(),
            },
            Arc::new(crate::network::encryptor::NoopEncryptor),
        ));
        let _ = FriendApi::new(gw);
    }
}
