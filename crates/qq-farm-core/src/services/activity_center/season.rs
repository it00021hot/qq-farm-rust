use prost::Message;

use crate::constants::{SEASON_SERVICE, SOLAR_TERMS_SERVICE};
use crate::error::{Error, Result};
use crate::proto::generated::gamepb::seasonpb::{
    ClaimBattlePassRewardsReply, ClaimBattlePassRewardsRequest, GetSeasonInfoReply,
    GetSeasonInfoRequest,
};
use crate::proto::generated::gamepb::solartermspb::{
    ClaimSolarTermsReply, ClaimSolarTermsRequest, GetSolarTermsReply, GetSolarTermsRequest,
};

use super::dto::{
    normalize_season, normalize_solar_terms, pass_dto, positive_decimal, season_item_dto,
    solar_term_dto, solar_term_reward_dto,
};
use super::error::{ActivityError, ActivityErrorCode};
use super::{ActivityCenterService, SeasonDto, SolarTermsDto};

impl ActivityCenterService {
    // ----- 赛季 -----

    /// 拉取赛季信息
    ///
    /// # Errors
    /// - 网络 / 网关错误
    /// - protobuf 解码失败
    pub async fn query_season(&self) -> Result<GetSeasonInfoReply> {
        let body = self
            .gateway
            .request(SEASON_SERVICE, "GetSeasonInfo", &GetSeasonInfoRequest {}.encode_to_vec())
            .await?;
        let reply = GetSeasonInfoReply::decode(&body[..])?;
        *self.cached_season.lock() = Some(reply.clone());
        Ok(reply)
    }
    pub async fn get_current_season_event(&self) -> Result<SeasonDto> {
        let reply = self.query_season().await?;
        normalize_season(&reply).ok_or_else(|| {
            ActivityError {
                code: ActivityErrorCode::SeasonDataEmpty,
                message: "当前赛季数据为空".to_string(),
            }
            .into()
        })
    }

    /// 领取战斗通行证奖励
    pub async fn claim_battle_pass_rewards(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let season_reply = self.query_season().await?;
        let pass = season_reply.season_info.as_ref().and_then(|s| s.pass.as_ref()).map(pass_dto);
        let Some(pass) = pass else {
            return Err(Error::Business("服务端未发现可用游记".into()));
        };
        if !pass.nodes.iter().any(|n| n.claimable) {
            return Err(Error::Business("当前没有可领取的游记奖励".into()));
        }
        let body = self
            .gateway
            .request(
                SEASON_SERVICE,
                "ClaimBattlePassRewards",
                &ClaimBattlePassRewardsRequest {}.encode_to_vec(),
            )
            .await?;
        let reply = ClaimBattlePassRewardsReply::decode(&body[..])?;
        Ok(serde_json::json!({
            "rewards": reply.rewards.iter().map(season_item_dto).collect::<Vec<_>>(),
            "field2Codes": reply.field_2.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
            "pass": reply.pass.as_ref().map(pass_dto),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }

    /// 刷新赛季通行证（用最新数据刷新缓存）
    pub async fn refresh_season_pass(&self) -> Result<SeasonDto> {
        self.get_current_season_event().await
    }

    // ----- 节气 -----

    /// 拉取节气信息
    pub async fn query_solar_terms(&self) -> Result<GetSolarTermsReply> {
        let body = self
            .gateway
            .request(SOLAR_TERMS_SERVICE, "GetSolarTerms", &GetSolarTermsRequest {}.encode_to_vec())
            .await?;
        Ok(GetSolarTermsReply::decode(&body[..])?)
    }

    /// 拉取节气并归一化
    pub async fn get_current_solar_terms(&self) -> Result<SolarTermsDto> {
        let reply = self.query_solar_terms().await?;
        Ok(normalize_solar_terms(&reply))
    }

    /// 领取指定节气奖励
    pub async fn claim_solar_term(&self, term_id: &str) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        if !term_id.chars().all(|c| c.is_ascii_digit())
            || term_id.is_empty()
            || term_id.starts_with('0')
        {
            return Err(Error::Business("termId 必须是正十进制整数".into()));
        }
        let parsed = positive_decimal(term_id, ActivityErrorCode::InvalidSolarTermId, "termId")?;
        let solar_reply = self.query_solar_terms().await?;
        let term = solar_reply
            .terms
            .iter()
            .find(|t| t.term_id == parsed)
            .ok_or_else(|| Error::Business("服务端未发现指定节令".into()))?;
        if term.status != 2 {
            return Err(Error::Business("指定节令当前不可领取".into()));
        }
        let req = ClaimSolarTermsRequest { term_id: parsed };
        let body = self
            .gateway
            .request(SOLAR_TERMS_SERVICE, "ClaimSolarTerms", &req.encode_to_vec())
            .await?;
        let reply = ClaimSolarTermsReply::decode(&body[..])?;
        Ok(serde_json::json!({
            "rewards": reply.rewards.iter().map(solar_term_reward_dto).collect::<Vec<_>>(),
            "term": reply.term.as_ref().map(solar_term_dto),
            "snapshot": self.snapshot_with_shop(None).await.ok(),
        }))
    }
}
