//! 好友互动（访客记录）。
//!
//! 1:1 翻译原 `core/src/services/interact.ts`（146 行）。
//!
//! ## 协议
//!
//! - `gamepb.interactpb.{InteractService|VisitorService}.InteractRecords` — 拉取访客记录
//! - `gamepb.interactpb.InteractService.GetInteractInfo` — 互动信息
//! - `gamepb.interactpb.InteractService.GetInteractSummary` — 互动汇总
//!
//! ## 特性
//!
//! - 4 个 RPC 候选名（兼容不同 proto 版本）
//! - 记录归一化（land_id / crop_id / action_type → 友好文本）

use std::sync::Arc;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::interactpb::{
    GetInteractInfoReply, GetInteractInfoRequest, GetInteractSummaryReply,
    GetInteractSummaryRequest, InteractRecordsReply, InteractRecordsRequest,
};
use crate::utils::time::to_time_secs;

const RPC_CANDIDATES: &[(&str, &str)] = &[
    ("gamepb.interactpb.InteractService", "InteractRecords"),
    ("gamepb.interactpb.InteractService", "GetInteractRecords"),
    ("gamepb.interactpb.VisitorService", "InteractRecords"),
    ("gamepb.interactpb.VisitorService", "GetInteractRecords"),
];

/// 互动类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    /// 偷取作物
    Steal = 1,
    /// 帮忙
    Help = 2,
    /// 捣乱
    Bad = 3,
}

impl ActionType {
    #[must_use]
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Steal),
            2 => Some(Self::Help),
            3 => Some(Self::Bad),
            _ => None,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Steal => "偷取作物",
            Self::Help => "帮忙",
            Self::Bad => "捣乱",
        }
    }
}

/// 归一化后的互动记录（面板 JSON 与 bot `/api/interact-records` 同为 camelCase）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedRecord {
    pub key: String,
    pub server_time_sec: i64,
    pub server_time_ms: i64,
    pub action_type: i32,
    pub action_label: String,
    pub visitor_gid: i64,
    pub nick: String,
    pub avatar_url: String,
    pub crop_id: i64,
    pub crop_name: String,
    pub crop_count: i64,
    pub times: i64,
    pub from_type: i64,
    pub level: i64,
    pub land_id: i64,
    pub flag1: i64,
    pub flag2: i64,
    pub action_detail: String,
}

/// 互动服务
pub struct InteractService {
    gateway: Arc<Gateway>,
}

impl InteractService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway }
    }

    /// 拉取互动记录（按时间倒序）
    pub async fn get_interact_records(&self) -> Result<Vec<NormalizedRecord>> {
        let reply = self.fetch_reply().await?;
        let records = reply.records;
        let mut out: Vec<NormalizedRecord> =
            records.iter().enumerate().map(|(i, r)| normalize_record(r, i)).collect();
        out.sort_by(|a, b| {
            b.server_time_sec
                .cmp(&a.server_time_sec)
                .then_with(|| b.visitor_gid.cmp(&a.visitor_gid))
                .then_with(|| b.action_type.cmp(&a.action_type))
        });
        Ok(out)
    }

    /// GetInteractInfo RPC
    pub async fn get_interact_info(&self) -> Result<GetInteractInfoReply> {
        let req = GetInteractInfoRequest {};
        let body = self
            .gateway
            .request(
                "gamepb.interactpb.InteractService",
                "GetInteractInfo",
                &prost::Message::encode_to_vec(&req),
            )
            .await?;
        Ok(GetInteractInfoReply::decode(&body)?)
    }

    /// GetInteractSummary RPC
    pub async fn get_interact_summary(&self) -> Result<GetInteractSummaryReply> {
        let req = GetInteractSummaryRequest {};
        let body = self
            .gateway
            .request(
                "gamepb.interactpb.InteractService",
                "GetInteractSummary",
                &prost::Message::encode_to_vec(&req),
            )
            .await?;
        Ok(GetInteractSummaryReply::decode(&body)?)
    }

    async fn fetch_reply(&self) -> Result<InteractRecordsReply> {
        let req = InteractRecordsRequest {};
        let req_body = prost::Message::encode_to_vec(&req);

        let mut errors = Vec::new();
        for &(service, method) in RPC_CANDIDATES {
            match self.gateway.request(service, method, &req_body).await {
                Ok(body) => return Ok(InteractRecordsReply::decode(&body)?),
                Err(e) => {
                    let retry = matches!(
                        e,
                        crate::network::error::NetworkError::Gateway { .. }
                            | crate::network::error::NetworkError::Frame(_)
                    );
                    errors.push(format!("{service}.{method}: {e}"));
                    if !retry {
                        break;
                    }
                }
            }
        }
        tracing::warn!("[好友] 访客记录接口调用失败: {}", errors.join(" | "));
        Err(crate::error::Error::Protocol(
            "访客记录接口调用失败，请确认服务名和方法名是否与当前版本一致".into(),
        ))
    }
}

// =====================================================================
// 辅助
// =====================================================================

fn normalize_record(
    record: &crate::proto::generated::gamepb::interactpb::InteractRecord,
    index: usize,
) -> NormalizedRecord {
    let (land_id, flag1, flag2) = match &record.extra {
        Some(e) => (e.land_id as i64, e.flag1 as i64, e.flag2 as i64),
        None => (0, 0, 0),
    };
    let action_type = record.action_type;
    let visitor_gid = record.visitor_gid;
    let crop_id = record.crop_id as i64;
    let crop_count = record.crop_count as i64;
    let times = record.times as i64;
    let level = record.level as i64;
    let from_type = record.from_type as i64;
    let server_time_sec = to_time_secs(record.server_time);
    let crop_name = resolve_crop_name(crop_id);
    let nick =
        if record.nick.is_empty() { format!("GID:{visitor_gid}") } else { record.nick.clone() };
    let avatar_url = record.avatar_url.clone();
    let action_label = ActionType::from_i32(action_type)
        .map(|a| a.label().to_string())
        .unwrap_or_else(|| "互动".to_string());
    let key = format!("{server_time_sec}-{visitor_gid}-{action_type}-{index}");
    let action_detail = build_action_detail(action_type, crop_count, times, land_id, &crop_name);
    NormalizedRecord {
        key,
        server_time_sec,
        server_time_ms: if server_time_sec > 0 { server_time_sec * 1000 } else { 0 },
        action_type,
        action_label,
        visitor_gid,
        nick,
        avatar_url,
        crop_id,
        crop_name,
        crop_count,
        times,
        from_type,
        level,
        land_id,
        flag1,
        flag2,
        action_detail,
    }
}

fn build_action_detail(
    action_type: i32,
    crop_count: i64,
    times: i64,
    land_id: i64,
    crop_name: &str,
) -> String {
    let mut parts = Vec::new();
    if action_type == 1 {
        if !crop_name.is_empty() && crop_count > 0 {
            parts.push(format!("偷取 {crop_name} × {crop_count}"));
        } else if !crop_name.is_empty() {
            parts.push(format!("偷取 {crop_name}"));
        } else if crop_count > 0 {
            parts.push(format!("偷取作物 × {crop_count}"));
        } else {
            parts.push("偷取作物".to_string());
        }
    } else if action_type == 2 {
        if times > 1 {
            parts.push(format!("帮忙 {times} 次"));
        } else {
            parts.push("帮忙".to_string());
        }
    } else if action_type == 3 {
        if times > 1 {
            parts.push(format!("捣乱 {times} 次"));
        } else {
            parts.push("捣乱".to_string());
        }
    } else if times > 1 {
        parts.push(format!("互动 {times} 次"));
    } else {
        parts.push("互动".to_string());
    }
    if land_id > 0 {
        parts.push(format!("地块 {land_id}"));
    }
    parts.join(" · ")
}

fn resolve_crop_name(crop_id: i64) -> String {
    use crate::config::game_config::global as global_game_config;
    let id = crop_id.max(0);
    if id <= 0 {
        return String::new();
    }
    let gc = global_game_config();
    if gc.get_plant_by_id(id).is_some() {
        return gc.get_plant_name(id);
    }
    if gc.get_plant_by_fruit_id(id).is_some() {
        return gc.get_fruit_name(id);
    }
    String::new()
}

trait DecodeExt: Sized {
    fn decode(_: &[u8]) -> Result<Self>;
}

impl<T: prost::Message + Default> DecodeExt for T {
    fn decode(bytes: &[u8]) -> Result<Self> {
        T::decode(bytes).map_err(crate::error::Error::from)
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_type_label() {
        assert_eq!(ActionType::Steal.label(), "偷取作物");
        assert_eq!(ActionType::Help.label(), "帮忙");
        assert_eq!(ActionType::Bad.label(), "捣乱");
    }

    #[test]
    fn action_type_from_i32() {
        assert_eq!(ActionType::from_i32(1), Some(ActionType::Steal));
        assert_eq!(ActionType::from_i32(2), Some(ActionType::Help));
        assert_eq!(ActionType::from_i32(3), Some(ActionType::Bad));
        assert_eq!(ActionType::from_i32(4), None);
    }

    #[test]
    fn build_action_detail_steal_with_count() {
        let s = build_action_detail(1, 5, 1, 10, "白萝卜");
        assert!(s.contains("偷取"));
        assert!(s.contains("白萝卜"));
        assert!(s.contains("5"));
        assert!(s.contains("地块 10"));
    }

    #[test]
    fn build_action_detail_help_multi() {
        let s = build_action_detail(2, 0, 3, 0, "");
        assert!(s.contains("帮忙"));
        assert!(s.contains("3"));
    }

    #[test]
    fn build_action_detail_bad_single() {
        let s = build_action_detail(3, 0, 1, 0, "");
        assert!(s.contains("捣乱"));
    }

    #[test]
    fn build_action_detail_other() {
        let s = build_action_detail(99, 0, 5, 0, "");
        assert!(s.contains("互动"));
        assert!(s.contains("5"));
    }

    #[test]
    fn interact_service_construction() {
        use crate::network::encryptor::NoopEncryptor;
        use crate::network::gateway::{Gateway, GatewayConfig};
        let cfg = GatewayConfig {
            server_url: "ws://127.0.0.1:0".into(),
            platform: "test".into(),
            os: "linux".into(),
            client_version: "0.1".into(),
            auth_code: "test".into(),
            headers: Default::default(),
        };
        let _ = InteractService::new(Arc::new(Gateway::new(cfg, Arc::new(NoopEncryptor))));
    }

    #[test]
    fn normalized_record_serializes_camel_case() {
        let record = NormalizedRecord {
            key: "1-2-1-0".into(),
            server_time_sec: 1,
            server_time_ms: 1000,
            action_type: 1,
            action_label: "偷取作物".into(),
            visitor_gid: 2,
            nick: "alice".into(),
            avatar_url: "https://example.com/a.png".into(),
            crop_id: 10,
            crop_name: "白萝卜".into(),
            crop_count: 3,
            times: 1,
            from_type: 0,
            level: 12,
            land_id: 401003,
            flag1: 0,
            flag2: 0,
            action_detail: "偷取 白萝卜 × 3 · 地块 401003".into(),
        };
        let value = serde_json::to_value(&record).expect("serialize");
        assert_eq!(value["actionType"], 1);
        assert_eq!(value["actionLabel"], "偷取作物");
        assert_eq!(value["visitorGid"], 2);
        assert_eq!(value["avatarUrl"], "https://example.com/a.png");
        assert_eq!(value["serverTimeMs"], 1000);
        assert_eq!(value["actionDetail"], "偷取 白萝卜 × 3 · 地块 401003");
        assert!(value.get("action_type").is_none());
        assert!(value.get("avatar_url").is_none());
    }
}
