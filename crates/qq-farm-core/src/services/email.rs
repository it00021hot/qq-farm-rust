//! 邮箱系统 — 自动领取邮箱奖励。
//!
//! 1:1 翻译原 `core/src/services/email.ts`（201 行）。
//!
//! ## 协议
//!
//! - `gamepb.emailpb.EmailService.GetEmailList(boxType)` — 拉取邮箱列表
//! - `gamepb.emailpb.EmailService.ClaimEmail(boxType, emailId)` — 单封领取
//! - `gamepb.emailpb.EmailService.BatchClaimEmail(boxType, emailId)` — 批量领取
//! - `gamepb.emailpb.EmailService.BatchDeleteEmail(boxType, emailIds)` — 批量删除
//!
//! ## 状态
//!
//! - 每日自动领取（5min 冷却）
//! - done_date_key 跨天重置
//! - 合并 box1/box2 同 ID 邮件（优先保留"有奖励未领"版本）

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::emailpb::{
    BatchClaimEmailReply, BatchClaimEmailRequest, BatchDeleteEmailReply, BatchDeleteEmailRequest,
    ClaimEmailReply, ClaimEmailRequest, EmailItem, GetEmailListReply, GetEmailListRequest,
};

const DAILY_KEY: &str = "email_rewards";
const CHECK_COOLDOWN_MS: i64 = 5 * 60 * 1000;

/// 邮箱服务
pub struct EmailService {
    gateway: Arc<Gateway>,
    done_date_key: Mutex<String>,
    last_check_at: Mutex<i64>,
}

impl EmailService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self { gateway, done_date_key: Mutex::new(String::new()), last_check_at: Mutex::new(0) }
    }

    /// 拉取邮箱列表
    pub async fn get_email_list(&self, box_type: i32) -> Result<GetEmailListReply> {
        let req = GetEmailListRequest { box_type };
        let body = self
            .gateway
            .request(
                "gamepb.emailpb.EmailService",
                "GetEmailList",
                &prost::Message::encode_to_vec(&req),
            )
            .await?;
        Ok(GetEmailListReply::decode(&body)?)
    }

    /// 单封领取
    pub async fn claim_email(&self, box_type: i32, email_id: &str) -> Result<ClaimEmailReply> {
        let req = ClaimEmailRequest { box_type, email_id: email_id.to_string() };
        let body = self
            .gateway
            .request(
                "gamepb.emailpb.EmailService",
                "ClaimEmail",
                &prost::Message::encode_to_vec(&req),
            )
            .await?;
        Ok(ClaimEmailReply::decode(&body)?)
    }

    /// 批量领取
    pub async fn batch_claim_email(
        &self,
        box_type: i32,
        email_id: &str,
    ) -> Result<BatchClaimEmailReply> {
        let req = BatchClaimEmailRequest { box_type, email_ids: vec![email_id.to_string()] };
        let body = self
            .gateway
            .request(
                "gamepb.emailpb.EmailService",
                "BatchClaimEmail",
                &prost::Message::encode_to_vec(&req),
            )
            .await?;
        Ok(BatchClaimEmailReply::decode(&body)?)
    }

    /// 批量删除
    pub async fn batch_delete_email(
        &self,
        box_type: i32,
        email_ids: Vec<String>,
    ) -> Result<BatchDeleteEmailReply> {
        let req = BatchDeleteEmailRequest { box_type, email_ids };
        let body = self
            .gateway
            .request(
                "gamepb.emailpb.EmailService",
                "BatchDeleteEmail",
                &prost::Message::encode_to_vec(&req),
            )
            .await?;
        Ok(BatchDeleteEmailReply::decode(&body)?)
    }

    /// 检查并领取每日邮箱奖励
    pub async fn check_and_claim_emails(&self, force: bool) -> EmailClaimResult {
        let today = get_date_key();
        {
            if !force && *self.done_date_key.lock() == today {
                return EmailClaimResult { claimed: 0, reward_items: 0 };
            }
        }
        let now = crate::utils::time::now_ms();
        {
            if !force && (now - *self.last_check_at.lock()) < CHECK_COOLDOWN_MS {
                return EmailClaimResult { claimed: 0, reward_items: 0 };
            }
        }
        *self.last_check_at.lock() = now;

        // 拉 box1 + box2
        let box1 = self.get_email_list(1).await.unwrap_or_default();
        let box2 = self.get_email_list(2).await.unwrap_or_default();

        // 合并去重（优先保留"有奖励未领"）
        let mut merged: HashMap<String, MergedEmail> = HashMap::new();
        for x in box1.emails.iter() {
            if x.id.is_empty() {
                continue;
            }
            let id = x.id.clone();
            merged.entry(id).or_insert_with(|| MergedEmail { item: x.clone(), box_type: 1 });
        }
        for x in box2.emails.iter() {
            if x.id.is_empty() {
                continue;
            }
            let id = x.id.clone();
            let now_claimable = x.has_reward && !x.claimed;
            if let Some(old) = merged.get(&id).cloned() {
                let old_claimable = old.item.has_reward && !old.item.claimed;
                if !old_claimable && now_claimable {
                    merged.insert(id, MergedEmail { item: x.clone(), box_type: 2 });
                }
            } else {
                merged.insert(id, MergedEmail { item: x.clone(), box_type: 2 });
            }
        }

        let claimable: Vec<&MergedEmail> =
            merged.values().filter(|m| m.item.has_reward && !m.item.claimed).collect();

        if claimable.is_empty() {
            *self.done_date_key.lock() = today;
            tracing::info!("[邮箱] 今日暂无可领取邮箱奖励");
            return EmailClaimResult { claimed: 0, reward_items: 0 };
        }

        // 按 boxType 分组
        let mut by_box: HashMap<i32, Vec<String>> = HashMap::new();
        for m in &claimable {
            let box_type = normalize_box_type(m.box_type);
            by_box.entry(box_type).or_default().push(m.item.id.clone());
        }

        let mut rewards: Vec<RewardItem> = vec![];
        let mut claimed: usize = 0;

        // 先按 box 批量
        for (box_type, ids) in &by_box {
            if let Some(first_id) = ids.first() {
                match self.batch_claim_email(*box_type, first_id).await {
                    Ok(reply) => {
                        rewards.extend(claim_reply_items(&reply));
                        claimed += 1;
                    }
                    Err(_) => {
                        // 批量失败回退到单领
                    }
                }
            }
        }

        // 单领补全
        for m in &claimable {
            let box_type = normalize_box_type(m.box_type);
            match self.claim_email(box_type, &m.item.id).await {
                Ok(reply) => {
                    rewards.extend(claim_reply_items_from_single(&reply));
                    claimed += 1;
                }
                Err(_) => {
                    // 单封失败静默
                }
            }
        }

        if claimed > 0 {
            let summary = get_reward_summary(&rewards);
            tracing::info!(
                "[邮箱] 领取成功 {} 封 → {}",
                claimed,
                if summary.is_empty() { String::new() } else { summary }
            );
            *self.done_date_key.lock() = today;
        }

        EmailClaimResult { claimed, reward_items: rewards.len() }
    }

    /// 获取每日状态
    #[must_use]
    pub fn get_daily_state(&self) -> serde_json::Value {
        serde_json::json!({
            "key": DAILY_KEY,
            "doneToday": *self.done_date_key.lock() == get_date_key(),
            "lastCheckAt": *self.last_check_at.lock(),
        })
    }
}

// =====================================================================
// 辅助
// =====================================================================

#[derive(Debug, Clone)]
pub struct EmailClaimResult {
    pub claimed: usize,
    pub reward_items: usize,
}

#[derive(Debug, Clone)]
struct MergedEmail {
    item: EmailItem,
    box_type: i32,
}

fn get_date_key() -> String {
    use chrono::Datelike;
    use chrono::Local;
    let now = Local::now();
    format!("{}-{:02}-{:02}", now.year(), now.month(), now.day())
}

fn normalize_box_type(v: i32) -> i32 {
    if v == 1 || v == 2 {
        v
    } else {
        1
    }
}

fn claim_reply_items(_reply: &BatchClaimEmailReply) -> Vec<RewardItem> {
    Vec::new()
}

fn claim_reply_items_from_single(reply: &ClaimEmailReply) -> Vec<RewardItem> {
    reply
        .items
        .iter()
        .map(|i| RewardItem { id: i.id, count: i.count })
        .filter(|r| r.count > 0)
        .collect()
}

/// 单个奖励项（id + count），由调用方从 mail config / reward proto 提取
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RewardItem {
    pub id: i64,
    pub count: i64,
}

/// 汇总奖励为可读字符串
///
/// 对应原 email.ts `getRewardSummary`：
/// - id=1 / 1001 → 金币
/// - id=2 / 1101 → 经验
/// - id=1002 → 点券
/// - 其它 → 物品#{id}x{count}
#[must_use]
pub fn get_reward_summary(items: &[RewardItem]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for it in items {
        if it.count <= 0 {
            continue;
        }
        let id = it.id;
        let count = it.count;
        if id == 1 || id == 1001 {
            parts.push(format!("金币{count}"));
        } else if id == 2 || id == 1101 {
            parts.push(format!("经验{count}"));
        } else if id == 1002 {
            parts.push(format!("点券{count}"));
        } else {
            parts.push(format!("物品#{id}x{count}"));
        }
    }
    parts.join("/")
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
    fn normalize_box_type_valid() {
        assert_eq!(normalize_box_type(1), 1);
        assert_eq!(normalize_box_type(2), 2);
    }

    #[test]
    fn normalize_box_type_invalid_defaults_to_1() {
        assert_eq!(normalize_box_type(0), 1);
        assert_eq!(normalize_box_type(99), 1);
        assert_eq!(normalize_box_type(-1), 1);
    }

    #[test]
    fn date_key_format() {
        let k = get_date_key();
        assert_eq!(k.len(), 10);
        assert!(k.chars().nth(4) == Some('-'));
        assert!(k.chars().nth(7) == Some('-'));
    }

    #[test]
    fn reward_summary_empty() {
        assert_eq!(get_reward_summary(&[]), "");
    }

    #[test]
    fn reward_summary_with_items() {
        let items = vec![
            RewardItem { id: 1, count: 100 },   // 金币
            RewardItem { id: 1002, count: 50 }, // 点券
            RewardItem { id: 2, count: 200 },   // 经验
            RewardItem { id: 9999, count: 3 },  // 物品
            RewardItem { id: 1, count: 0 },     // 跳过
        ];
        assert_eq!(get_reward_summary(&items), "金币100/点券50/经验200/物品#9999x3");
    }
}
