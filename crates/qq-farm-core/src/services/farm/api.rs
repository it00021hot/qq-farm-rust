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
    HarvestRequest, OperationLimit, PlantItem, PlantReply, PlantRequest, RemovePlantReply,
    RemovePlantRequest, UnlockLandReply, UnlockLandRequest, UpgradeLandReply, UpgradeLandRequest,
    WaterLandReply, WaterLandRequest,
};
use crate::proto::generated::gamepb::shoppb::{
    BuyGoodsReply, BuyGoodsRequest, ShopInfoReply, ShopInfoRequest,
};

/// 操作限制更新回调（对齐 TS `onOperationLimitsUpdate(reply.operation_limits)`）
pub type OperationLimitsCallback = Arc<dyn Fn(Vec<OperationLimit>) + Send + Sync + 'static>;

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
        Self { gateway, on_operation_limits_update: Arc::new(Mutex::new(None)) }
    }

    /// 设置操作限制更新回调
    pub fn set_operation_limits_callback(&self, cb: OperationLimitsCallback) {
        *self.on_operation_limits_update.lock() = Some(cb);
    }

    /// 底层网关
    #[must_use]
    pub fn gateway(&self) -> &Arc<Gateway> {
        &self.gateway
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
            .request("gamepb.plantpb.PlantService", method, &body)
            .await
            .map_err(Error::from)
    }

    /// 播种（1:1 对齐原 `api.ts` 的 `plant`，RPC 方法 `Plant`）
    ///
    /// `items` 按种子分组：每组一个 `PlantItem { seed_id, land_ids }`。
    pub async fn plant(&self, items: Vec<PlantItem>) -> Result<PlantReply> {
        let body = PlantRequest { land_and_seed: Default::default(), items }.encode_to_vec();
        let resp = self.gateway.request("gamepb.plantpb.PlantService", "Plant", &body).await?;
        PlantReply::decode(&*resp).map_err(Error::from)
    }

    /// 获取所有土地
    ///
    /// 对齐 TS `getAllLands()`：`AllLandsRequest.create({})`，不传 host_gid。
    pub async fn get_all_lands(&self, _host_gid: i64) -> Result<AllLandsReply> {
        let body = AllLandsRequest { host_gid: 0 }.encode_to_vec();
        let resp = self.gateway.request("gamepb.plantpb.PlantService", "AllLands", &body).await?;
        let reply = AllLandsReply::decode(&*resp).map_err(Error::from)?;
        if !reply.operation_limits.is_empty() {
            if let Some(cb) = self.on_operation_limits_update.lock().as_ref() {
                cb(reply.operation_limits.clone());
            }
        }
        Ok(reply)
    }

    /// 收获
    pub async fn harvest(
        &self,
        land_ids: Vec<i64>,
        host_gid: i64,
        all: bool,
    ) -> Result<HarvestReply> {
        let body = HarvestRequest { land_ids, host_gid, is_all: all }.encode_to_vec();
        let resp = self.gateway.request("gamepb.plantpb.PlantService", "Harvest", &body).await?;
        HarvestReply::decode(&*resp).map_err(Error::from)
    }

    /// 浇水
    pub async fn water_land(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<WaterLandReply> {
        let body = WaterLandRequest { land_ids, host_gid }.encode_to_vec();
        let resp = self.gateway.request("gamepb.plantpb.PlantService", "WaterLand", &body).await?;
        WaterLandReply::decode(&*resp).map_err(Error::from)
    }

    /// 锄地（自己农场）
    ///
    /// 对齐 TS `farming()`：只带 `land_ids` + `host_gid`，不传 field_3/field_4。
    pub async fn farming(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<FarmingReply> {
        let body = FarmingRequest { land_ids, host_gid, ..Default::default() }.encode_to_vec();
        let resp = self.gateway.request("gamepb.plantpb.PlantService", "Farming", &body).await?;
        FarmingReply::decode(&*resp).map_err(Error::from)
    }

    /// 施肥（单块）
    pub async fn fertilize(&self, land_id: i64, fertilizer_id: i64) -> Result<()> {
        let body = FertilizeRequest { land_ids: vec![land_id], fertilizer_id }.encode_to_vec();
        self.gateway.request("gamepb.plantpb.PlantService", "Fertilize", &body).await?;
        Ok(())
    }

    /// 有机肥循环施肥（对齐 TS `fertilizeOrganicLoop`：按地块轮询直到失败）
    pub async fn fertilize_organic_loop(&self, land_ids: &[i64]) -> usize {
        let ids: Vec<i64> = land_ids.iter().copied().filter(|id| *id > 0).collect();
        if ids.is_empty() {
            return 0;
        }
        let mut success = 0usize;
        let mut idx = 0usize;
        loop {
            if self.fertilize(ids[idx], ORGANIC_FERTILIZER_ID).await.is_err() {
                break;
            }
            success += 1;
            idx = (idx + 1) % ids.len();
            let delay_ms = 1000 + (rand::random::<u64>() % 500);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        success
    }

    /// 铲除植物
    pub async fn remove_plant(&self, land_ids: Vec<i64>) -> Result<RemovePlantReply> {
        let body = RemovePlantRequest { land_ids }.encode_to_vec();
        let resp =
            self.gateway.request("gamepb.plantpb.PlantService", "RemovePlant", &body).await?;
        RemovePlantReply::decode(&*resp).map_err(Error::from)
    }

    /// 升级土地
    pub async fn upgrade_land(&self, land_id: i64) -> Result<UpgradeLandReply> {
        let body = UpgradeLandRequest { land_id }.encode_to_vec();
        let resp =
            self.gateway.request("gamepb.plantpb.PlantService", "UpgradeLand", &body).await?;
        UpgradeLandReply::decode(&*resp).map_err(Error::from)
    }

    /// 解锁土地
    pub async fn unlock_land(&self, land_id: i64, do_shared: bool) -> Result<UnlockLandReply> {
        let body = UnlockLandRequest { land_id, do_shared }.encode_to_vec();
        let resp = self.gateway.request("gamepb.plantpb.PlantService", "UnlockLand", &body).await?;
        UnlockLandReply::decode(&*resp).map_err(Error::from)
    }

    /// 获取商店信息
    pub async fn get_shop_info(&self, shop_id: i64) -> Result<ShopInfoReply> {
        let body = ShopInfoRequest { shop_id }.encode_to_vec();
        let resp = self.gateway.request("gamepb.shoppb.ShopService", "ShopInfo", &body).await?;
        ShopInfoReply::decode(&*resp).map_err(Error::from)
    }

    /// 购买商品
    pub async fn buy_goods(&self, goods_id: i64, num: i64, price: i64) -> Result<BuyGoodsReply> {
        let body = BuyGoodsRequest { goods_id, num, price }.encode_to_vec();
        let resp = self.gateway.request("gamepb.shoppb.ShopService", "BuyGoods", &body).await?;
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

    #[test]
    fn own_farm_all_lands_encodes_empty_like_ts() {
        let body = AllLandsRequest { host_gid: 0 }.encode_to_vec();
        assert!(body.is_empty());
    }

    #[test]
    fn own_farming_omits_scene_fields() {
        let with_defaults =
            FarmingRequest { land_ids: vec![1, 2], host_gid: 123, ..Default::default() }
                .encode_to_vec();
        let explicit_zeros =
            FarmingRequest { land_ids: vec![1, 2], host_gid: 123, field_3: 0, field_4: 0 }
                .encode_to_vec();
        assert_eq!(with_defaults, explicit_zeros);
        // field 4 = 2 (帮好友) 必须出现在 wire 上
        let help = FarmingRequest { land_ids: vec![1, 2], host_gid: 123, field_3: 0, field_4: 2 }
            .encode_to_vec();
        assert_ne!(with_defaults, help);
    }
}
