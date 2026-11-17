//! 邮箱系统 — 每天在设定的钟点检查一次并领取奖励。
//!
//! 默认 10 点。领取完成、确认没有奖励或检查失败后，当天不再重复。执行到一半留下的记录不算完成，下次重新读取还没领的邮件。
//!
//! ## 协议
//!
//! - `gamepb.emailpb.EmailService.GetEmailList(boxType)` — 拉取邮箱列表
//! - `gamepb.emailpb.EmailService.ClaimEmail(boxType, emailId)` — 单封领取
//! - `gamepb.emailpb.EmailService.BatchClaimEmail(boxType, emailIds)` — 批量领取
//! - `gamepb.emailpb.EmailService.BatchDeleteEmail(boxType, emailIds)` — 批量删除

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::emailpb::{
    BatchClaimEmailReply, BatchClaimEmailRequest, BatchDeleteEmailReply, BatchDeleteEmailRequest,
    ClaimEmailReply, ClaimEmailRequest, EmailItem, GetEmailListReply, GetEmailListRequest,
};

const DAILY_KEY: &str = "email_rewards";
const DEFAULT_CLAIM_HOUR: i64 = 10;
const STATE_VERSION: i64 = 1;

/// 邮箱服务
pub struct EmailService {
    gateway: Arc<Gateway>,
    account_id: String,
}

impl EmailService {
    #[must_use]
    pub fn new(gateway: Arc<Gateway>, account_id: impl Into<String>) -> Self {
        Self { gateway, account_id: account_id.into() }
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
        GetEmailListReply::decode(&body)
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
        ClaimEmailReply::decode(&body)
    }

    /// 批量领取
    pub async fn batch_claim_email(
        &self,
        box_type: i32,
        email_ids: &[String],
    ) -> Result<BatchClaimEmailReply> {
        let req = BatchClaimEmailRequest { box_type, email_ids: email_ids.to_vec() };
        let body = self
            .gateway
            .request(
                "gamepb.emailpb.EmailService",
                "BatchClaimEmail",
                &prost::Message::encode_to_vec(&req),
            )
            .await?;
        BatchClaimEmailReply::decode(&body)
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
        BatchDeleteEmailReply::decode(&body)
    }

    /// 到了设定钟点就检查一次。当天已经有最终结果时直接返回。
    pub async fn check_and_claim_emails(&self, force: bool) -> EmailClaimResult {
        let today = crate::utils::time::today_system_date_key();
        let hour = claim_hour(&self.account_id);
        let stored = load_state(&self.account_id, &today);
        let reached = claim_time_reached(hour, crate::utils::time::now_system_clock_minutes());
        match claim_gate(stored.is_done(), stored.result == "running", force, reached) {
            ClaimGate::Done | ClaimGate::Running | ClaimGate::Waiting => {
                return EmailClaimResult { claimed: 0, reward_items: 0 };
            }
            ClaimGate::Proceed => {}
        }
        if !mark_attempt(&self.account_id, &today) {
            tracing::error!("无法保存邮箱检查状态，本次跳过以免重复请求");
            return EmailClaimResult { claimed: 0, reward_items: 0 };
        }

        let mut failures = Vec::new();
        let box1 = match self.get_email_list(1).await {
            Ok(reply) => reply,
            Err(error) => {
                failures.push(error.to_string());
                GetEmailListReply::default()
            }
        };
        let box2 = match self.get_email_list(2).await {
            Ok(reply) => reply,
            Err(error) => {
                failures.push(error.to_string());
                GetEmailListReply::default()
            }
        };
        let claimable = merged_claimable(&box1.emails, &box2.emails);
        if claimable.is_empty() {
            if let Some(message) = failures.first() {
                finish_attempt(&self.account_id, &today, "error", message, 0);
                tracing::error!(%message, "领取邮箱奖励失败");
            } else {
                finish_attempt(&self.account_id, &today, "none", "", 0);
                tracing::info!("当前暂无可领取邮箱奖励");
            }
            return EmailClaimResult { claimed: 0, reward_items: 0 };
        }

        let mut by_box: HashMap<i32, Vec<String>> = HashMap::new();
        for mail in &claimable {
            by_box.entry(normalize_box_type(mail.box_type)).or_default().push(mail.item.id.clone());
        }
        let mut batch_claimed = HashSet::new();
        let mut claimed = 0usize;
        for (box_type, ids) in &by_box {
            if ids.is_empty() {
                continue;
            }
            if self.batch_claim_email(*box_type, ids).await.is_ok() {
                for id in ids {
                    batch_claimed.insert(format!("{box_type}:{id}"));
                }
                claimed += ids.len();
            }
        }
        let mut rewards = Vec::new();
        for mail in &claimable {
            let box_type = normalize_box_type(mail.box_type);
            if batch_claimed.contains(&format!("{box_type}:{}", mail.item.id)) {
                continue;
            }
            match self.claim_email(box_type, &mail.item.id).await {
                Ok(reply) => {
                    rewards.extend(claim_reply_items_from_single(&reply));
                    claimed += 1;
                }
                Err(error) => failures.push(error.to_string()),
            }
        }
        if claimed > 0 {
            let incomplete = claimed < claimable.len() || !failures.is_empty();
            finish_attempt(
                &self.account_id,
                &today,
                if incomplete { "partial" } else { "ok" },
                if incomplete { "部分邮件未能领取" } else { "" },
                i64::try_from(claimed).unwrap_or(i64::MAX),
            );
            let summary = get_reward_summary(&rewards);
            tracing::info!(claimed, total = claimable.len(), summary, "邮箱领取完成");
        } else {
            finish_attempt(&self.account_id, &today, "error", "本次没有邮件领取成功", 0);
            tracing::error!(count = claimable.len(), "邮箱奖励领取失败：本次没有邮件领取成功");
        }
        EmailClaimResult { claimed, reward_items: rewards.len() }
    }

    /// 获取每日状态
    #[must_use]
    pub fn get_daily_state(&self) -> serde_json::Value {
        let today = crate::utils::time::today_system_date_key();
        let hour = claim_hour(&self.account_id);
        let stored = load_state(&self.account_id, &today);
        let reached = claim_time_reached(hour, crate::utils::time::now_system_clock_minutes());
        let server_ms = crate::utils::time::get_server_time_ms();
        let now = crate::utils::time::now_ms();
        serde_json::json!({
            "key": DAILY_KEY,
            "doneToday": stored.is_done(),
            "lastCheckAt": stored.attempted_at,
            "nextCheckAt": next_check_at(stored.is_done(), reached, now, server_ms, crate::utils::time::now_system_clock_minutes(), hour),
            "scheduledHour": hour,
            "result": stored.result,
            "lastError": stored.last_error,
            "claimed": stored.claimed,
        })
    }
}

// =====================================================================
// 辅助
// =====================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClaimGate {
    Proceed,
    Done,
    Running,
    Waiting,
}

#[derive(Clone, Debug, Default)]
struct StoredClaim {
    result: String,
    last_error: String,
    claimed: i64,
    attempted_at: i64,
    checked_today: bool,
}

impl StoredClaim {
    fn is_done(&self) -> bool {
        self.checked_today && self.result != "running"
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ClaimFile {
    version: i64,
    #[serde(rename = "dateKey")]
    date_key: String,
    #[serde(rename = "attemptedAt", default)]
    attempted_at: i64,
    #[serde(default)]
    result: String,
    #[serde(rename = "lastError", default)]
    last_error: String,
    #[serde(default)]
    claimed: i64,
}

/// 只接受 0–23 的整点。
#[must_use]
pub fn normalize_email_claim_hour(hour: i64) -> Option<i64> {
    (0..=23).contains(&hour).then_some(hour)
}

fn claim_hour(account_id: &str) -> i64 {
    let stored = crate::models::store::account_config::get_config_snapshot(Some(account_id));
    normalize_email_claim_hour(stored.config.email_claim_hour).unwrap_or(DEFAULT_CLAIM_HOUR)
}

fn claim_time_reached(hour: i64, minutes_now: u32) -> bool {
    let hour = normalize_email_claim_hour(hour).unwrap_or(DEFAULT_CLAIM_HOUR);
    minutes_now >= u32::try_from(hour.saturating_mul(60)).unwrap_or(u32::MAX)
}

fn claim_gate(done: bool, running: bool, force: bool, reached: bool) -> ClaimGate {
    if done {
        ClaimGate::Done
    } else if running {
        ClaimGate::Running
    } else if !force && !reached {
        ClaimGate::Waiting
    } else {
        ClaimGate::Proceed
    }
}

fn next_check_at(
    done: bool,
    reached: bool,
    now_ms: i64,
    server_ms: i64,
    minutes_now: u32,
    hour: i64,
) -> i64 {
    if !done && reached {
        return 0;
    }
    let hour = normalize_email_claim_hour(hour).unwrap_or(DEFAULT_CLAIM_HOUR);
    let mut delta = hour * 60 - i64::from(minutes_now);
    if delta <= 0 {
        delta += 24 * 60;
    }
    let second = server_ms.rem_euclid(60_000) / 1000;
    now_ms + delta * 60_000 - second * 1000
}

fn load_state(account_id: &str, today: &str) -> StoredClaim {
    if let Some(stored) = remembered_claim(account_id, today) {
        return stored;
    }
    let Some(file) = read_state(account_id) else {
        return StoredClaim::default();
    };
    if file.version != STATE_VERSION || file.date_key != today || file.result == "running" {
        return StoredClaim::default();
    }
    stored_from_file(&file)
}

fn mark_attempt(account_id: &str, today: &str) -> bool {
    let stored = StoredClaim {
        result: "running".to_owned(),
        last_error: String::new(),
        claimed: 0,
        attempted_at: crate::utils::time::now_ms(),
        checked_today: true,
    };
    if !write_state(account_id, &file_from_stored(today, &stored)) {
        return false;
    }
    remember_claim(account_id, today, stored);
    true
}

fn finish_attempt(account_id: &str, today: &str, result: &str, error: &str, claimed: i64) {
    let stored = StoredClaim {
        result: result.to_owned(),
        last_error: error.to_owned(),
        claimed,
        attempted_at: crate::utils::time::now_ms(),
        checked_today: true,
    };
    let _ = write_state(account_id, &file_from_stored(today, &stored));
    remember_claim(account_id, today, stored);
}

fn stored_from_file(file: &ClaimFile) -> StoredClaim {
    StoredClaim {
        result: file.result.clone(),
        last_error: file.last_error.clone(),
        claimed: file.claimed.max(0),
        attempted_at: file.attempted_at.max(0),
        checked_today: true,
    }
}

fn file_from_stored(today: &str, stored: &StoredClaim) -> ClaimFile {
    ClaimFile {
        version: STATE_VERSION,
        date_key: today.to_owned(),
        attempted_at: stored.attempted_at,
        result: stored.result.clone(),
        last_error: stored.last_error.clone(),
        claimed: stored.claimed,
    }
}

struct ProcessClaim {
    date_key: String,
    stored: StoredClaim,
}

fn process_claims() -> &'static Mutex<HashMap<String, ProcessClaim>> {
    static CLAIMS: std::sync::OnceLock<Mutex<HashMap<String, ProcessClaim>>> =
        std::sync::OnceLock::new();
    CLAIMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn remembered_claim(account_id: &str, today: &str) -> Option<StoredClaim> {
    let claims = process_claims().lock();
    let claim = claims.get(account_id)?;
    (claim.date_key == today).then(|| claim.stored.clone())
}

fn remember_claim(account_id: &str, today: &str, stored: StoredClaim) {
    process_claims()
        .lock()
        .insert(account_id.to_owned(), ProcessClaim { date_key: today.to_owned(), stored });
}

fn merged_claimable(box1: &[EmailItem], box2: &[EmailItem]) -> Vec<MergedEmail> {
    let mut merged: HashMap<String, MergedEmail> = HashMap::new();
    for item in box1 {
        if item.id.is_empty() {
            continue;
        }
        merged
            .entry(item.id.clone())
            .or_insert_with(|| MergedEmail { item: item.clone(), box_type: 1 });
    }
    for item in box2 {
        if item.id.is_empty() {
            continue;
        }
        let claimable = item.has_reward && !item.claimed;
        if let Some(old) = merged.get(&item.id) {
            let old_claimable = old.item.has_reward && !old.item.claimed;
            if !old_claimable && claimable {
                merged.insert(item.id.clone(), MergedEmail { item: item.clone(), box_type: 2 });
            }
        } else {
            merged.insert(item.id.clone(), MergedEmail { item: item.clone(), box_type: 2 });
        }
    }
    merged.into_values().filter(|mail| mail.item.has_reward && !mail.item.claimed).collect()
}

fn state_path(account_id: &str) -> PathBuf {
    let digest = hex::encode(Sha256::digest(account_id.as_bytes()));
    let name = format!("email-claim-{digest}.json");
    #[cfg(test)]
    if let Some(dir) = test_root() {
        return dir.join(name);
    }
    crate::config::paths::get_data_file(&name)
}

fn file_lock() -> &'static Mutex<()> {
    static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn read_state(account_id: &str) -> Option<ClaimFile> {
    let _guard = file_lock().lock();
    let path = state_path(account_id);
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_state(account_id: &str, state: &ClaimFile) -> bool {
    let _guard = file_lock().lock();
    let path = state_path(account_id);
    let Some(parent) = path.parent() else {
        return false;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return false;
    }
    let text = match serde_json::to_string(state) {
        Ok(text) => text,
        Err(_) => return false,
    };
    let temporary = path.with_extension("json.tmp");
    if std::fs::write(&temporary, text).is_err() {
        return false;
    }
    replace_file(&temporary, &path)
}

fn replace_file(temporary: &Path, path: &Path) -> bool {
    if std::fs::rename(temporary, path).is_ok() {
        return true;
    }
    let copied = std::fs::copy(temporary, path).is_ok();
    let _ = std::fs::remove_file(temporary);
    copied
}

#[cfg(test)]
fn test_root() -> Option<PathBuf> {
    TEST_ROOT.with(|root| root.borrow().clone())
}

#[cfg(test)]
thread_local! {
    static TEST_ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

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

fn normalize_box_type(v: i32) -> i32 {
    if v == 1 || v == 2 {
        v
    } else {
        1
    }
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
        let k = crate::utils::time::today_system_date_key();
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

    #[test]
    fn claim_waits_for_the_configured_hour_and_resumes_a_running_record() {
        assert_eq!(normalize_email_claim_hour(0), Some(0));
        assert_eq!(normalize_email_claim_hour(23), Some(23));
        assert_eq!(normalize_email_claim_hour(24), None);
        assert!(!claim_time_reached(10, 9 * 60 + 59));
        assert!(claim_time_reached(10, 10 * 60));
        assert_eq!(claim_gate(true, false, true, true), ClaimGate::Done);
        assert_eq!(claim_gate(false, true, true, true), ClaimGate::Running);
        assert_eq!(claim_gate(false, false, false, false), ClaimGate::Waiting);
        assert_eq!(claim_gate(false, false, true, false), ClaimGate::Proceed);
        assert_eq!(next_check_at(false, true, 1_000, 30_000, 600, 10), 0);
        assert_eq!(
            next_check_at(false, false, 1_000_000, 30_000, 8 * 60, 10),
            1_000_000 + 2 * 60 * 60_000 - 30_000
        );

        let dir = std::env::temp_dir().join(format!("qq-farm-email-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        TEST_ROOT.with(|root| *root.borrow_mut() = Some(dir.clone()));
        let account = "email-schedule";
        let today = "2026-11-17";
        assert!(!load_state(account, today).is_done());
        assert!(mark_attempt(account, today));
        let running = read_state(account).expect("running");
        assert_eq!(running.result, "running");
        assert!(!load_state(account, today).is_done());
        finish_attempt(account, today, "ok", "", 2);
        assert!(load_state(account, today).is_done());
        assert!(!load_state(account, "2026-11-18").is_done());
        TEST_ROOT.with(|root| *root.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn restart_drops_a_running_record_but_the_same_process_keeps_it() {
        let dir = std::env::temp_dir().join(format!("qq-farm-email-resume-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        TEST_ROOT.with(|root| *root.borrow_mut() = Some(dir.clone()));
        let account = "email-resume";
        let today = "2026-11-17";
        assert!(write_state(
            account,
            &ClaimFile {
                version: STATE_VERSION,
                date_key: today.to_owned(),
                attempted_at: 1,
                result: "running".to_owned(),
                last_error: String::new(),
                claimed: 0,
            },
        ));
        let restarted = load_state(account, today);
        assert!(!restarted.is_done());
        assert!(restarted.result.is_empty());
        assert_eq!(
            claim_gate(restarted.is_done(), restarted.result == "running", false, true),
            ClaimGate::Proceed
        );

        assert!(mark_attempt(account, today));
        let running = load_state(account, today);
        assert_eq!(running.result, "running");
        assert!(!running.is_done());
        assert_eq!(
            claim_gate(running.is_done(), running.result == "running", true, true),
            ClaimGate::Running
        );

        finish_attempt(account, today, "none", "", 0);
        let finished = load_state(account, today);
        assert!(finished.is_done());
        assert_eq!(finished.result, "none");
        assert_eq!(
            claim_gate(finished.is_done(), finished.result == "running", true, true),
            ClaimGate::Done
        );
        TEST_ROOT.with(|root| *root.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
