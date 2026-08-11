//! 底层农场 API —— protobuf 请求、商店、铲除。
//!
//! 对应原 `core/src/services/farm/api.ts`。
//!
//! ## 用法
//!
//! 业务层通过 [`Api`] 调用：
//! ```ignore
//! let api = Api::new(gateway.clone());
//! let lands = api.get_all_lands().await?;
//! let harvested = api.harvest(&[1, 2, 3]).await?;
//! ```

use std::sync::Arc;

use prost::Message as _;

use crate::error::{Error, Result};
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::plantpb::{
    AllLandsReply, AllLandsRequest, FarmingReply, FarmingRequest, FertilizeRequest, HarvestReply,
    HarvestRequest, RemovePlantReply, RemovePlantRequest, UnlockLandReply, UnlockLandRequest,
    UpgradeLandReply, UpgradeLandRequest, WaterLandReply, WaterLandRequest,
};
use crate::proto::generated::gamepb::shoppb::{
    BuyGoodsReply, BuyGoodsRequest, ShopInfoReply, ShopInfoRequest,
};

/// 操作限制更新回调
pub type OperationLimitsCallback = Arc<dyn Fn(AllLandsReply) + Send + Sync + 'static>;

/// 默认请求超时（20 秒，与原 TS 一致）
const DEFAULT_TIMEOUT_MS: u64 = 20_000;

/// 普通肥料 ID
pub const NORMAL_FERTILIZER_ID: i64 = 1011;
/// 有机肥料 ID
pub const ORGANIC_FERTILIZER_ID: i64 = 1012;

/// 农场 API 客户端
#[derive(Clone)]
pub struct Api {
    gateway: Arc<Gateway>,
    on_operation_limits_update: Arc<Mutex<Option<OperationLimitsCallback>>>,
}

use parking_lot::Mutex;

impl Api {
    /// 创建 API 客户端
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            on_operation_limits_update: Arc::new(Mutex::new(None)),
        }
    }

    /// 设置操作限制更新回调
    pub fn set_operation_limits_callback(&self, cb: OperationLimitsCallback) {
        *self.on_operation_limits_update.lock() = Some(cb);
    }

    /// 通用植物操作请求
    pub async fn send_plant_request(
        &self,
        method: &str,
        land_ids: Vec<i64>,
        host_gid: i64,
    ) -> Result<Vec<u8>> {
        // 简化版：用 WaterLandRequest 作为通用 land_ids + host_gid 的载体
        let body = WaterLandRequest { land_ids, host_gid }.encode_to_vec();
        self.gateway
            .request("gamepb.plantpb.PlantService", method, &body, DEFAULT_TIMEOUT_MS)
            .await
            .map_err(Error::from)
    }

    /// 获取所有土地
    pub async fn get_all_lands(&self, host_gid: i64) -> Result<AllLandsReply> {
        let body = AllLandsRequest { host_gid }.encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "AllLands",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        let reply = AllLandsReply::decode(&*resp).map_err(Error::from)?;
        // 触发操作限制更新回调
        if let Some(cb) = self.on_operation_limits_update.lock().as_ref() {
            cb(reply.clone());
        }
        Ok(reply)
    }

    /// 收获
    pub async fn harvest(&self, land_ids: Vec<i64>, host_gid: i64, all: bool) -> Result<HarvestReply> {
        let body = HarvestRequest {
            land_ids,
            host_gid,
            is_all: all,
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
        HarvestReply::decode(&*resp).map_err(Error::from)
    }

    /// 浇水
    pub async fn water_land(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<WaterLandReply> {
        let body = WaterLandRequest {
            land_ids,
            host_gid,
        }
        .encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "WaterLand",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        WaterLandReply::decode(&*resp).map_err(Error::from)
    }

    /// 锄地
    pub async fn farming(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<FarmingReply> {
        let body = FarmingRequest {
            land_ids,
            host_gid,
            field_3: 0,
            field_4: 0,
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
        FarmingReply::decode(&*resp).map_err(Error::from)
    }

    /// 施肥（单块）
    pub async fn fertilize(&self, land_id: i64, fertilizer_id: i64) -> Result<()> {
        let body = FertilizeRequest {
            land_ids: vec![land_id],
            fertilizer_id,
        }
        .encode_to_vec();
        self.gateway
            .request(
                "gamepb.plantpb.PlantService",
                "Fertilize",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        Ok(())
    }

    /// 铲除植物
    pub async fn remove_plant(&self, land_ids: Vec<i64>) -> Result<RemovePlantReply> {
        let body = RemovePlantRequest { land_ids }.encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "RemovePlant",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        RemovePlantReply::decode(&*resp).map_err(Error::from)
    }

    /// 升级土地
    pub async fn upgrade_land(&self, land_id: i64) -> Result<UpgradeLandReply> {
        let body = UpgradeLandRequest { land_id }.encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "UpgradeLand",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        UpgradeLandReply::decode(&*resp).map_err(Error::from)
    }

    /// 解锁土地
    pub async fn unlock_land(&self, land_id: i64, do_shared: bool) -> Result<UnlockLandReply> {
        let body = UnlockLandRequest {
            land_id,
            do_shared,
        }
        .encode_to_vec();
        let resp = self
            .gateway
            .request(
                "gamepb.plantpb.PlantService",
                "UnlockLand",
                &body,
                DEFAULT_TIMEOUT_MS,
            )
            .await?;
        UnlockLandReply::decode(&*resp).map_err(Error::from)
    }

    /// 获取商店信息
    pub async fn get_shop_info(&self, shop_id: i64) -> Result<ShopInfoReply> {
        let body = ShopInfoRequest { shop_id }.encode_to_vec();
        let resp = self
            .gateway
            .request("gamepb.shoppb.ShopService", "ShopInfo", &body, DEFAULT_TIMEOUT_MS)
            .await?;
        ShopInfoReply::decode(&*resp).map_err(Error::from)
    }

    /// 购买商品
    pub async fn buy_goods(&self, goods_id: i64, num: i64, price: i64) -> Result<BuyGoodsReply> {
        let body = BuyGoodsRequest {
            goods_id,
            num,
            price,
        }
        .encode_to_vec();
        let resp = self
            .gateway
            .request("gamepb.shoppb.ShopService", "BuyGoods", &body, DEFAULT_TIMEOUT_MS)
            .await?;
        BuyGoodsReply::decode(&*resp).map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fertilizer_ids() {
        assert_eq!(NORMAL_FERTILIZER_ID, 1011);
        assert_eq!(ORGANIC_FERTILIZER_ID, 1012);
    }
}
