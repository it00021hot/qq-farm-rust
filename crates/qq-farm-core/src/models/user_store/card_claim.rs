//! 卡密领取（按 UA 限流）。
//!
//! 1:1 翻译原 `core/src/models/user-store/card-claim.ts`（171 行）。

use std::fs;
use std::path::PathBuf;

use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::paths::{ensure_data_dir, get_data_file};
use crate::models::user_store::users;

const ONE_DAY_MS: i64 = 86_400_000;

// =====================================================================
// 类型
// =====================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CardClaimRecord {
    pub ua_hash: String,
    pub claim_time: i64,
    pub card_code: String,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UaClaimCheckResult {
    pub allowed: bool,
    pub remaining_ms: Option<i64>,
    pub message: Option<String>,
}

/// 领取结果
pub type ClaimResult = Result<ClaimSuccess, String>;

/// 领取成功
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimSuccess {
    pub card_code: String,
    pub days: i64,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CardClaimFile {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    records: Vec<CardClaimRecord>,
}

// =====================================================================
// 全局状态
// =====================================================================

static ENABLED: once_cell::sync::Lazy<parking_lot::RwLock<bool>> =
    once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(false));

static MESSAGE: once_cell::sync::Lazy<parking_lot::RwLock<String>> =
    once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(String::new()));

static RECORDS: once_cell::sync::Lazy<parking_lot::RwLock<Vec<CardClaimRecord>>> =
    once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(Vec::new()));

// =====================================================================
// 文件路径
// =====================================================================

#[must_use]
pub fn card_claim_file() -> PathBuf {
    get_data_file("card-claim.json")
}

// =====================================================================
// 加载 / 保存
// =====================================================================

pub fn load_card_claim_records() {
    let _ = ensure_data_dir();
    let path = card_claim_file();
    if !path.exists() {
        *ENABLED.write() = true;
        RECORDS.write().clear();
        save_card_claim_records();
        return;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => return,
    };
    if let Ok(data) = serde_json::from_str::<CardClaimFile>(&raw) {
        *ENABLED.write() = data.enabled;
        *RECORDS.write() = data.records;
    } else {
        *ENABLED.write() = true;
        RECORDS.write().clear();
    }
}

pub fn save_card_claim_records() {
    let _ = ensure_data_dir();
    let enabled = *ENABLED.read();
    let records = RECORDS.read().clone();
    let data = CardClaimFile { enabled, records };
    if let Ok(body) = serde_json::to_string_pretty(&data) {
        let path = card_claim_file();
        let tmp = path.with_extension("json.tmp");
        let _ = crate::infra::spawn_blocking(move || {
            let _ = fs::write(&tmp, &body);
            let _ = fs::rename(&tmp, &path);
        });
    }
}

// =====================================================================
// 状态查询
// =====================================================================

/// 获取卡密领取功能开关
#[must_use]
pub fn get_card_claim_status() -> bool {
    load_card_claim_records();
    *ENABLED.read()
}

/// 完整状态（含 message）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardClaimStatusDto {
    pub enabled: bool,
    pub message: String,
}

/// 获取完整状态
#[must_use]
pub fn get_status() -> CardClaimStatusDto {
    load_card_claim_records();
    CardClaimStatusDto { enabled: *ENABLED.read(), message: MESSAGE.read().clone() }
}

/// 设置卡密领取功能开关（带 message）
pub fn set_status(enabled: bool, message: Option<String>) -> bool {
    load_card_claim_records();
    *ENABLED.write() = enabled;
    if let Some(m) = message {
        *MESSAGE.write() = m;
    }
    save_card_claim_records();
    enabled
}

/// 设置卡密领取功能开关
pub fn set_card_claim_status(enabled: bool) -> bool {
    load_card_claim_records();
    *ENABLED.write() = enabled;
    save_card_claim_records();
    enabled
}

/// 全部领取记录
#[must_use]
pub fn get_card_claim_records() -> Vec<CardClaimRecord> {
    load_card_claim_records();
    RECORDS.read().clone()
}

/// 清理过期记录（> 24h）
pub fn clear_expired_claim_records() -> usize {
    load_card_claim_records();
    let now = crate::utils::time::now_ms();
    let before = RECORDS.read().len();
    RECORDS.write().retain(|r| (now - r.claim_time) < ONE_DAY_MS);
    let after = RECORDS.read().len();
    if before != after {
        save_card_claim_records();
    }
    before - after
}

// =====================================================================
// UA 校验 + 领取
// =====================================================================

/// 检查 UA 是否在 24h 内已领过
#[must_use]
pub fn check_ua_claim_limit(ua: &str) -> UaClaimCheckResult {
    load_card_claim_records();
    let now = crate::utils::time::now_ms();
    let ua_hash = hash_ua(ua);

    let records = RECORDS.read();
    if let Some(r) = records.iter().find(|r| r.ua_hash == ua_hash) {
        let elapsed = now - r.claim_time;
        if elapsed < ONE_DAY_MS {
            return UaClaimCheckResult {
                allowed: false,
                remaining_ms: Some(ONE_DAY_MS - elapsed),
                message: Some("您已经在24小时内领取过一次卡密了！".to_string()),
            };
        }
    }
    UaClaimCheckResult { allowed: true, remaining_ms: None, message: None }
}

/// 按 UA 领取一个时间卡密
pub fn claim_card_by_ua(ua: &str, username: Option<&str>) -> ClaimResult {
    users::load_cards();
    load_card_claim_records();

    if !*ENABLED.read() {
        return Err("卡密领取功能未开启".to_string());
    }

    let ua_check = check_ua_claim_limit(ua);
    if !ua_check.allowed {
        return Err(ua_check.message.unwrap_or_default());
    }

    // 从库存随机选一张时间卡密
    let all_cards = users::get_all_cards();
    let unused_time: Vec<_> = all_cards
        .into_iter()
        .filter(|c| c.card_type == "time" && c.used_by.is_none() && c.enabled)
        .collect();

    if unused_time.is_empty() {
        return Err("卡密库存不足，请联系管理员！".to_string());
    }

    let mut rng = rand::thread_rng();
    let selected = unused_time[rng.gen_range(0..unused_time.len())].clone();

    let ua_hash = hash_ua(ua);
    RECORDS.write().push(CardClaimRecord {
        ua_hash,
        claim_time: crate::utils::time::now_ms(),
        card_code: selected.code.clone(),
        username: username.map(String::from),
    });
    save_card_claim_records();

    Ok(ClaimSuccess {
        card_code: selected.code,
        days: selected.days,
        description: selected.description,
    })
}

fn hash_ua(ua: &str) -> String {
    let mut h = Sha256::new();
    h.update(ua.as_bytes());
    format!("{:x}", h.finalize())
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn reset() {
        RECORDS.write().clear();
        *ENABLED.write() = true;
        let _ = fs::remove_file(card_claim_file());
    }

    #[test]
    #[serial(user_store)]
    fn default_enabled() {
        reset();
        assert!(get_card_claim_status());
    }

    #[test]
    #[serial(user_store)]
    fn set_status() {
        reset();
        set_card_claim_status(false);
        assert!(!get_card_claim_status());
        set_card_claim_status(true);
        assert!(get_card_claim_status());
    }

    #[test]
    #[serial(user_store)]
    fn claim_blocked_when_disabled() {
        reset();
        set_card_claim_status(false);
        let r = claim_card_by_ua("test-ua", None);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("未开启"));
    }

    #[test]
    #[serial(user_store)]
    fn claim_no_cards_available() {
        reset();
        let r = claim_card_by_ua("test-ua", None);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("库存不足"));
    }

    #[test]
    #[serial(user_store)]
    fn claim_success() {
        reset();
        let _c = users::create_card("test", 30, "time");
        let r = claim_card_by_ua("test-ua", Some("alice"));
        assert!(r.is_ok(), "error = {:?}", r.err());
        assert_eq!(r.as_ref().unwrap().days, 30);
    }

    #[test]
    #[serial(user_store)]
    fn claim_rate_limit_24h() {
        reset();
        let _c1 = users::create_card("test", 30, "time");
        let _c2 = users::create_card("test", 30, "time");
        let r1 = claim_card_by_ua("same-ua", None);
        assert!(r1.is_ok());
        let r2 = claim_card_by_ua("same-ua", None);
        assert!(r2.is_err());
    }

    #[test]
    #[serial(user_store)]
    fn check_ua_limit_allows_first_time() {
        reset();
        let r = check_ua_claim_limit("new-ua");
        assert!(r.allowed);
    }

    #[test]
    #[serial(user_store)]
    fn clear_expired_keeps_recent() {
        reset();
        let _c = users::create_card("test", 30, "time");
        claim_card_by_ua("u1", None).ok();
        claim_card_by_ua("u2", None).ok();
        let cleared = clear_expired_claim_records();
        assert_eq!(cleared, 0); // 都在 24h 内
        assert_eq!(get_card_claim_records().len(), 2);
    }
}
