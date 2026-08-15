//! 自动化开关 — 业务 service 层用 `is_automation_on_for(account_id, category)` 判断。
//!
//! 默认所有 category 返回 `true`。runtime / 测试可通过
//! [`set_automation_flag`] / [`set_all_automation_flags`] 按账号覆盖单个 category。
//!
//! 进程内 FLAGS 按 `account_id` 隔离；未覆盖时读 `AccountConfig.automation`。

use std::collections::HashMap;
use std::sync::RwLock;

/// 所有 category 的默认状态
const DEFAULT_ENABLED: bool = true;

/// `account_id` → (`category` → enabled)
static FLAGS: RwLock<Option<HashMap<String, HashMap<String, bool>>>> = RwLock::new(None);

fn flags() -> std::sync::RwLockReadGuard<'static, Option<HashMap<String, HashMap<String, bool>>>>
{
    FLAGS.read().unwrap_or_else(|p| p.into_inner())
}

fn flags_mut(
) -> std::sync::RwLockWriteGuard<'static, Option<HashMap<String, HashMap<String, bool>>>> {
    FLAGS.write().unwrap_or_else(|p| p.into_inner())
}

fn account_flags(account_id: &str) -> Option<HashMap<String, bool>> {
    flags()
        .as_ref()
        .and_then(|all| all.get(account_id).cloned())
}

fn account_flag(account_id: &str, category: &str) -> Option<bool> {
    account_flags(account_id).and_then(|m| m.get(category).copied())
}

/// 判断指定 category 的自动化开关是否启用（无账号上下文时的全局 shim）
#[must_use]
pub fn is_automation_on(category: &str) -> bool {
    is_automation_on_for("", category)
}

/// 按账号读 `AccountConfig.automation`（对齐 TS worker 进程内 `isAutomationOn`）。
///
/// 进程内 FLAGS 仍可覆盖（测试 / 面板临时开关）；未覆盖时读该账号配置。
#[must_use]
pub fn is_automation_on_for(account_id: &str, category: &str) -> bool {
    if let Some(v) = account_flag(account_id, category) {
        return v;
    }
    if account_id.is_empty() {
        return DEFAULT_ENABLED;
    }
    let a = crate::models::store::account_config::get_automation(Some(account_id));
    match category {
        "farm" => a.farm,
        "farm_push" => a.farm_push,
        "land_upgrade" => a.land_upgrade,
        "friend" => a.friend,
        "friend_help" => a.friend_help,
        "friend_help_exp_limit" => a.friend_help_exp_limit,
        "friend_steal" => a.friend_steal,
        "friend_steal_activity_only" => a.friend_steal_activity_only,
        "friend_bad" => a.friend_bad,
        "task" => a.task,
        "fertilizer_gift" => a.fertilizer_gift,
        "fertilizer_buy_organic" => a.fertilizer_buy_organic,
        "fertilizer_buy_normal" => a.fertilizer_buy_normal,
        "sell" => a.sell,
        "fertilizer_multi_season" => a.fertilizer_multi_season,
        "skip_own_weed_bug" => a.skip_own_weed_bug,
        _ => DEFAULT_ENABLED,
    }
}

/// 设置指定账号 + category 的开关
pub fn set_automation_flag(account_id: &str, category: &str, enabled: bool) {
    let mut guard = flags_mut();
    let all = guard.get_or_insert_with(HashMap::new);
    all.entry(account_id.to_string())
        .or_default()
        .insert(category.to_string(), enabled);
}

/// 批量设置某账号的开关
pub fn set_all_automation_flags<I, S>(account_id: &str, entries: I)
where
    I: IntoIterator<Item = (S, bool)>,
    S: Into<String>,
{
    let mut guard = flags_mut();
    let all = guard.get_or_insert_with(HashMap::new);
    let map = all.entry(account_id.to_string()).or_default();
    for (k, v) in entries {
        map.insert(k.into(), v);
    }
}

/// 清除所有自定义开关（恢复默认）
pub fn clear_automation_flags() {
    let mut guard = flags_mut();
    *guard = None;
}

/// 清除指定账号的自定义开关
pub fn clear_automation_flags_for(account_id: &str) {
    let mut guard = flags_mut();
    if let Some(all) = guard.as_mut() {
        all.remove(account_id);
        if all.is_empty() {
            *guard = None;
        }
    }
}

/// 获取指定账号当前所有已知 category 的开关（合并默认）
#[must_use]
pub fn current_automation_flags(account_id: &str) -> std::collections::HashMap<String, bool> {
    let guard = flags();
    let known: Vec<&str> = vec![
        category::TASK,
        category::FARM,
        category::FRIEND,
        category::EMAIL,
        category::SHARE,
        category::INTERACT,
        category::WAREHOUSE,
        category::MALL,
        category::MONTHCARD,
        category::QQVIP,
        category::GUIDE,
    ];
    let mut out = std::collections::HashMap::new();
    for k in known {
        let v = guard
            .as_ref()
            .and_then(|all| all.get(account_id))
            .and_then(|m| m.get(k).copied())
            .unwrap_or(DEFAULT_ENABLED);
        out.insert(k.to_string(), v);
    }
    out
}

/// 当前已知 category 集合
pub mod category {
    pub const TASK: &str = "task";
    pub const FARM: &str = "farm";
    pub const FRIEND: &str = "friend";
    pub const EMAIL: &str = "email";
    pub const SHARE: &str = "share";
    pub const INTERACT: &str = "interact";
    pub const WAREHOUSE: &str = "warehouse";
    pub const MALL: &str = "mall";
    pub const MONTHCARD: &str = "monthcard";
    pub const QQVIP: &str = "qqvip";
    pub const GUIDE: &str = "guide";
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    const TEST_ACCOUNT: &str = "test-automation-account";

    #[test]
    #[serial]
    fn default_returns_true() {
        clear_automation_flags();
        assert!(is_automation_on("any_category"));
        assert!(is_automation_on_for(TEST_ACCOUNT, "any_category"));
    }

    #[test]
    #[serial]
    fn set_and_read() {
        clear_automation_flags();
        set_automation_flag(TEST_ACCOUNT, "test_cat", false);
        assert!(!is_automation_on_for(TEST_ACCOUNT, "test_cat"));
        set_automation_flag(TEST_ACCOUNT, "test_cat", true);
        assert!(is_automation_on_for(TEST_ACCOUNT, "test_cat"));
        clear_automation_flags();
    }

    #[test]
    #[serial]
    fn accounts_are_isolated() {
        clear_automation_flags();
        set_automation_flag("acc-a", "test_cat", false);
        set_automation_flag("acc-b", "test_cat", true);
        assert!(!is_automation_on_for("acc-a", "test_cat"));
        assert!(is_automation_on_for("acc-b", "test_cat"));
        clear_automation_flags();
    }

    #[test]
    #[serial]
    fn bulk_set() {
        clear_automation_flags();
        set_all_automation_flags(
            TEST_ACCOUNT,
            vec![("a", false), ("b", true)],
        );
        assert!(!is_automation_on_for(TEST_ACCOUNT, "a"));
        assert!(is_automation_on_for(TEST_ACCOUNT, "b"));
        clear_automation_flags();
    }

    #[test]
    #[serial]
    fn unknown_category_defaults() {
        clear_automation_flags();
        set_automation_flag(TEST_ACCOUNT, "known", false);
        assert!(is_automation_on_for(TEST_ACCOUNT, "unknown"));
        clear_automation_flags();
    }

    #[test]
    fn category_constants() {
        assert_eq!(category::TASK, "task");
        assert_eq!(category::FARM, "farm");
        assert_eq!(category::FRIEND, "friend");
    }
}
