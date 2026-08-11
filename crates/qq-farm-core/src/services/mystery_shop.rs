//! 神秘商店 — GetActiveNPC / Buy RPC 封装。
//!
//! 1:1 翻译原 `core/src/services/mystery-shop.ts`（26 行）。
//!
//! ## 协议
//!
//! - `gamepb.mysteryshoppb.MysteryShopService.GetActiveNPC` — 拉取当前生效的神秘商店 NPC
//! - `gamepb.mysteryshoppb.MysteryShopService.Buy` — 购买（无返回体，依赖 ItemNotify 推送）
//!
//! ## 业务
//!
//! - 本模块只做协议封装，不解析价格 / 库存 / 折扣
//! - 业务编排（DTO、库存检查、余额校验）见 [`super::commerce`]

use std::sync::Arc;

use prost::Message;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::mysteryshoppb::{
    BuyRequest, GetActiveNpcReply, GetActiveNpcRequest,
};

const MYSTERY_SHOP_SERVICE: &str = "gamepb.mysteryshoppb.MysteryShopService";

/// 神秘商店服务
pub struct MysteryShopService {
    gateway: Arc<Gateway>,
}

impl MysteryShopService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 拉取当前生效的神秘商店 NPC
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn get_active_npc(&self) -> Result<GetActiveNpcReply> {
        let body = self
            .gateway
            .request(
                MYSTERY_SHOP_SERVICE,
                "GetActiveNPC",
                &GetActiveNpcRequest {}.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(GetActiveNpcReply::decode(&body[..])?)
    }

    /// 购买指定 NPC 的商品
    ///
    /// # Errors
    /// - 网络 / 网关错误
    pub async fn buy(&self, npc_id: i64) -> Result<()> {
        let req = BuyRequest { npc_id };
        self.gateway
            .request(
                MYSTERY_SHOP_SERVICE,
                "Buy",
                &req.encode_to_vec(),
                10_000,
            )
            .await?;
        Ok(())
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_constant_matches_ts() {
        // 1:1 对齐原 TS 中的常量
        assert_eq!(MYSTERY_SHOP_SERVICE, "gamepb.mysteryshoppb.MysteryShopService");
    }

    #[test]
    fn buy_request_field() {
        let req = BuyRequest { npc_id: 12345 };
        let bytes = req.encode_to_vec();
        let back = BuyRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.npc_id, 12345);
    }
}
