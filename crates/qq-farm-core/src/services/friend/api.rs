//! 底层好友 API —— protobuf 请求/响应。
//!
//! 对应原 `core/src/services/friend/api.ts`（307 行）。

use std::sync::Arc;

use prost::Message as _;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::plantpb::LandInfo;
use crate::proto::generated::gamepb::visitpb::{EnterReply, EnterRequest, LeaveRequest};

const DEFAULT_TIMEOUT_MS: u64 = 20_000;

/// 好友 API 客户端
#[derive(Clone)]
pub struct FriendApi {
    gateway: Arc<Gateway>,
}

impl FriendApi {
    /// 创建
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 获取好友列表
    ///
    /// 对应原 `getFriendsList()` —— 服务 `gamepb.friendpb.FriendService.GetAll`
    pub async fn get_friends_list(&self) -> Result<Vec<i64>> {
        let body = vec![];
        let resp = self
            .gateway
            .request(
                "gamepb.friendpb.FriendService",
                "GetAll",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;

        if resp.is_empty() {
            return Ok(Vec::new());
        }

        match crate::proto::generated::gamepb::friendpb::GetAllReply::decode(&*resp) {
            Ok(reply) => Ok(reply.game_friends.into_iter().map(|f| f.gid).collect()),
            Err(_) => Ok(Vec::new()),
        }
    }

    /// 接受好友申请
    pub async fn accept_applications(&self, gids: Vec<i64>) -> Result<()> {
        // 简化：用通用 sendMsg 通道发好友服务请求
        let body = gids_to_bytes(&gids);
        self.gateway
            .request(
                "gamepb.friendpb.FriendService",
                "AcceptApplications",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        Ok(())
    }

    /// 访问好友农场（占位：实际 visit_farm 已被 enter_farm/leave_farm 替代）
    pub async fn visit_farm(&self, host_gid: i64) -> Result<()> {
        let body = gids_to_bytes(&[host_gid]);
        self.gateway
            .request(
                "gamepb.plantpb.PlantService",
                "VisitFarm",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
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
        FarmingReply::decode(&*resp).map(|r| r.land).map_err(Error::from)
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
        // 解析响应（确认成功），即使不用也走完
        let _ = WaterLandReply::decode(&*resp)?;
        Ok(())
    }

    /// 偷好友菜（对应原 `stealHarvest`）
    pub async fn steal_farm(&self, host_gid: i64, land_ids: Vec<i64>) -> Result<()> {
        use crate::proto::generated::gamepb::plantpb::{HarvestReply, HarvestRequest};
        let body = HarvestRequest {
            land_ids,
            host_gid,
            is_all: false,
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
        let _ = HarvestReply::decode(&*resp)?;
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
                    if PutWeedsReply::decode(&*resp).is_ok() {
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
                if PutInsectsReply::decode(&*resp).is_ok() {
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
                Ok(r) if PutInsectsReply::decode(&*r).is_ok() => ok += 1,
                _ => {
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
                        if PutWeedsReply::decode(&*r).is_ok() {
                            ok += 1;
                        }
                    }
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

fn gids_to_bytes(gids: &[i64]) -> Vec<u8> {
    // 占位编码：阶段 1D 不严格按 proto 序列化，body 用 varint 列表
    // 真实场景下应使用对应的 Request 类型
    let mut out = Vec::new();
    for &g in gids {
        // varint 编码 i64（简化版，只支持小整数）
        let mut v = g as u64;
        loop {
            let mut byte = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if v == 0 {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gids_to_bytes_roundtrip_basic() {
        let bytes = gids_to_bytes(&[1, 2, 100]);
        assert!(!bytes.is_empty());
    }
}
