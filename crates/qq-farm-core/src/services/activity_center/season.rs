use prost::Message;

use crate::constants::{SEASON_SERVICE, SOLAR_TERMS_SERVICE};
use crate::error::{Error, Result};
use crate::proto::generated::gamepb::seasonpb::{
    ClaimBattlePassRewardsReply, ClaimBattlePassRewardsRequest, GetSeasonInfoReply,
    GetSeasonInfoRequest, SeasonInfo, SeasonPass,
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

    /// 处理 BattlePassChangeNotify 推送：把推送里的通行证合并进赛季缓存。
    ///
    /// 对齐 go `activitycenter.ApplySeasonPassNotify`（snapshot.go）：推送常省略
    /// title / activityId / nodes，合并时保留缓存里的旧值；领取标记（claimed /
    /// claimable 等）由 `pass_dto` 按 currentLevel / claimedThroughLevel 实时推导，
    /// 无需像 go 那样显式重算。缓存缺失时按最小 Reply 落一份，等下次
    /// GetSeasonInfo 拉全量覆盖。
    pub fn apply_battle_pass_notify(&self, pass: &SeasonPass) {
        let mut cached = self.cached_season.lock();
        let reply = cached.get_or_insert_with(GetSeasonInfoReply::default);
        let info = reply.season_info.get_or_insert_with(SeasonInfo::default);
        let merged = match info.pass.as_ref() {
            None => pass.clone(),
            Some(prev) => {
                let mut merged = pass.clone();
                // proto3 里 0 / 空 = 未携带，沿用缓存旧值（对齐 go 的
                // title / activityId / nodes 三段保留逻辑）
                if merged.title.is_empty() {
                    merged.title = prev.title.clone();
                }
                if merged.activity_id == 0 {
                    merged.activity_id = prev.activity_id;
                }
                if merged.nodes.is_empty() {
                    merged.nodes = prev.nodes.clone();
                }
                merged
            }
        };
        // 日志字段对齐 go 的 "battle pass changed"（title 空时按 go 习惯显示"游记"）
        let mut title = String::from_utf8_lossy(&merged.title).trim().to_string();
        if title.is_empty() {
            title = "游记".to_string();
        }
        tracing::info!(
            activity_id = merged.activity_id,
            title = %title,
            level = merged.current_level,
            progress = merged.current_progress,
            progress_max = merged.progress_target,
            "通行证变化推送，赛季缓存已刷新"
        );
        info.pass = Some(merged);
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
