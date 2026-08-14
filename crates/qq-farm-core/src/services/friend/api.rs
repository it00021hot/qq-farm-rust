//! 底层好友 API —— protobuf 请求/响应。
//!
//! 对应原 `core/src/services/friend/api.ts`（307 行）。

use std::sync::Arc;

use prost::Message as _;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::friendpb::{
    GameFriend, GetAllReply, GetGameFriendsReply, GetGameFriendsRequest, SyncAllReply,
    SyncAllRequest,
};
use crate::proto::generated::gamepb::plantpb::{LandInfo, OperationLimit};
use crate::proto::generated::gamepb::visitpb::{EnterReply, EnterRequest, LeaveRequest};

const DEFAULT_TIMEOUT_MS: u64 = 20_000;
const QQ_FRIEND_LIST_BATCH_SIZE: usize = 35;

/// 好友 API 客户端
#[derive(Clone)]
pub struct FriendApi {
    gateway: Arc<Gateway>,
    account_id: Arc<parking_lot::Mutex<String>>,
    on_operation_limits_update: Arc<parking_lot::Mutex<Option<crate::services::farm::api::OperationLimitsCallback>>>,
}

impl FriendApi {
    /// 创建
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            account_id: Arc::new(parking_lot::Mutex::new(String::new())),
            on_operation_limits_update: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    pub fn set_account_id(&self, account_id: &str) {
        *self.account_id.lock() = account_id.to_string();
    }

    /// 设置操作限制更新回调（对齐 TS `schedulerRef().updateOperationLimits`）
    pub fn set_operation_limits_callback(
        &self,
        cb: crate::services::farm::api::OperationLimitsCallback,
    ) {
        *self.on_operation_limits_update.lock() = Some(cb);
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
        let platform = self.gateway.platform();
        if platform.eq_ignore_ascii_case("qq") {
            self.fetch_qq_friends().await
        } else {
            self.fetch_wx_friends().await
        }
    }

    async fn fetch_wx_friends(&self) -> Result<Vec<GameFriend>> {
        let resp = self
            .gateway
            .request(
                "gamepb.friendpb.FriendService",
                "GetAll",
                &[],
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        if resp.is_empty() {
            return Ok(Vec::new());
        }
        match GetAllReply::decode(&*resp) {
            Ok(reply) => Ok(reply.game_friends),
            Err(_) => Ok(Vec::new()),
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

    /// 放草（捣乱）—— 对应原 `putWeeds` / `putWeedsDetailed`
    ///
    /// 简化实现：每块地独立发包；返回成功数。
    pub async fn put_weeds(&self, host_gid: i64, land_ids: Vec<i64>) -> Result<usize> {
        use crate::proto::generated::gamepb::plantpb::{
            PutInsectsReply, PutInsectsRequest, PutWeedsReply, PutWeedsRequest,
        };
        let mut ok = 0usize;
        for land_id in land_ids {
            let body_weed = PutWeedsRequest {
                host_gid,
                land_ids: vec![land_id],
            }
            .encode_to_vec();
            let weed_resp = self
                .gateway
                .request(
                    "gamepb.plantpb.PlantService",
                    "PutWeeds",
                    &body_weed,
                    DEFAULT_TIMEOUT_MS,
                )
                .await;
            match weed_resp {
                Ok(resp) => {
                    if let Ok(reply) = PutWeedsReply::decode(&*resp) {
                        self.fire_operation_limits(reply.operation_limits);
                        ok += 1;
                        continue;
                    }
                }
                Err(_) => {}
            }
            // 退化：发 PutInsects
            let body_insect = PutInsectsRequest {
                host_gid,
                land_ids: vec![land_id],
            }
            .encode_to_vec();
            if let Ok(resp) = self
                .gateway
                .request(
                    "gamepb.plantpb.PlantService",
                    "PutInsects",
                    &body_insect,
                    DEFAULT_TIMEOUT_MS,
                )
                .await
            {
                if let Ok(reply) = PutInsectsReply::decode(&*resp) {
                    self.fire_operation_limits(reply.operation_limits);
                    ok += 1;
                }
            }
        }
        Ok(ok)
    }

    /// 放虫（捣乱）—— 对应原 `putInsects` / `putInsectsDetailed`
    pub async fn put_insects(&self, host_gid: i64, land_ids: Vec<i64>) -> Result<usize> {
        use crate::proto::generated::gamepb::plantpb::{
            PutInsectsReply, PutInsectsRequest, PutWeedsReply, PutWeedsRequest,
        };
        let mut ok = 0usize;
        for land_id in land_ids {
            let body = PutInsectsRequest {
                host_gid,
                land_ids: vec![land_id],
            }
            .encode_to_vec();
            let resp = self
                .gateway
                .request(
                    "gamepb.plantpb.PlantService",
                    "PutInsects",
                    &body,
                    DEFAULT_TIMEOUT_MS,
                )
                .await;
            match resp {
                Ok(r) => {
                    if let Ok(reply) = PutInsectsReply::decode(&*r) {
                        self.fire_operation_limits(reply.operation_limits);
                        ok += 1;
                        continue;
                    }
                }
                _ => {}
            }
            // 退化：发 PutWeeds
            let body_weed = PutWeedsRequest {
                host_gid,
                land_ids: vec![land_id],
            }
            .encode_to_vec();
            if let Ok(r) = self
                .gateway
                .request(
                    "gamepb.plantpb.PlantService",
                    "PutWeeds",
                    &body_weed,
                    DEFAULT_TIMEOUT_MS,
                )
                .await
            {
                if let Ok(reply) = PutWeedsReply::decode(&*r) {
                    self.fire_operation_limits(reply.operation_limits);
                    ok += 1;
                }
            }
        }
        Ok(ok)
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
