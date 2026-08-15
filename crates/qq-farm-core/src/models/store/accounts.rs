//! 账号列表 CRUD。
//!
//! 1:1 翻译原 `core/src/models/store/accounts.ts`（124 行）。
//!
//! 数据存储：`data/accounts.json`（`AccountsData` 格式）。
//!
//! ## 数据结构
//!
//! ```json
//! {
//!   "accounts": [...],
//!   "nextId": 1
//! }
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::paths::{ensure_data_dir, get_data_file};

/// 数据文件
#[must_use]
pub fn accounts_file() -> PathBuf {
    get_data_file("accounts.json")
}

/// 账号持久化记录（store 形态，与 runtime [`AccountSession`](crate::models::account::AccountSession) 分离）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    /// 账号唯一 ID
    pub id: String,
    /// 显示名
    pub name: String,
    /// 游戏内昵称
    #[serde(default)]
    pub nick: String,
    /// 登录 code
    pub code: String,
    /// 平台 (qq/wx)
    pub platform: String,
    /// 内部 uin
    pub uin: String,
    /// QQ 号
    pub qq: String,
    /// 头像 URL
    pub avatar: String,
    /// 真实用户名（用于通知等）
    pub username: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 兼容旧名；请改用 [`AccountRecord`]。
#[deprecated(note = "use AccountRecord")]
pub type Account = AccountRecord;

/// 账号数据文件结构
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountsData {
    pub accounts: Vec<AccountRecord>,
    #[serde(rename = "nextId", alias = "next_id")]
    pub next_id: i64,
}

/// 全局状态：账号数据 + 文件路径
///
/// 由 `set_accounts_data` / `accounts_data` 读写。文件持久化在 controllers 层
/// （2A）注入，本模块只做内存层。
static ACCOUNTS: once_cell::sync::Lazy<parking_lot::RwLock<AccountsData>> =
    once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(AccountsData::default()));

/// 获取当前账号数据快照
#[must_use]
pub fn accounts_data() -> AccountsData {
    ACCOUNTS.read().clone()
}

/// 替换账号数据
pub fn set_accounts_data(data: AccountsData) {
    *ACCOUNTS.write() = data;
}

/// 全部账号
#[must_use]
pub fn get_accounts() -> Vec<AccountRecord> {
    ACCOUNTS.read().accounts.clone()
}

/// 按用户聚合账号
#[must_use]
pub fn get_accounts_by_user() -> HashMap<String, Vec<AccountRecord>> {
    let mut out: HashMap<String, Vec<AccountRecord>> = HashMap::new();
    for a in ACCOUNTS.read().accounts.iter() {
        out.entry(a.username.clone()).or_default().push(a.clone());
    }
    out
}

/// 添加或更新账号
pub fn add_or_update_account(acc: AccountRecord) -> AccountRecord {
    let mut guard = ACCOUNTS.write();
    let now = crate::utils::time::now_secs();
    let mut acc = acc;
    if acc.created_at == 0 {
        acc.created_at = now;
    }
    acc.updated_at = now;

    if let Some(existing) = guard.accounts.iter_mut().find(|a| a.id == acc.id) {
        let created = existing.created_at;
        *existing = acc.clone();
        existing.created_at = created;
        return existing.clone();
    }

    // 新账号
    if acc.id.is_empty() {
        let nid = guard.next_id.max(1);
        acc.id = nid.to_string();
        guard.next_id = nid + 1;
    } else if let Ok(n) = acc.id.parse::<i64>() {
        if n >= guard.next_id {
            guard.next_id = n + 1;
        }
    }
    guard.accounts.push(acc.clone());
    acc
}

/// 删除账号
pub fn delete_account(id: &str) -> bool {
    let mut guard = ACCOUNTS.write();
    let before = guard.accounts.len();
    guard.accounts.retain(|a| a.id != id);
    guard.accounts.len() != before
}

/// 删除某用户的所有账号
pub fn delete_accounts_by_user(username: &str) -> usize {
    let mut guard = ACCOUNTS.write();
    let before = guard.accounts.len();
    guard.accounts.retain(|a| a.username != username);
    before - guard.accounts.len()
}

/// 删除某用户的所有账号 + 关联配置
pub fn delete_user_config(username: &str) -> usize {
    delete_accounts_by_user(username)
}

/// 从文件加载
pub fn load_from_file() -> std::io::Result<AccountsData> {
    let path = accounts_file();
    if !path.exists() {
        return Ok(AccountsData::default());
    }
    let raw = fs::read_to_string(&path)?;
    let data: AccountsData = serde_json::from_str(&raw).unwrap_or_default();
    Ok(data)
}

/// 保存到文件（原子写）
pub fn save_to_file(data: &AccountsData) -> std::io::Result<()> {
    ensure_data_dir()?;
    let path = accounts_file();
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(data).map_err(std::io::Error::other)?;
    fs::write(&tmp, body)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// 加载文件到全局状态
pub fn load_into_global() -> std::io::Result<usize> {
    let data = load_from_file()?;
    let n = data.accounts.len();
    set_accounts_data(data);
    Ok(n)
}

/// 把当前内存账号列表写回 `accounts.json`。失败只打日志，不回滚内存。
pub fn persist_global() {
    let data = accounts_data();
    if let Err(e) = save_to_file(&data) {
        tracing::error!(error = %e, "failed to persist accounts.json");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn make_account(id: &str, name: &str, username: &str) -> AccountRecord {
        AccountRecord {
            id: id.to_string(),
            name: name.to_string(),
            code: "code123".to_string(),
            platform: "qq".to_string(),
            uin: "u123".to_string(),
            qq: "12345".to_string(),
            avatar: String::new(),
            username: username.to_string(),
            nick: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn reset() {
        set_accounts_data(AccountsData::default());
    }

    #[test]
    #[serial(accounts)]
    fn add_new_account_assigns_id() {
        reset();
        let acc = add_or_update_account(make_account("", "test1", "user1"));
        assert_eq!(acc.id, "1");
        assert_eq!(accounts_data().next_id, 2);
        assert_eq!(accounts_data().accounts.len(), 1);
    }

    #[test]
    #[serial(accounts)]
    fn update_existing_keeps_created_at() {
        reset();
        let a1 = add_or_update_account(make_account("1", "test1", "user1"));
        let original_created = a1.created_at;

        let mut a2 = make_account("1", "test1-renamed", "user1");
        a2.created_at = 0;
        let updated = add_or_update_account(a2);
        assert_eq!(updated.id, "1");
        assert_eq!(updated.name, "test1-renamed");
        assert_eq!(updated.created_at, original_created);
    }

    #[test]
    #[serial(accounts)]
    fn test_delete_account() {
        reset();
        add_or_update_account(make_account("1", "a1", "u1"));
        add_or_update_account(make_account("2", "a2", "u1"));
        assert_eq!(get_accounts().len(), 2);
        assert!(super::delete_account("1"));
        assert_eq!(get_accounts().len(), 1);
        assert!(!super::delete_account("1"));
    }

    #[test]
    #[serial(accounts)]
    fn delete_by_user() {
        reset();
        add_or_update_account(make_account("1", "a1", "u1"));
        add_or_update_account(make_account("2", "a2", "u1"));
        add_or_update_account(make_account("3", "a3", "u2"));
        let deleted = delete_accounts_by_user("u1");
        assert_eq!(deleted, 2);
        assert_eq!(get_accounts().len(), 1);
    }

    #[test]
    #[serial(accounts)]
    fn get_accounts_by_user_groups() {
        reset();
        add_or_update_account(make_account("1", "a1", "u1"));
        add_or_update_account(make_account("2", "a2", "u1"));
        add_or_update_account(make_account("3", "a3", "u2"));
        let m = get_accounts_by_user();
        assert_eq!(m.get("u1").unwrap().len(), 2);
        assert_eq!(m.get("u2").unwrap().len(), 1);
        assert!(m.get("u3").is_none());
    }

    #[test]
    #[serial(accounts)]
    fn file_save_load_roundtrip() {
        reset();
        add_or_update_account(make_account("1", "a1", "u1"));
        let data = accounts_data();
        save_to_file(&data).expect("save");
        let loaded = load_from_file().expect("load");
        assert_eq!(loaded.accounts.len(), 1);
        assert_eq!(loaded.accounts[0].id, "1");
        // 清理
        let _ = fs::remove_file(accounts_file());
    }
}
