//! 每日分享礼包。
//!
//! 1:1 翻译原 `core/src/services/share.ts`（137 行）。
//!
//! ## 协议
//!
//! - `gamepb.sharepb.ShareService.CheckCanShare` — 查询入口
//! - `gamepb.sharepb.ShareService.GetInviteInfo` — 邀请信息
//! - `gamepb.sharepb.ShareService.ReportShare` — 报告分享
//! - `gamepb.sharepb.ShareService.ClaimShareReward` — 领取分享奖励
//!
//! ## 状态
//!
//! - `checked_date_key` / `claimed_date_key`：跨天重置
//! - `last_check_at` / `last_claim_at`：时间戳
//! - `can_share` / `check_status`：每日状态

use std::sync::Arc;

use parking_lot::Mutex;

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::sharepb::{
    CheckCanShareReply, CheckCanShareRequest, ClaimShareRewardReply, ClaimShareRewardRequest,
    GetInviteInfoReply, GetInviteInfoRequest, ReportShareReply, ReportShareRequest,
};

const DAILY_KEY: &str = "daily_share";
const CHECK_COOLDOWN_MS: i64 = 10 * 60 * 1000;

/// 分享检查状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShareCheckStatus {
    #[default]
    Unchecked,
    EntryAvailable,
    EntryUnavailable,
    AlreadyClaimed,
    CheckFailed,
}

impl ShareCheckStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unchecked => "unchecked",
            Self::EntryAvailable => "entry_available",
            Self::EntryUnavailable => "entry_unavailable",
            Self::AlreadyClaimed => "already_claimed",
            Self::CheckFailed => "check_failed",
        }
    }
}

/// 分享服务
pub struct ShareService {
    gateway: Arc<Gateway>,
    checked_date_key: Mutex<String>,
    claimed_date_key: Mutex<String>,
    last_check_at: Mutex<i64>,
    last_claim_at: Mutex<i64>,
    check_status: Mutex<ShareCheckStatus>,
    can_share: Mutex<Option<bool>>,
}

impl ShareService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            checked_date_key: Mutex::new(String::new()),
            claimed_date_key: Mutex::new(String::new()),
            last_check_at: Mutex::new(0),
            last_claim_at: Mutex::new(0),
            check_status: Mutex::new(ShareCheckStatus::Unchecked),
            can_share: Mutex::new(None),
        }
    }

    /// CheckCanShare RPC
    pub async fn check_can_share(&self) -> Result<CheckCanShareReply> {
        let req = CheckCanShareRequest {};
        let body = self
            .gateway
            .request(
                "gamepb.sharepb.ShareService",
                "CheckCanShare",
                &prost::Message::encode_to_vec(&req),
                10_000,
            )
            .await?;
        Ok(CheckCanShareReply::decode(&body)?)
    }

    /// GetInviteInfo RPC
    pub async fn get_invite_info(&self) -> Result<GetInviteInfoReply> {
        let req = GetInviteInfoRequest {};
        let body = self
            .gateway
            .request(
                "gamepb.sharepb.ShareService",
                "GetInviteInfo",
                &prost::Message::encode_to_vec(&req),
                10_000,
            )
            .await?;
        Ok(GetInviteInfoReply::decode(&body)?)
    }

    /// ReportShare RPC（每日礼包：field_1=1 / field_4=42）
    pub async fn report_share(&self) -> Result<ReportShareReply> {
        let req = ReportShareRequest { field_1: 1, field_4: 42 };
        let body = self
            .gateway
            .request(
                "gamepb.sharepb.ShareService",
                "ReportShare",
                &prost::Message::encode_to_vec(&req),
                10_000,
            )
            .await?;
        Ok(ReportShareReply::decode(&body)?)
    }

    /// 对齐原 `reportActivityShare`：只发送不等待回包（青梅酿 field_1=11 / field_4=215）。
    pub async fn report_activity_share(&self, source: i32, scene: i32) -> Result<()> {
        let req = ReportShareRequest { field_1: source, field_4: scene };
        self.gateway
            .send_no_reply(
                "gamepb.sharepb.ShareService",
                "ReportShare",
                &prost::Message::encode_to_vec(&req),
            )
            .await?;
        Ok(())
    }

    /// ClaimShareReward RPC
    pub async fn claim_share_reward(&self) -> Result<ClaimShareRewardReply> {
        let req = ClaimShareRewardRequest { field_1: true };
        let body = self
            .gateway
            .request(
                "gamepb.sharepb.ShareService",
                "ClaimShareReward",
                &prost::Message::encode_to_vec(&req),
                10_000,
            )
            .await?;
        Ok(ClaimShareRewardReply::decode(&body)?)
    }

    /// 检查并领取每日分享（5min 冷却）
    pub async fn check_daily_share_status(&self, force: bool) -> bool {
        let today = get_date_key();
        if *self.claimed_date_key.lock() == today {
            return false;
        }
        if !force {
            let now = crate::utils::time::now_ms();
            if (now - *self.last_check_at.lock()) < CHECK_COOLDOWN_MS {
                return false;
            }
        }
        *self.last_check_at.lock() = crate::utils::time::now_ms();

        match self.check_can_share().await {
            Ok(reply) => {
                let can = reply.can_share;
                *self.can_share.lock() = Some(can);
                *self.check_status.lock() = if can {
                    ShareCheckStatus::EntryAvailable
                } else {
                    ShareCheckStatus::EntryUnavailable
                };
                *self.checked_date_key.lock() = today.clone();

                if !can {
                    tracing::info!("[分享] 分享入口暂不可用");
                    return true;
                }

                if let Err(e) = self.report_share().await {
                    tracing::warn!("[分享] ReportShare 失败: {e}");
                    return false;
                }

                match self.claim_share_reward().await {
                    Ok(_claim_reply) => {
                        *self.claimed_date_key.lock() = today;
                        *self.last_claim_at.lock() = crate::utils::time::now_ms();
                        tracing::info!("[分享] 分享礼包领取成功");
                        true
                    }
                    Err(e) => {
                        if is_already_claimed_error(&e.to_string()) {
                            *self.claimed_date_key.lock() = today.clone();
                            *self.checked_date_key.lock() = today;
                            *self.last_claim_at.lock() = crate::utils::time::now_ms();
                            *self.check_status.lock() = ShareCheckStatus::AlreadyClaimed;
                            tracing::info!("[分享] 分享礼包今日已领取");
                            true
                        } else {
                            *self.can_share.lock() = None;
                            *self.check_status.lock() = ShareCheckStatus::CheckFailed;
                            tracing::warn!("[分享] 状态检查失败: {e}");
                            false
                        }
                    }
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if is_already_claimed_error(&msg) {
                    *self.claimed_date_key.lock() = today.clone();
                    *self.checked_date_key.lock() = today;
                    *self.last_claim_at.lock() = crate::utils::time::now_ms();
                    *self.check_status.lock() = ShareCheckStatus::AlreadyClaimed;
                    tracing::info!("[分享] 分享礼包今日已领取");
                    true
                } else {
                    *self.can_share.lock() = None;
                    *self.check_status.lock() = ShareCheckStatus::CheckFailed;
                    tracing::warn!("[分享] 状态检查失败: {msg}");
                    false
                }
            }
        }
    }

    /// 获取每日状态
    #[must_use]
    pub fn get_daily_state(&self) -> serde_json::Value {
        let today = get_date_key();
        serde_json::json!({
            "key": DAILY_KEY,
            "mode": "auto_claim",
            "checkedToday": *self.checked_date_key.lock() == today,
            "checkStatus": self.check_status.lock().as_str(),
            "canShare": *self.can_share.lock(),
            "doneToday": *self.claimed_date_key.lock() == today,
            "lastCheckAt": *self.last_check_at.lock(),
            "lastClaimAt": *self.last_claim_at.lock(),
        })
    }
}

// =====================================================================
// 辅助
// =====================================================================

fn get_date_key() -> String {
    use chrono::Datelike;
    use chrono::Local;
    let now = Local::now();
    format!("{}-{:02}-{:02}", now.year(), now.month(), now.day())
}

/// 检测 "1009001 错误"（已领取）
#[must_use]
pub fn is_already_claimed_error(msg: &str) -> bool {
    msg.contains("code=1009001") || msg.contains("1009001")
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
    fn share_check_status_as_str() {
        assert_eq!(ShareCheckStatus::Unchecked.as_str(), "unchecked");
        assert_eq!(ShareCheckStatus::EntryAvailable.as_str(), "entry_available");
        assert_eq!(ShareCheckStatus::EntryUnavailable.as_str(), "entry_unavailable");
        assert_eq!(ShareCheckStatus::AlreadyClaimed.as_str(), "already_claimed");
        assert_eq!(ShareCheckStatus::CheckFailed.as_str(), "check_failed");
    }

    #[test]
    fn is_already_claimed_error_detect() {
        assert!(is_already_claimed_error("code=1009001"));
        assert!(is_already_claimed_error("1009001"));
        assert!(!is_already_claimed_error("code=1009002"));
        assert!(!is_already_claimed_error(""));
    }

    #[test]
    fn date_key_format() {
        let k = get_date_key();
        assert_eq!(k.len(), 10);
    }

    #[test]
    fn share_service_construction() {
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
        let _ = ShareService::new(Arc::new(Gateway::new(cfg, Arc::new(NoopEncryptor))));
    }
}
