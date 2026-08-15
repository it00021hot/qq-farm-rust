use prost::Message;

use crate::error::{Error, Result};
use crate::proto::generated::gamepb::activitypb::{
    ActivityOperateReply, ConstellationData, OperateConstellationRequest,
};
use crate::constants::{ACTIVITY_SERVICE, CONSTELLATION_ACTIVITY_TYPE, LIGHT_CONSTELLATION_OPERATE_TYPE};
use crate::services::activity_center_state::{
    load_constellation_state, merge_constellation_states, persist_constellation_state,
    state_from_dynamic_nodes, state_record_key, state_with_no_claimable_day, ActivityStateIdentity,
    ConstellationActivityState, StateFileOptions,
};

use super::dto::{
    constellation_day_from_beijing_midnight, constellation_dto, constellation_identity,
    find_season_activity, normalize_season,
};
use super::error::{ActivityError, ActivityErrorCode};
use super::{ConstellationDto, SeasonDto};
use super::ActivityCenterService;

impl ActivityCenterService {
    // ----- 星座 -----

    /// 点亮星座（一次性操作）
    pub async fn light_constellation(&self) -> Result<serde_json::Value> {
        let _guard = self.mutation_lock.lock().await;
        let season_reply = self.query_season().await?;
        let activity = find_season_activity(&season_reply, CONSTELLATION_ACTIVITY_TYPE)
            .ok_or_else(|| ActivityError {
                code: ActivityErrorCode::ConstellationActivityMissing,
                message: "服务端未发现星座活动".to_string(),
            })?;
        let activity_id = activity.activity_id;
        let begin_time = activity.begin_time;
        let end_time = activity.end_time;
        let season = normalize_season(&season_reply).ok_or_else(|| ActivityError {
            code: ActivityErrorCode::SeasonDataEmpty,
            message: "当前赛季数据为空".to_string(),
        })?;
        let identity = constellation_identity(&season, activity_id);
        let state_key = state_record_key(&identity);
        let server_time = season.server_time;
        let current_day = constellation_day_from_beijing_midnight(begin_time, server_time);
        let activity_active = server_time > 0
            && begin_time > 0
            && server_time >= begin_time
            && (end_time <= 0 || server_time <= end_time);

        let req = OperateConstellationRequest {
            activity_id,
            operate_type: LIGHT_CONSTELLATION_OPERATE_TYPE,
            field_119: Some(
                crate::proto::generated::gamepb::activitypb::operate_constellation_request::Empty {},
            ),
        };
        let body = match self
            .gateway
            .request(ACTIVITY_SERVICE, "Operate", &req.encode_to_vec(), 10_000)
            .await
        {
            Ok(bytes) => bytes,
            Err(crate::network::error::NetworkError::Gateway { code, .. })
                if code == 1_034_038
                    && activity_active
                    && current_day.is_some_and(|d| (1..=28).contains(&d)) =>
            {
                let day = current_day.unwrap_or(0);
                let rejection = state_with_no_claimable_day(
                    &identity,
                    i64::from(day),
                    &server_time.to_string(),
                    None,
                );
                self.merge_and_persist_constellation(&identity, &state_key, rejection);
                let snapshot = self.snapshot_with_shop(None).await.ok();
                return Ok(serde_json::json!({
                    "outcome": "nothingToClaim",
                    "noClaimable": true,
                    "message": "今日星宿奖励已经领取，无需重复操作",
                    "snapshot": snapshot,
                }));
            }
            Err(e) => return Err(e.into()),
        };
        let reply = ActivityOperateReply::decode(&body[..])?;
        if reply.activity_id != activity_id {
            return Err(Error::Protocol("星座操作返回了不匹配的活动 ID".to_string()));
        }
        if reply.operate_type != LIGHT_CONSTELLATION_OPERATE_TYPE {
            return Err(Error::Protocol(format!(
                "星座操作返回了未知操作类型: {}",
                reply.operate_type
            )));
        }
        let constellation_state = reply
            .data
            .as_ref()
            .and_then(|d| d.constellation.clone())
            .ok_or_else(|| Error::Protocol("星座操作成功但回包缺少动态状态".to_string()))?;

        self.last_constellation_dynamic
            .lock()
            .insert(state_key.clone(), constellation_state.clone());
        let nodes_json = serde_json::Value::Array(
            constellation_state
                .nodes
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "node_id": n.node_id.to_string(),
                        "nodeId": n.node_id.to_string(),
                        "field_2": n.field_2,
                        "field_3": n.field_3,
                    })
                })
                .collect(),
        );
        let from_nodes = state_from_dynamic_nodes(&identity, nodes_json);
        self.merge_and_persist_constellation(&identity, &state_key, from_nodes);

        let dto = self.build_constellation_dto(&season, Some(&constellation_state));
        let snapshot = self.snapshot_with_shop(None).await.ok();
        let constellation = snapshot
            .as_ref()
            .and_then(|s| s.get("constellation").cloned())
            .or_else(|| serde_json::to_value(dto).ok());
        Ok(serde_json::json!({
            "outcome": "lighted",
            "rewards": [],
            "activity": season.constellation_activity,
            "constellation": constellation,
            "snapshot": snapshot,
        }))
    }

    fn merge_and_persist_constellation(
        &self,
        identity: &ActivityStateIdentity,
        state_key: &str,
        incoming: ConstellationActivityState,
    ) {
        let account_id = self.account_id.lock().clone();
        let memory = self
            .last_constellation_memory
            .lock()
            .get(state_key)
            .cloned();
        let file = load_constellation_state(
            identity,
            Some(account_id.as_str()).filter(|s| !s.is_empty()),
            &StateFileOptions::default(),
        );
        let merged = merge_constellation_states(
            identity,
            &[
                serde_json::to_value(&file).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(&memory).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(&incoming).unwrap_or(serde_json::Value::Null),
            ],
        );
        self.last_constellation_memory
            .lock()
            .insert(state_key.to_string(), merged.clone());
        if !account_id.is_empty() {
            let _ = persist_constellation_state(
                serde_json::to_value(&merged).unwrap_or(serde_json::Value::Null),
                identity,
                Some(&account_id),
                &StateFileOptions::default(),
            );
        }
    }

    pub(crate) fn build_constellation_dto(
        &self,
        season: &SeasonDto,
        dynamic_override: Option<&ConstellationData>,
    ) -> Option<ConstellationDto> {
        let act = season.constellation_activity.as_ref()?;
        let identity = constellation_identity(season, act.id);
        let state_key = state_record_key(&identity);
        let stored_dynamic = self.last_constellation_dynamic.lock().get(&state_key).cloned();
        let dynamic = dynamic_override.cloned().or(stored_dynamic);
        let account_id = self.account_id.lock().clone();
        let file = load_constellation_state(
            &identity,
            Some(account_id.as_str()).filter(|s| !s.is_empty()),
            &StateFileOptions::default(),
        );
        let memory = self.last_constellation_memory.lock().get(&state_key).cloned();
        let confirmed = merge_constellation_states(
            &identity,
            &[
                serde_json::to_value(&file).unwrap_or(serde_json::Value::Null),
                serde_json::to_value(&memory).unwrap_or(serde_json::Value::Null),
            ],
        );
        Some(constellation_dto(act, season.server_time, dynamic.as_ref(), &confirmed))
    }

}
