//! 自动化开关 — 业务 service 层用 `is_automation_on(category)` 判断某个功能是否启用。
//!
//! 默认所有 category 返回 `true`。runtime engine 可在配置变更时通过
//! [`set_automation_flag`] / [`set_all_automation_flags`] 覆盖单个 category 的开关。
//!
//! 当前实现是进程内全局开关：所有账号共享一份标志。
//! 真正的多账号隔离会在 runtime engine 层做（每账号独立 scheduler）。

use std::collections::HashMap;
use std::sync::RwLock;

/// 所有 category 的默认状态
const DEFAULT_ENABLED: bool = true;

static FLAGS: RwLock<Option<HashMap<String, bool>>> = RwLock::new(None);

fn flags() -> std::sync::RwLockReadGuard<'static, Option<HashMap<String, bool>>> {
    FLAGS.read().unwrap_or_else(|p| p.into_inner())
}

fn flags_mut() -> std::sync::RwLockWriteGuard<'static, Option<HashMap<String, bool>>> {
    FLAGS.write().unwrap_or_else(|p| p.into_inner())
}

/// 判断指定 category 的自动化开关是否启用
#[must_use]
pub fn is_automation_on(category: &str) -> bool {
    let guard = flags();
    match guard.as_ref() {
        Some(map) => map.get(category).copied().unwrap_or(DEFAULT_ENABLED),
        None => DEFAULT_ENABLED,
    }
}

/// 设置指定 category 的开关
pub fn set_automation_flag(category: &str, enabled: bool) {
    let mut guard = flags_mut();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(category.to_string(), enabled);
}

/// 批量设置开关
pub fn set_all_automation_flags<I, S>(entries: I)
where
    I: IntoIterator<Item = (S, bool)>,
    S: Into<String>,
{
    let mut guard = flags_mut();
    let map = guard.get_or_insert_with(HashMap::new);
    for (k, v) in entries {
        map.insert(k.into(), v);
    }
}

/// 清除所有自定义开关（恢复默认）
pub fn clear_automation_flags() {
    let mut guard = flags_mut();
    *guard = None;
}

/// 获取当前所有已知 category 的开关（合并默认）
#[must_use]
pub fn current_automation_flags() -> std::collections::HashMap<String, bool> {
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
        let v = match guard.as_ref() {
            Some(map) => map.get(k).copied().unwrap_or(DEFAULT_ENABLED),
            None => DEFAULT_ENABLED,
        };
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

    #[test]
    #[serial]
    fn default_returns_true() {
        clear_automation_flags();
        assert!(is_automation_on("any_category"));
    }

    #[test]
    #[serial]
    fn set_and_read() {
        clear_automation_flags();
        set_automation_flag("test_cat", false);
        assert!(!is_automation_on("test_cat"));
        set_automation_flag("test_cat", true);
        assert!(is_automation_on("test_cat"));
        clear_automation_flags();
    }

    #[test]
    #[serial]
    fn bulk_set() {
        clear_automation_flags();
        set_all_automation_flags(vec![("a", false), ("b", true)]);
        assert!(!is_automation_on("a"));
        assert!(is_automation_on("b"));
        clear_automation_flags();
    }

    #[test]
    #[serial]
    fn unknown_category_defaults() {
        clear_automation_flags();
        set_automation_flag("known", false);
        // 未设置的 category 走默认
        assert!(is_automation_on("unknown"));
        clear_automation_flags();
    }

    #[test]
    fn category_constants() {
        assert_eq!(category::TASK, "task");
        assert_eq!(category::FARM, "farm");
        assert_eq!(category::FRIEND, "friend");
    }
}
