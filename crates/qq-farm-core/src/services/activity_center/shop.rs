use prost::Message;

use crate::error::Result;
use crate::services::warehouse::WarehouseService;
use crate::proto::generated::gamepb::activitypb::{
    ActivityOperateReply, ExchangeShopRequest,
};
use crate::proto::generated::gamepb::seasonpb::GetSeasonInfoReply;
use crate::constants::{ACTIVITY_SERVICE, EXCHANGE_SHOP_OPERATE_TYPE, QUERY_SHOP_OPERATE_TYPE, SHOP_ACTIVITY_TYPE};

use super::dto::{
    extract_goods, find_season_activity, item_from_id, normalize_shop_from_reply,
    positive_decimal, read_bag_balances,
};
use super::error::{ActivityError, ActivityErrorCode};
use super::{ExchangeResultDto, StarSandShopDto};
use super::ActivityCenterService;

impl ActivityCenterService {
    // ----- 活动商店 -----

    /// 拉取当前赛季的活动商店
    pub async fn get_current_star_sand_shop(
        &self,
        warehouse: Option<&WarehouseService>,
    ) -> Result<StarSandShopDto> {
        let season_reply = self.query_season().await?;
        self.shop_from_season_reply(&season_reply, warehouse).await
    }

    pub(crate) async fn shop_from_season_reply(
        &self,
        season_reply: &GetSeasonInfoReply,
        warehouse: Option<&WarehouseService>,
    ) -> Result<StarSandShopDto> {
        let shop_activity = find_season_activity(season_reply, SHOP_ACTIVITY_TYPE)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopUnavailable,
                message: "当前赛季未发现活动商店".to_string(),
            })?;
        let reply = self
            .query_shop_catalog(shop_activity.activity_id)
            .await?;
        let goods = extract_goods(&reply);
        let currency_ids: Vec<i64> = goods
            .iter()
            .map(|g| g.cost.as_ref().map(|c| c.id).unwrap_or(0))
            .filter(|id| *id > 0)
            .collect();
        let balances = match warehouse {
            Some(wh) => read_bag_balances(wh, &currency_ids).await,
            None => None,
        };
        Ok(normalize_shop_from_reply(
            season_reply,
            shop_activity,
            &reply,
            balances.as_ref(),
        ))
    }

    /// 查询商店目录（带响应校验）
    async fn query_shop_catalog(&self, activity_id: i64) -> Result<ActivityOperateReply> {
        let reply = self.query_activity(activity_id, QUERY_SHOP_OPERATE_TYPE).await?;
        if reply.activity_id != activity_id {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "活动商店查询返回了不匹配的活动 ID".to_string(),
            }
            .into());
        }
        if reply.operate_type != QUERY_SHOP_OPERATE_TYPE {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: format!(
                    "活动商店查询返回了未知操作类型: {}",
                    reply.operate_type
                ),
            }
            .into());
        }
        let data = reply.data.as_ref().ok_or_else(|| ActivityError {
            code: ActivityErrorCode::ShopResponseInvalid,
            message: "活动商店查询回包缺少商品目录".to_string(),
        })?;
        if data.catalog.is_none() {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "活动商店查询回包缺少商品目录".to_string(),
            }
            .into());
        }
        Ok(reply)
    }

    /// 兑换星砂商店商品
    pub async fn exchange_star_sand_goods(
        &self,
        warehouse: &WarehouseService,
        goods_id_input: &str,
        count_input: &str,
    ) -> Result<ExchangeResultDto> {
        let goods_id = positive_decimal(
            goods_id_input,
            ActivityErrorCode::InvalidShopGoodsId,
            "goodsId",
        )?;
        let count = positive_decimal(
            count_input,
            ActivityErrorCode::InvalidExchangeCount,
            "count",
        )?;
        if count <= 0 {
            return Err(ActivityError {
                code: ActivityErrorCode::InvalidExchangeCount,
                message: "count must be a positive integer".to_string(),
            }
            .into());
        }

        let _guard = self.mutation_lock.lock().await;

        let season_reply = self.query_season().await?;
        let shop_activity = find_season_activity(&season_reply, SHOP_ACTIVITY_TYPE)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopUnavailable,
                message: "当前赛季未发现活动商店".to_string(),
            })?;

        let catalog_reply = self
            .query_shop_catalog(shop_activity.activity_id)
            .await?;
        let raw_goods_list = extract_goods(&catalog_reply);
        let raw_goods = raw_goods_list
            .iter()
            .find(|g| g.id == goods_id)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopGoodsNotFound,
                message: "活动商店中未找到指定商品".to_string(),
            })?
            .clone();

        let currency_id = raw_goods.cost.as_ref().map(|c| c.id).unwrap_or(0);
        let unit_cost = raw_goods.cost.as_ref().map(|c| c.count).unwrap_or(0);
        if currency_id <= 0 || unit_cost <= 0 {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "商品兑换成本无效，请刷新商店后重试".to_string(),
            }
            .into());
        }

        let balances = read_bag_balances(warehouse, &[currency_id])
            .await
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopBalanceUnavailable,
                message: "无法确认当前星砂余额，请稍后重试".to_string(),
            })?;
        let shop_before = normalize_shop_from_reply(
            &season_reply,
            shop_activity,
            &catalog_reply,
            Some(&balances),
        );
        let normalized_goods = shop_before
            .goods
            .iter()
            .find(|g| g.id == goods_id)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ShopGoodsNotFound,
                message: "活动商店中未找到指定商品".to_string(),
            })?;
        if !normalized_goods.exchangeable || normalized_goods.sold_out {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopGoodsUnavailable,
                message: "该商品当前不可兑换，请刷新商店后重试".to_string(),
            }
            .into());
        }
        let cost_name = if normalized_goods.cost.name.is_empty() {
            "星砂".to_string()
        } else {
            normalized_goods.cost.name.clone()
        };
        let balance = *balances.get(&currency_id).unwrap_or(&0);
        let total_cost = unit_cost * count;
        if balance < total_cost {
            return Err(ActivityError {
                code: ActivityErrorCode::InsufficientStarSand,
                message: "星砂余额不足，无法完成本次兑换".to_string(),
            }
            .into());
        }

        // 发送兑换请求
        let req = ExchangeShopRequest {
            activity_id: shop_activity.activity_id,
            operate_type: EXCHANGE_SHOP_OPERATE_TYPE,
            exchange_shop_operate: Some(
                crate::proto::generated::gamepb::activitypb::ExchangeShopOperateParams {
                    goods_id,
                    count,
                },
            ),
        };
        let body = self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec(), 10_000)
            .await?;
        let reply = ActivityOperateReply::decode(&body[..])?;
        if reply.activity_id != shop_activity.activity_id {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "活动商店兑换返回了不匹配的活动 ID".to_string(),
            }
            .into());
        }
        if reply.operate_type != EXCHANGE_SHOP_OPERATE_TYPE {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: format!(
                    "活动商店兑换返回了未知操作类型: {}",
                    reply.operate_type
                ),
            }
            .into());
        }
        if reply
            .data
            .as_ref()
            .and_then(|d| d.catalog.as_ref())
            .is_none()
        {
            return Err(ActivityError {
                code: ActivityErrorCode::ShopResponseInvalid,
                message: "活动商店兑换回包缺少最新商品目录".to_string(),
            }
            .into());
        }

        let unit_item_count = raw_goods.item.as_ref().map(|i| i.count).unwrap_or(0);
        let total_item_count = if unit_item_count > 0 {
            unit_item_count * count
        } else {
            0
        };
        let received = match raw_goods.item.as_ref() {
            Some(item) if item.id > 0 && total_item_count > 0 => {
                vec![item_from_id(item.id, total_item_count)]
            }
            _ => vec![],
        };

        // 刷新最新目录
        let latest_currency_ids: Vec<i64> = extract_goods(&reply)
            .iter()
            .filter_map(|g| g.cost.as_ref().map(|c| c.id))
            .filter(|id| *id > 0)
            .collect();
        let latest_balances = read_bag_balances(warehouse, &latest_currency_ids).await;
        let shop = normalize_shop_from_reply(
            &season_reply,
            shop_activity,
            &reply,
            latest_balances.as_ref(),
        );
        let snapshot = self.snapshot_with_shop(Some(shop.clone())).await.ok();
        let message = format!("兑换成功，共消耗 {total_cost} {cost_name}");

        Ok(ExchangeResultDto {
            purchase_count: count.to_string(),
            total_item_count: total_item_count.to_string(),
            total_cost: total_cost.to_string(),
            rewards: received.clone(),
            received_items: received,
            shop,
            message,
            snapshot,
        })
    }

}
