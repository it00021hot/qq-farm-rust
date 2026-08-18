//! 好友 / 植物黑名单与进入农场错误处理。

use std::collections::HashMap;

use parking_lot::Mutex as PMutex;

use crate::constants::INVALID_KNOWN_FRIEND_GID_COOLDOWN_MS;

use super::now_ms;

// ============ 错误检测（与原 TS 1:1 翻译） ============

/// 检测"进入农场被封"错误（code 1002003）
#[must_use]
pub fn is_enter_farm_banned_error(error_message: &str) -> bool {
    error_message.contains("1002003")
}

/// 从错误消息中解析 RPC 错误码
#[must_use]
pub fn parse_rpc_error_code(error_message: &str) -> i32 {
    if let Some(start) = error_message.find("code=") {
        let rest = &error_message[start + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().unwrap_or(0)
    } else {
        0
    }
}

/// 检测瞬态网络错误（用于重试判断）
#[must_use]
pub fn is_transient_network_error(error_message: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "连接未打开",
        "请求超时",
        "request timeout",
        "请求已中断",
        "连接关闭",
        "连接已在加密途中关闭",
        "worker exited",
    ];
    KEYWORDS.iter().any(|k| error_message.contains(k))
}

// ============ 黑名单管理（仅 per-account store，无进程级 gid 表） ============

/// 加入好友黑名单（持久化到账号配置，对齐 bot `postToMaster(friend_blacklist_add)`）
pub fn add_friend_to_blacklist(
    account_id: &str,
    friend_gid: i64,
    friend_name: &str,
    reason: &str,
) -> bool {
    if friend_gid == 0 || account_id.is_empty() {
        return false;
    }
    let added =
        crate::models::store::account_config::add_friend_to_blacklist(account_id, friend_gid);
    if !added {
        return false;
    }
    tracing::warn!(
        friend_gid,
        friend_name = %friend_name,
        reason = %reason,
        account_id = %account_id,
        "好友已加入黑名单"
    );
    true
}

/// 移除黑名单（账号配置落盘）
pub fn remove_from_blacklist(account_id: &str, friend_gid: i64) -> bool {
    if account_id.is_empty() || friend_gid <= 0 {
        return false;
    }
    let current = crate::models::store::account_config::get_friend_blacklist(Some(account_id));
    if !current.contains(&friend_gid) {
        return false;
    }
    let next: Vec<i64> = current.into_iter().filter(|g| *g != friend_gid).collect();
    crate::models::store::account_config::set_friend_blacklist(account_id, next);
    true
}

/// 是否在该账号黑名单
#[must_use]
pub fn is_in_blacklist(account_id: &str, friend_gid: i64) -> bool {
    is_friend_blacklisted(account_id, friend_gid)
}

/// 是否在账号黑名单（配置落盘源）
#[must_use]
pub fn is_friend_blacklisted(account_id: &str, friend_gid: i64) -> bool {
    if friend_gid <= 0 || account_id.is_empty() {
        return false;
    }
    crate::models::store::account_config::get_friend_blacklist(Some(account_id))
        .contains(&friend_gid)
}

/// 黑名单大小
#[must_use]
pub fn blacklist_size(account_id: &str) -> usize {
    if account_id.is_empty() {
        return 0;
    }
    crate::models::store::account_config::get_friend_blacklist(Some(account_id)).len()
}

fn invalid_known_friend_gid_cooldown() -> &'static PMutex<HashMap<i64, u64>> {
    use std::sync::OnceLock;
    static MAP: OnceLock<PMutex<HashMap<i64, u64>>> = OnceLock::new();
    MAP.get_or_init(|| PMutex::new(HashMap::new()))
}

/// 标记失效 known GID（24h 冷却，对齐 bot `markKnownFriendGidInvalid`）
pub fn mark_known_friend_gid_invalid(friend_gid: i64) {
    if friend_gid <= 0 {
        return;
    }
    let until = now_ms().saturating_add(INVALID_KNOWN_FRIEND_GID_COOLDOWN_MS);
    invalid_known_friend_gid_cooldown().lock().insert(friend_gid, until);
}

/// 是否在失效冷却期
#[must_use]
pub fn is_known_friend_gid_invalid(friend_gid: i64) -> bool {
    let now = now_ms();
    let mut map = invalid_known_friend_gid_cooldown().lock();
    map.retain(|_, until| *until > now);
    map.contains_key(&friend_gid)
}

/// 移除失效好友 GID（对齐 bot `removeKnownFriendGid`）
pub fn remove_invalid_known_friend_gid(
    account_id: &str,
    friend_gid: i64,
    friend_name: &str,
    reason: &str,
) -> bool {
    if friend_gid <= 0 {
        return false;
    }
    mark_known_friend_gid_invalid(friend_gid);
    if !account_id.is_empty() {
        crate::models::store::account_config::remove_known_friend_gid(account_id, friend_gid);
    }
    tracing::warn!(
        friend_gid,
        friend_name = %friend_name,
        reason = %reason,
        "检测到失效好友 GID，已自动移除"
    );
    true
}

/// 检测"好友关系失效"错误（无效 / 不存在 / 删除 / 关系 / not found / invalid）
#[must_use]
pub fn is_invalid_friend_access_error(error_message: &str) -> bool {
    if error_message.is_empty() {
        return false;
    }
    if is_enter_farm_banned_error(error_message) || is_transient_network_error(error_message) {
        return false;
    }
    let lower = error_message.to_lowercase();
    let has_keyword =
        ["无效", "不存在", "删除", "关系", "not found", "invalid", "not friend", "friend"]
            .iter()
            .any(|k| lower.contains(&k.to_lowercase()));
    has_keyword && parse_rpc_error_code(error_message) > 0
}

/// 进入好友农场错误分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendEnterErrorKind {
    /// 已加入黑名单
    Blacklist,
    /// 关系失效
    InvalidRemoved,
    /// 普通错误（未处理）
    Error,
}

/// 处理"进入好友农场"错误
///
/// 返回 `{ handled, kind }`：`blacklist` / `invalid_removed` / `error`
#[must_use]
pub fn handle_friend_enter_error(
    account_id: &str,
    friend_gid: i64,
    friend_name: &str,
    error_message: &str,
) -> FriendEnterErrorKind {
    if is_enter_farm_banned_error(error_message) {
        add_friend_to_blacklist(account_id, friend_gid, friend_name, error_message);
        return FriendEnterErrorKind::Blacklist;
    }
    if is_invalid_friend_access_error(error_message) {
        remove_invalid_known_friend_gid(account_id, friend_gid, friend_name, error_message);
        return FriendEnterErrorKind::InvalidRemoved;
    }
    FriendEnterErrorKind::Error
}

/// 植物黑名单（按 account_id 隔离）
pub fn plant_blacklist() -> &'static PMutex<std::collections::HashMap<String, Vec<i64>>> {
    use std::sync::OnceLock;
    static MAP: OnceLock<PMutex<std::collections::HashMap<String, Vec<i64>>>> = OnceLock::new();
    MAP.get_or_init(|| PMutex::new(std::collections::HashMap::new()))
}

/// 设置植物黑名单（内存镜像 + 账号配置落盘）
pub fn set_plant_blacklist(account_id: &str, seeds: Vec<i64>) {
    plant_blacklist().lock().insert(account_id.to_string(), seeds.clone());
    if !account_id.is_empty() {
        let _ = crate::models::store::account_config::set_plant_blacklist(account_id, seeds);
    }
}

/// 获取植物黑名单（账号配置落盘源）
#[must_use]
pub fn get_plant_blacklist(account_id: &str) -> Vec<i64> {
    if account_id.is_empty() {
        return plant_blacklist().lock().get(account_id).cloned().unwrap_or_default();
    }
    crate::models::store::account_config::get_plant_blacklist(Some(account_id))
}

/// 好友黑名单（按 account_id 隔离）—— 兼容旧内存表；生产路径读账号配置
pub fn account_friend_blacklist() -> &'static PMutex<std::collections::HashMap<String, Vec<i64>>> {
    use std::sync::OnceLock;
    static MAP: OnceLock<PMutex<std::collections::HashMap<String, Vec<i64>>>> = OnceLock::new();
    MAP.get_or_init(|| PMutex::new(std::collections::HashMap::new()))
}

/// 设置好友黑名单（内存镜像 + 账号配置落盘）
pub fn set_account_friend_blacklist(account_id: &str, gids: Vec<i64>) {
    account_friend_blacklist().lock().insert(account_id.to_string(), gids.clone());
    if !account_id.is_empty() {
        let _ = crate::models::store::account_config::set_friend_blacklist(account_id, gids);
    }
}

/// 获取好友黑名单（账号配置落盘源）
#[must_use]
pub fn get_account_friend_blacklist(account_id: &str) -> Vec<i64> {
    if account_id.is_empty() {
        return account_friend_blacklist().lock().get(account_id).cloned().unwrap_or_default();
    }
    crate::models::store::account_config::get_friend_blacklist(Some(account_id))
}
