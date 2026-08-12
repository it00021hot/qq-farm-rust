//! 用户系统（注册 / 登录 / 续费 / 卡密管理）。
//!
//! 1:1 翻译原 `core/src/models/user-store/users.ts`（707 行）。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::config::paths::{ensure_data_dir, get_data_file};
use crate::models::user_store::auth;

pub const DEFAULT_ACCOUNT_LIMIT: i64 = 2;
const CARD_CODE_LENGTH: usize = 16;
const CARD_CODE_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

// =====================================================================
// 类型
// =====================================================================

/// 用户卡密
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserCard {
    pub code: String,
    pub description: String,
    pub days: i64,
    /// None = 永久
    pub expires_at: Option<i64>,
    pub enabled: bool,
}

/// 用户
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password: String,
    pub role: String, // "admin" | "user"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<UserCard>,
    #[serde(default)]
    pub account_limit: i64,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_change_password: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wx_login_config: Option<serde_json::Value>,
}

/// 卡密
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Card {
    pub code: String,
    pub description: String,
    pub days: i64,
    /// "time" | "quota"
    #[serde(rename = "type")]
    pub card_type: String,
    pub enabled: bool,
    pub used_by: Option<String>,
    pub used_at: Option<i64>,
    pub created_at: i64,
}

// =====================================================================
// 文件路径
// =====================================================================

#[must_use]
pub fn users_file() -> PathBuf {
    get_data_file("users.json")
}

#[must_use]
pub fn cards_file() -> PathBuf {
    get_data_file("cards.json")
}

// =====================================================================
// 全局状态
// =====================================================================

static USERS: once_cell::sync::Lazy<parking_lot::RwLock<Vec<User>>> =
    once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(Vec::new()));

static CARDS: once_cell::sync::Lazy<parking_lot::RwLock<Vec<Card>>> =
    once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(Vec::new()));

fn read_json_or_default<T: serde::de::DeserializeOwned + Default>(path: &PathBuf) -> T {
    let raw = match fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return T::default(),
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_json_atomic<T: serde::Serialize>(path: &PathBuf, value: &T) {
    let _ = ensure_data_dir();
    if let Ok(body) = serde_json::to_string_pretty(value) {
        let tmp = path.with_extension("json.tmp");
        let _ = fs::write(&tmp, body);
        let _ = fs::rename(&tmp, path);
    }
}

// =====================================================================
// 加载 / 保存
// =====================================================================

pub fn load_users() {
    let data: UsersFile = read_json_or_default(&users_file());
    *USERS.write() = data.users;
}

pub fn save_users() {
    let users = USERS.read().clone();
    write_json_atomic(&users_file(), &UsersFile { users });
}

pub fn load_cards() {
    let data: CardsFile = read_json_or_default(&cards_file());
    *CARDS.write() = data.cards;
}

pub fn save_cards() {
    let cards = CARDS.read().clone();
    write_json_atomic(&cards_file(), &CardsFile { cards });
}

#[derive(Default, Serialize, Deserialize)]
struct UsersFile {
    #[serde(default)]
    users: Vec<User>,
}

#[derive(Default, Serialize, Deserialize)]
struct CardsFile {
    #[serde(default)]
    cards: Vec<Card>,
}

// =====================================================================
// 初始化默认管理员
// =====================================================================

/// 初始化默认 admin 账号（密码 admin/admin）
pub fn init_default_admin() {
    load_users();
    let exists = USERS.read().iter().any(|u| u.username == "admin");
    if !exists {
        let new_user = User {
            username: "admin".to_string(),
            password: auth::hash_password("admin", None),
            role: "admin".to_string(),
            account_limit: DEFAULT_ACCOUNT_LIMIT,
            created_at: crate::utils::time::now_secs(),
            ..Default::default()
        };
        USERS.write().push(new_user);
        save_users();
        tracing::info!("[用户系统] 已创建默认管理员账号，默认密码: admin");
    }
}

// =====================================================================
// 验证 / 注册 / 续费
// =====================================================================

/// 验证结果
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ValidationResult {
    pub username: Option<String>,
    pub role: Option<String>,
    pub card_code: Option<String>,
    pub card: Option<UserCard>,
    pub account_limit: Option<i64>,
    pub error: Option<String>,
    pub message: Option<String>,
    pub remaining_ms: Option<i64>,
}

/// 验证用户
pub fn validate_user(username: &str, password: &str, ip: &str) -> ValidationResult {
    load_users();
    auth::load_login_attempts();

    let rate = auth::check_rate_limit(ip);
    if !rate.allowed {
        return ValidationResult {
            error: Some("rate_limit".to_string()),
            message: rate.message,
            remaining_ms: rate.remaining_ms,
            ..Default::default()
        };
    }

    let lockout = auth::check_account_lockout(username);
    if lockout.locked {
        return ValidationResult {
            error: Some("locked".to_string()),
            message: lockout.message,
            remaining_ms: lockout.remaining_ms,
            ..Default::default()
        };
    }

    let user = USERS.read().iter().find(|u| u.username == username).cloned();
    let Some(user) = user else {
        auth::record_failed_attempt(username);
        return ValidationResult {
            error: Some("invalid_credentials".to_string()),
            message: Some("用户名或密码错误".to_string()),
            ..Default::default()
        };
    };

    if !auth::verify_password(password, &user.password) {
        let r = auth::record_failed_attempt(username);
        if r.locked {
            return ValidationResult {
                error: Some("locked".to_string()),
                message: r.message,
                ..Default::default()
            };
        }
        return ValidationResult {
            error: Some("invalid_credentials".to_string()),
            message: Some(format!(
                "用户名或密码错误，剩余尝试次数: {}",
                r.remaining_attempts.unwrap_or(0)
            )),
            ..Default::default()
        };
    }

    auth::clear_failed_attempts(username);

    // 升级老 hash
    let mut user = user;
    if auth::needs_rehash(&user.password) {
        user.password = auth::hash_password(password, None);
        if let Some(u) = USERS.write().iter_mut().find(|u| u.username == user.username) {
            u.password = user.password.clone();
        }
        save_users();
        tracing::info!(username = %user.username, "[安全] 密码已升级为新哈希算法");
    }

    ValidationResult {
        username: Some(user.username),
        role: Some(user.role),
        card_code: user.card_code,
        card: user.card,
        account_limit: Some(user.account_limit.max(1)),
        ..Default::default()
    }
}

/// 用户摘要
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserSummary {
    pub username: String,
    pub role: String,
    pub card: Option<UserCard>,
    pub account_limit: i64,
}

/// 注册结果
pub type RegisterResult = Result<UserSummary, String>;

/// 注册用户
pub fn register_user(username: &str, password: &str, card_code: &str) -> RegisterResult {
    load_users();
    load_cards();

    if username.len() < 3 || username.len() > 32 {
        return Err("用户名长度需在3-32位之间".to_string());
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("用户名只能包含字母、数字和下划线".to_string());
    }
    if USERS.read().iter().any(|u| u.username == username) {
        return Err("用户名已存在".to_string());
    }

    create_user_internal(username, password, "user", card_code)
}

/// 管理员创建用户（指定 role）
pub fn create_user_with_role(
    username: &str,
    password: &str,
    role: &str,
    card_code: &str,
) -> RegisterResult {
    load_users();
    load_cards();

    if USERS.read().iter().any(|u| u.username == username) {
        return Err("用户名已存在".to_string());
    }
    create_user_internal(username, password, role, card_code)
}

fn create_user_internal(
    username: &str,
    password: &str,
    role: &str,
    card_code: &str,
) -> RegisterResult {

    let pw = auth::validate_password_strength(password);
    if !pw.valid {
        return Err(pw.errors.join("；"));
    }

    let card = CARDS.read().iter().find(|c| c.code == card_code).cloned();
    let Some(card) = card else {
        return Err("卡密不存在".to_string());
    };
    if !card.enabled {
        return Err("卡密已被禁用".to_string());
    }
    if card.used_by.is_some() {
        return Err("卡密已被使用".to_string());
    }
    let card_type = if card.card_type.is_empty() { "time" } else { &card.card_type };
    if card_type == "quota" {
        return Err("注册只能使用时间卡密，额度卡密请登录后在续费中使用".to_string());
    }

    let now_secs = crate::utils::time::now_secs();
    let new_user = User {
        username: username.to_string(),
        password: auth::hash_password(password, None),
        role: "user".to_string(),
        card_code: Some(card_code.to_string()),
        card: Some(UserCard {
            code: card.code.clone(),
            description: card.description.clone(),
            days: card.days,
            expires_at: if card.days == -1 { None } else { Some(crate::utils::time::now_ms() + card.days * 86_400_000) },
            enabled: true,
        }),
        account_limit: DEFAULT_ACCOUNT_LIMIT,
        created_at: now_secs,
        ..Default::default()
    };

    USERS.write().push(new_user.clone());
    if let Some(c) = CARDS.write().iter_mut().find(|c| c.code == card_code) {
        c.used_by = Some(username.to_string());
        c.used_at = Some(now_secs);
    }
    save_users();
    save_cards();
    auth::clear_failed_attempts(username);

    Ok(UserSummary {
        username: new_user.username,
        role: new_user.role,
        card: new_user.card,
        account_limit: new_user.account_limit,
    })
}

/// 续费结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RenewResult {
    pub card: Option<UserCard>,
    pub account_limit: Option<i64>,
    pub card_type: Option<String>,
    pub added_sec: Option<i64>,
}

/// 续费
pub fn renew_user(username: &str, card_code: &str) -> Result<RenewResult, String> {
    load_users();
    load_cards();

    let mut users = USERS.write();
    let Some(user) = users.iter_mut().find(|u| u.username == username) else {
        return Err("用户不存在".to_string());
    };

    let Some(card) = CARDS.read().iter().find(|c| c.code == card_code).cloned() else {
        return Err("卡密不存在".to_string());
    };
    if !card.enabled {
        return Err("卡密已被禁用".to_string());
    }
    if card.used_by.is_some() {
        return Err("卡密已被使用".to_string());
    }

    let now_secs = crate::utils::time::now_secs();
    let now_ms = crate::utils::time::now_ms();
    let card_type = if card.card_type.is_empty() { "time".to_string() } else { card.card_type.clone() };

    if card_type == "quota" {
        let current_limit = if user.account_limit == 0 { DEFAULT_ACCOUNT_LIMIT } else { user.account_limit };
        user.account_limit = current_limit + card.days;
    } else {
        if user.card.is_none() {
            user.card = Some(UserCard {
                code: card.code.clone(),
                description: card.description.clone(),
                days: 0,
                expires_at: None,
                enabled: true,
            });
        }
        let uc = user.card.as_mut().unwrap();
        let current_expires = uc.expires_at.unwrap_or(0);
        let current_days = uc.days;

        if card.days == -1 {
            uc.expires_at = None;
            uc.days = -1;
        } else if current_days == -1 {
            uc.expires_at = None;
        } else {
            uc.days = current_days + card.days;
            if current_expires > now_ms {
                uc.expires_at = Some(current_expires + card.days * 86_400_000);
            } else {
                uc.expires_at = Some(now_ms + card.days * 86_400_000);
            }
        }
    }

    if let Some(c) = CARDS.write().iter_mut().find(|c| c.code == card_code) {
        c.used_by = Some(username.to_string());
        c.used_at = Some(now_secs);
    }

    let updated_card = user.card.clone();
    let updated_limit = user.account_limit;
    let added_sec = card.days * 86_400; // days → sec
    drop(users);
    save_users();
    save_cards();

    Ok(RenewResult {
        card: updated_card,
        account_limit: Some(updated_limit),
        card_type: Some(card_type),
        added_sec: Some(added_sec),
    })
}

// =====================================================================
// 用户 CRUD
// =====================================================================

/// 全部用户（脱敏：不含 password）
#[must_use]
pub fn get_all_users() -> Vec<UserSummary> {
    load_users();
    USERS
        .read()
        .iter()
        .map(|u| UserSummary {
            username: u.username.clone(),
            role: u.role.clone(),
            card: u.card.clone(),
            account_limit: if u.account_limit == 0 { DEFAULT_ACCOUNT_LIMIT } else { u.account_limit },
        })
        .collect()
}

/// 更新用户卡密（expiresAt / enabled）
#[must_use]
pub fn update_user(username: &str, expires_at: Option<Option<i64>>, enabled: Option<bool>) -> Option<UserSummary> {
    load_users();
    let mut users = USERS.write();
    let user = users.iter_mut().find(|u| u.username == username)?;
    if let Some(exp) = expires_at {
        if user.card.is_none() {
            user.card = Some(UserCard::default());
        }
        user.card.as_mut().unwrap().expires_at = exp;
    }
    if let Some(en) = enabled {
        if user.card.is_none() {
            user.card = Some(UserCard::default());
        }
        user.card.as_mut().unwrap().enabled = en;
    }
    let card = user.card.clone();
    let account_limit = user.account_limit;
    drop(users);
    save_users();
    Some(UserSummary {
        username: username.to_string(),
        role: "user".to_string(),
        card,
        account_limit: if account_limit == 0 { DEFAULT_ACCOUNT_LIMIT } else { account_limit },
    })
}

/// 编辑用户更新
#[derive(Debug, Clone, Default)]
pub struct EditUpdates {
    pub new_username: Option<String>,
    pub password: Option<String>,
    pub account_limit: Option<i64>,
    pub is_permanent: bool,
    pub expires_at: Option<Option<i64>>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub card_code: Option<String>,
}

/// 编辑结果
pub type EditResult = Result<UserSummary, String>;

/// 编辑用户
pub fn edit_user(old_username: &str, updates: EditUpdates) -> EditResult {
    load_users();
    let mut users = USERS.write();

    // 1) 先校验：用户存在 + 新用户名是否合法/重名
    if !users.iter().any(|u| u.username == old_username) {
        return Err("用户不存在".to_string());
    }
    if let Some(new_name) = &updates.new_username {
        if new_name != old_username {
            if new_name.len() < 3
                || new_name.len() > 32
                || !new_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err("用户名只能包含字母、数字和下划线，长度3-32位".to_string());
            }
            if users.iter().any(|u| u.username == *new_name) {
                return Err("用户名已存在".to_string());
            }
        }
    }

    // 2) 拿到可变引用并修改
    let user = users.iter_mut().find(|u| u.username == old_username).unwrap();

    if let Some(new_name) = updates.new_username {
        if new_name != old_username {
            user.username = new_name;
        }
    }

    if let Some(pw) = updates.password {
        let v = auth::validate_password_strength(&pw);
        if !v.valid {
            return Err(v.errors.join("；"));
        }
        user.password = auth::hash_password(&pw, None);
    }

    if let Some(limit) = updates.account_limit {
        user.account_limit = limit;
    }

    if updates.is_permanent {
        if user.card.is_none() {
            user.card = Some(UserCard::default());
        }
        let uc = user.card.as_mut().unwrap();
        uc.days = -1;
        uc.expires_at = None;
    } else if let Some(exp) = updates.expires_at {
        if user.card.is_none() {
            user.card = Some(UserCard::default());
        }
        let uc = user.card.as_mut().unwrap();
        if exp.is_none() {
            uc.days = 0;
            uc.expires_at = None;
        } else {
            let expires_at = exp.unwrap();
            uc.expires_at = Some(expires_at);
            let now_ms = crate::utils::time::now_ms();
            let diff_ms = expires_at - now_ms;
            let diff_days = if diff_ms > 0 { (diff_ms + 86_400_000 - 1) / 86_400_000 } else { 0 };
            uc.days = diff_days;
        }
    }

    let card = user.card.clone();
    let account_limit = user.account_limit;
    let username = user.username.clone();
    let role = user.role.clone();
    drop(users);
    save_users();

    Ok(UserSummary {
        username,
        role,
        card,
        account_limit: if account_limit == 0 { DEFAULT_ACCOUNT_LIMIT } else { account_limit },
    })
}

/// 删除用户
pub fn delete_user(username: &str) -> bool {
    load_users();
    let mut users = USERS.write();
    let before = users.len();
    users.retain(|u| u.username != username);
    let removed = users.len() != before;
    drop(users);
    if removed {
        save_users();
    }
    removed
}

/// 修改密码
pub fn change_password(username: &str, old_pw: &str, new_pw: &str) -> Result<(), String> {
    load_users();
    let mut users = USERS.write();
    let Some(user) = users.iter_mut().find(|u| u.username == username) else {
        return Err("用户不存在".to_string());
    };
    if !auth::verify_password(old_pw, &user.password) {
        return Err("原密码错误".to_string());
    }
    let v = auth::validate_password_strength(new_pw);
    if !v.valid {
        return Err(v.errors.join("；"));
    }
    user.password = auth::hash_password(new_pw, None);
    drop(users);
    save_users();
    Ok(())
}

/// 获取 session 用户（已登录 session）
#[must_use]
pub fn get_session_user(username: &str) -> Option<UserSummary> {
    load_users();
    USERS.read().iter().find(|u| u.username == username).map(|u| UserSummary {
        username: u.username.clone(),
        role: u.role.clone(),
        card: u.card.clone(),
        account_limit: if u.account_limit == 0 { DEFAULT_ACCOUNT_LIMIT } else { u.account_limit },
    })
}

// =====================================================================
// 卡密管理
// =====================================================================

/// 全部卡密
#[must_use]
pub fn get_all_cards() -> Vec<Card> {
    load_cards();
    CARDS.read().clone()
}

fn generate_card_code() -> String {
    let mut rng = rand::thread_rng();
    (0..CARD_CODE_LENGTH)
        .map(|_| CARD_CODE_CHARS[rng.gen_range(0..CARD_CODE_CHARS.len())] as char)
        .collect()
}

/// 创建卡密
pub fn create_card(description: &str, days: i64, card_type: &str) -> Card {
    load_cards();
    let new_card = Card {
        code: generate_card_code(),
        description: description.to_string(),
        days: if days == 0 { 30 } else { days },
        card_type: if card_type == "quota" { "quota".to_string() } else { "time".to_string() },
        enabled: true,
        used_by: None,
        used_at: None,
        created_at: crate::utils::time::now_secs(),
    };
    CARDS.write().push(new_card.clone());
    save_cards();
    new_card
}

/// 批量创建
pub fn create_cards_batch(description: &str, days: i64, count: i64, card_type: &str) -> Vec<Card> {
    load_cards();
    let days_n = if days == 0 { 30 } else { days };
    let count_n = count.clamp(1, 100);
    let ctype = if card_type == "quota" { "quota".to_string() } else { "time".to_string() };
    let mut created = Vec::new();
    let now_secs = crate::utils::time::now_secs();
    let mut cards = CARDS.write();
    for _ in 0..count_n {
        let c = Card {
            code: generate_card_code(),
            description: description.to_string(),
            days: days_n,
            card_type: ctype.clone(),
            enabled: true,
            used_by: None,
            used_at: None,
            created_at: now_secs,
        };
        cards.push(c.clone());
        created.push(c);
    }
    drop(cards);
    save_cards();
    created
}

/// 更新卡密
pub fn update_card(code: &str, enabled: Option<bool>, days: Option<i64>, description: Option<String>) -> Option<Card> {
    load_cards();
    let mut cards = CARDS.write();
    let card = cards.iter_mut().find(|c| c.code == code)?;
    if let Some(e) = enabled {
        card.enabled = e;
    }
    if let Some(d) = days {
        card.days = d;
    }
    if let Some(desc) = description {
        card.description = desc;
    }
    let updated = card.clone();
    drop(cards);
    save_cards();
    Some(updated)
}

/// 删除卡密
pub fn delete_card(code: &str) -> bool {
    load_cards();
    let mut cards = CARDS.write();
    let before = cards.len();
    cards.retain(|c| c.code != code);
    let removed = cards.len() != before;
    drop(cards);
    if removed {
        save_cards();
    }
    removed
}

/// 批量删除
pub fn delete_cards_batch(codes: &[&str]) -> usize {
    load_cards();
    let mut cards = CARDS.write();
    let before = cards.len();
    cards.retain(|c| !codes.contains(&c.code.as_str()));
    let removed = before - cards.len();
    drop(cards);
    if removed > 0 {
        save_cards();
    }
    removed
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn reset() {
        *USERS.write() = Vec::new();
        *CARDS.write() = Vec::new();
        // 清理磁盘文件，避免测试间状态泄漏
        let _ = fs::remove_file(users_file());
        let _ = fs::remove_file(cards_file());
    }

    #[test]
    #[serial(user_store)]
    fn register_user_validates_username() {
        reset();
        let r = register_user("ab", "pass1234", "CARD");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("长度"));
    }

    #[test]
    #[serial(user_store)]
    fn register_user_validates_username_chars() {
        reset();
        let r = register_user("ab-cd", "pass1234", "CARD");
        assert!(r.is_err());
    }

    #[test]
    #[serial(user_store)]
    fn register_user_card_not_found() {
        reset();
        let r = register_user("alice", "Pass1234", "NOPE");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("卡密"));
    }

    #[test]
    #[serial(user_store)]
    fn register_user_quota_card_rejected() {
        reset();
        let c = create_card("quota", 10, "quota");
        let r = register_user("alice", "Pass1234", &c.code);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("时间卡密"));
    }

    #[test]
    #[serial(user_store)]
    fn register_user_success() {
        reset();
        let c = create_card("test", 30, "time");
        let r = register_user("alice", "Pass1234", &c.code);
        assert!(r.is_ok(), "error = {:?}", r.err());
        assert_eq!(r.as_ref().unwrap().account_limit, 2);
        let users = get_all_users();
        assert_eq!(users.len(), 1);
    }

    #[test]
    #[serial(user_store)]
    fn register_user_duplicate_name() {
        reset();
        let c1 = create_card("c1", 30, "time");
        register_user("alice", "Pass1234", &c1.code).ok();
        let c2 = create_card("c2", 30, "time");
        let r = register_user("alice", "Pass1234", &c2.code);
        assert!(r.is_err());
    }

    #[test]
    #[serial(user_store)]
    fn register_user_card_already_used() {
        reset();
        let c = create_card("c", 30, "time");
        register_user("alice", "Pass1234", &c.code).ok();
        let r = register_user("bob", "Pass1234", &c.code);
        assert!(r.is_err());
    }

    #[test]
    #[serial(user_store)]
    fn validate_user_wrong_password() {
        reset();
        let c = create_card("c", 30, "time");
        register_user("alice", "Pass1234", &c.code).ok();
        let r = validate_user("alice", "WrongPass", "127.0.0.1");
        assert_eq!(r.error.as_deref(), Some("invalid_credentials"));
    }

    #[test]
    #[serial(user_store)]
    fn validate_user_success() {
        reset();
        let c = create_card("c", 30, "time");
        register_user("alice", "Pass1234", &c.code).ok();
        let r = validate_user("alice", "Pass1234", "127.0.0.1");
        assert!(r.error.is_none());
        assert_eq!(r.username.as_deref(), Some("alice"));
    }

    #[test]
    #[serial(user_store)]
    fn renew_user_quota() {
        reset();
        let c = create_card("c", 5, "time");
        register_user("alice", "Pass1234", &c.code).ok();
        let quota_card = create_card("q", 3, "quota");
        let r = renew_user("alice", &quota_card.code);
        assert!(r.is_ok(), "error = {:?}", r.err());
        assert_eq!(r.as_ref().unwrap().account_limit, Some(5));
    }

    #[test]
    #[serial(user_store)]
    fn renew_user_time_extends() {
        reset();
        let c1 = create_card("c1", 30, "time");
        register_user("alice", "Pass1234", &c1.code).ok();
        let c2 = create_card("c2", 7, "time");
        let r = renew_user("alice", &c2.code).unwrap();
        assert_eq!(r.card.unwrap().days, 37);
    }

    #[test]
    #[serial(user_store)]
    fn renew_user_time_permanent() {
        reset();
        let c1 = create_card("c1", 30, "time");
        register_user("alice", "Pass1234", &c1.code).ok();
        let c2 = create_card("c2", -1, "time");
        let r = renew_user("alice", &c2.code).unwrap();
        let card = r.card.unwrap();
        assert_eq!(card.days, -1);
        assert!(card.expires_at.is_none());
    }

    #[test]
    #[serial(user_store)]
    fn test_delete_user() {
        reset();
        let c1 = create_card("a", 30, "time");
        let c2 = create_card("b", 30, "time");
        register_user("alice", "Pass1234", &c1.code).ok();
        register_user("bob", "Pass1234", &c2.code).ok();
        assert_eq!(get_all_users().len(), 2);
        assert!(super::delete_user("alice"));
        assert_eq!(get_all_users().len(), 1);
    }

    #[test]
    #[serial(user_store)]
    fn test_change_password() {
        reset();
        let c = create_card("c", 30, "time");
        register_user("alice", "Pass1234", &c.code).ok();
        assert!(super::change_password("alice", "Pass1234", "NewPass56").is_ok());
        // 旧密码失败
        let r = validate_user("alice", "Pass1234", "127.0.0.1");
        assert!(r.error.is_some());
        // 新密码成功
        let r = validate_user("alice", "NewPass56", "127.0.0.1");
        assert!(r.error.is_none());
    }

    #[test]
    #[serial(user_store)]
    fn create_and_delete_card() {
        reset();
        let c = create_card("test", 30, "time");
        assert!(!c.code.is_empty());
        assert_eq!(c.code.len(), 16);
        assert!(delete_card(&c.code));
        assert!(!delete_card(&c.code));
    }

    #[test]
    #[serial(user_store)]
    fn create_cards_batch_limit() {
        reset();
        let cards = create_cards_batch("batch", 7, 50, "time");
        assert_eq!(cards.len(), 50);
        assert!(cards.iter().all(|c| c.days == 7));
    }

    #[test]
    #[serial(user_store)]
    fn update_card_fields() {
        reset();
        let c = create_card("test", 30, "time");
        let updated = update_card(&c.code, Some(false), Some(60), Some("new desc".to_string()));
        let u = updated.expect("update");
        assert!(!u.enabled);
        assert_eq!(u.days, 60);
        assert_eq!(u.description, "new desc");
    }

    #[test]
    #[serial(user_store)]
    fn test_delete_cards_batch() {
        reset();
        let c1 = create_card("a", 30, "time");
        let c2 = create_card("b", 30, "time");
        let c3 = create_card("c", 30, "time");
        let n = super::delete_cards_batch(&[&c1.code, &c2.code]);
        assert_eq!(n, 2);
        assert_eq!(get_all_cards().len(), 1);
    }
}
