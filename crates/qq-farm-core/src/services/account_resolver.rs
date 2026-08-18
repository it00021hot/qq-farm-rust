//! 账号 ref 解析（1:1 对应原 `services/account-resolver.ts` 47 行）。
//!
//! 提供：
//! - `normalize_account_ref(raw_ref)`：把任意输入（可能是数组/字符串）归一化为字符串
//! - `build_account_keys(account)`：用 id/uin/qq 三个字段构造可匹配 set
//! - `find_account_by_ref(accounts, raw_ref)`：在账号列表中按多 key 匹配
//! - `resolve_account_id(accounts, raw_ref)`：find + 归一化的组合

use crate::models::AccountSession;

/// 把任意输入归一化为字符串
///
/// 处理：
/// - null/undefined → ""
/// - 数组 → 取第一个元素再递归
/// - 其他 → trim 后 String
pub fn normalize_account_ref(raw_ref: Option<&serde_json::Value>) -> String {
    let Some(v) = raw_ref else {
        return String::new();
    };
    if v.is_null() {
        return String::new();
    }
    if let Some(arr) = v.as_array() {
        return arr.first().and_then(|x| normalize_account_ref(Some(x)).into()).unwrap_or_default();
    }
    v.as_str().map(|s| s.trim().to_string()).unwrap_or_else(|| v.to_string().trim().to_string())
}

/// 构造账号的 key 集合（id/uin/qq 都算"自身"）
pub fn build_account_keys(account: &AccountSession) -> std::collections::HashSet<String> {
    let mut keys = std::collections::HashSet::new();
    for v in [&account.id, &account.uin, &account.qq] {
        let s = v.trim();
        if !s.is_empty() {
            keys.insert(s.to_string());
        }
    }
    keys
}

/// 在账号列表中按多 key 匹配
pub fn find_account_by_ref(
    accounts: &[AccountSession],
    raw_ref: Option<&serde_json::Value>,
) -> Option<AccountSession> {
    let key = normalize_account_ref(raw_ref);
    if key.is_empty() {
        return None;
    }
    for account in accounts {
        if build_account_keys(account).contains(&key) {
            return Some(account.clone());
        }
    }
    None
}

/// 组合：find + 归一化返回 id
pub fn resolve_account_id(
    accounts: &[AccountSession],
    raw_ref: Option<&serde_json::Value>,
) -> String {
    find_account_by_ref(accounts, raw_ref)
        .map(|a| normalize_account_ref(Some(&serde_json::Value::String(a.id.clone()))))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AccountSession;

    fn acc_with(id: &str, uin: &str, qq: &str) -> AccountSession {
        let mut a =
            AccountSession::new(id.to_string(), "openid-1".to_string(), "name-1".to_string());
        a.uin = uin.to_string();
        a.qq = qq.to_string();
        a
    }

    #[test]
    fn normalize_handles_null_and_array() {
        assert_eq!(normalize_account_ref(None), "");
        assert_eq!(normalize_account_ref(Some(&serde_json::Value::Null)), "");
        let arr = serde_json::json!(["acc1", "acc2"]);
        assert_eq!(normalize_account_ref(Some(&arr)), "acc1");
    }

    #[test]
    fn normalize_trims_string() {
        let v = serde_json::json!("  acc1  ");
        assert_eq!(normalize_account_ref(Some(&v)), "acc1");
    }

    #[test]
    fn build_account_keys_includes_all_fields() {
        let a = acc_with("id-1", "uin-1", "qq-1");
        let keys = build_account_keys(&a);
        assert!(keys.contains("id-1"));
        assert!(keys.contains("uin-1"));
        assert!(keys.contains("qq-1"));
    }

    #[test]
    fn find_by_id() {
        let a = acc_with("id-1", "u1", "q1");
        let b = acc_with("id-2", "u2", "q2");
        let accounts = vec![a.clone(), b.clone()];
        let found = find_account_by_ref(&accounts, Some(&serde_json::json!("id-1"))).unwrap();
        assert_eq!(found.id, "id-1");
    }

    #[test]
    fn find_by_uin() {
        let a = acc_with("id-1", "u1", "q1");
        let b = acc_with("id-2", "u2", "q2");
        let accounts = vec![a.clone(), b.clone()];
        let found = find_account_by_ref(&accounts, Some(&serde_json::json!("u2"))).unwrap();
        assert_eq!(found.id, "id-2");
    }

    #[test]
    fn find_by_qq() {
        let a = acc_with("id-1", "u1", "q1");
        let accounts = vec![a.clone()];
        let found = find_account_by_ref(&accounts, Some(&serde_json::json!("q1"))).unwrap();
        assert_eq!(found.id, "id-1");
    }

    #[test]
    fn find_returns_none_for_empty_key() {
        let a = acc_with("id-1", "u1", "q1");
        let found = find_account_by_ref(&[a], Some(&serde_json::json!("")));
        assert!(found.is_none());
    }

    #[test]
    fn find_returns_none_when_not_found() {
        let a = acc_with("id-1", "u1", "q1");
        let found = find_account_by_ref(&[a], Some(&serde_json::json!("unknown")));
        assert!(found.is_none());
    }

    #[test]
    fn resolve_account_id_returns_normalized_id() {
        let a = acc_with("id-1", "u1", "q1");
        let accounts = vec![a];
        let id = resolve_account_id(&accounts, Some(&serde_json::json!("q1")));
        assert_eq!(id, "id-1");
    }

    #[test]
    fn resolve_account_id_returns_empty_when_missing() {
        let a = acc_with("id-1", "u1", "q1");
        let id = resolve_account_id(&[a], Some(&serde_json::json!("unknown")));
        assert_eq!(id, "");
    }
}
