//! 支付 / 充值 — `PayService.GetRechargeInfo` 封装。
//!
//! 1:1 翻译原 `core/src/services/pay.ts`（33 行）。
//!
//! ## 协议
//!
//! - `gamepb.paypb.PayService.GetRechargeInfo(source)` — 拉取充值档位 / 余额信息
//!
//! ## 业务
//!
//! - 解析首条 `recharge_infos[0].balance` 作为"钻石余额"
//! - source 缺省为 `MallUI`（前端商城来源）

use std::sync::Arc;

use prost::Message;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::paypb::{
    GetRechargeInfoReply, GetRechargeInfoRequest,
};

const PAY_SERVICE: &str = "gamepb.paypb.PayService";
/// 默认 `source` 字段，1:1 对齐原 TS `DEFAULT_RECHARGE_SOURCE = 'MallUI'`
pub const DEFAULT_RECHARGE_SOURCE: &str = "MallUI";

/// 支付 / 充值服务
pub struct PayService {
    gateway: Arc<Gateway>,
}

impl PayService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 拉取充值档位 / 余额信息
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn get_recharge_info(&self, source: &str) -> Result<GetRechargeInfoReply> {
        let req = GetRechargeInfoRequest {
            source: source.to_string(),
        };
        let body = self
            .gateway
            .request(PAY_SERVICE, "GetRechargeInfo", &req.encode_to_vec(), 10_000)
            .await?;
        Ok(GetRechargeInfoReply::decode(&body[..])?)
    }

    /// 读取首条 RechargeInfo 的 `balance` 字段作为钻石余额
    ///
    /// # Errors
    /// - 同 [`Self::get_recharge_info`]
    pub async fn get_diamond_balance(&self) -> Result<i64> {
        let reply = self.get_recharge_info(DEFAULT_RECHARGE_SOURCE).await?;
        let infos = reply.recharge_infos;
        let first = infos.first();
        let raw = first.map_or(0, |info| info.balance);
        Ok(raw.max(0))
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::generated::gamepb::paypb::RechargeInfo;

    #[test]
    fn default_source_constant() {
        assert_eq!(DEFAULT_RECHARGE_SOURCE, "MallUI");
    }

    #[test]
    fn service_constant_matches_ts() {
        assert_eq!(PAY_SERVICE, "gamepb.paypb.PayService");
    }

    #[test]
    fn encode_decode_roundtrip() {
        let mut reply = GetRechargeInfoReply::default();
        reply.recharge_infos.push(RechargeInfo {
            balance: 1024,
            field_3: 0,
        });
        let bytes = reply.encode_to_vec();
        let back = GetRechargeInfoReply::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.recharge_infos.len(), 1);
        assert_eq!(back.recharge_infos[0].balance, 1024);
    }

    #[test]
    fn diamond_balance_handles_empty() {
        // 纯函数测试：empty list -> 0
        let reply = GetRechargeInfoReply::default();
        let first = reply.recharge_infos.first();
        let raw = first.map_or(0, |info| info.balance);
        assert_eq!(raw.max(0), 0);
    }

    #[test]
    fn diamond_balance_takes_first() {
        let mut reply = GetRechargeInfoReply::default();
        reply.recharge_infos.push(RechargeInfo {
            balance: 100,
            field_3: 0,
        });
        reply.recharge_infos.push(RechargeInfo {
            balance: 999,
            field_3: 0,
        });
        let first = reply.recharge_infos.first();
        let raw = first.map_or(0, |info| info.balance);
        assert_eq!(raw.max(0), 100);
    }
}
