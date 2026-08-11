//! QR Login 工具。
//!
//! 1:1 翻译原 `core/src/utils/qrutils.ts`（`CookieUtils` + `HashUtils`）。

// =====================================================================
// CookieUtils
// =====================================================================

/// Cookie 工具（取 key=value、提取 uin）
pub struct CookieUtils;

impl CookieUtils {
    /// 从 cookie 字符串或字符串数组中取 `key` 的值
    #[must_use]
    pub fn get_value(cookies: Option<&str>, key: &str) -> Option<String> {
        let cookies = cookies?;
        // regex: (^|;\s*)key=value
        let needle = format!("{key}=");
        for part in cookies.split(';') {
            let part = part.trim();
            if let Some(rest) = part.strip_prefix(&needle) {
                return Some(rest.to_string());
            }
        }
        None
    }

    /// 取 uin（按 wxuin / uin / ptui_loginuin 顺序，去前缀 `o0*`）
    #[must_use]
    pub fn get_uin(cookies: Option<&str>) -> Option<String> {
        let uin = Self::get_value(cookies, "wxuin")
            .or_else(|| Self::get_value(cookies, "uin"))
            .or_else(|| Self::get_value(cookies, "ptui_loginuin"))?;
        // 去掉前缀 o + 零
        let trimmed = uin.trim_start_matches('o').trim_start_matches('0');
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

// =====================================================================
// HashUtils
// =====================================================================

/// Hash 工具（Java 风格字符串 hash）
pub struct HashUtils;

impl HashUtils {
    /// 经典 Java `String.hashCode()` 算法：`hash = hash * 31 + c`
    #[must_use]
    pub fn hash(s: &str) -> i32 {
        let mut hash: i64 = 0;
        for c in s.chars() {
            hash = hash.wrapping_mul(31).wrapping_add(c as i64);
        }
        (hash & 0x7FFFFFFF) as i32
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_get_value_basic() {
        let c = "wxuin=o1234567890; pt2_token=abc; pgv_pvid=999";
        assert_eq!(
            CookieUtils::get_value(Some(c), "wxuin"),
            Some("o1234567890".to_string())
        );
        assert_eq!(
            CookieUtils::get_value(Some(c), "pt2_token"),
            Some("abc".to_string())
        );
        assert_eq!(CookieUtils::get_value(Some(c), "nonexistent"), None);
    }

    #[test]
    fn cookie_get_value_none() {
        assert_eq!(CookieUtils::get_value(None, "x"), None);
        assert_eq!(CookieUtils::get_value(Some(""), "x"), None);
    }

    #[test]
    fn cookie_get_uin_strip_prefix() {
        let c = "wxuin=o0123456789; ptui_loginuin=o0987654321";
        assert_eq!(CookieUtils::get_uin(Some(c)), Some("123456789".to_string()));
        let c2 = "uin=12345";
        assert_eq!(CookieUtils::get_uin(Some(c2)), Some("12345".to_string()));
    }

    #[test]
    fn cookie_get_uin_fallback_order() {
        // wxuin > uin > ptui_loginuin
        let c = "uin=111; wxuin=o0222";
        assert_eq!(CookieUtils::get_uin(Some(c)), Some("222".to_string()));
        let c2 = "ptui_loginuin=o0333";
        assert_eq!(CookieUtils::get_uin(Some(c2)), Some("333".to_string()));
        // 只有 ptui_loginuin
        let c3 = "ptui_loginuin=o0444";
        assert_eq!(CookieUtils::get_uin(Some(c3)), Some("444".to_string()));
    }

    #[test]
    fn cookie_get_uin_missing() {
        assert_eq!(CookieUtils::get_uin(Some("foo=bar")), None);
        assert_eq!(CookieUtils::get_uin(None), None);
    }

    #[test]
    fn hash_java_string_basic() {
        // Java "abc".hashCode() = 96354
        assert_eq!(HashUtils::hash("abc"), 96_354);
        // Java "".hashCode() = 0
        assert_eq!(HashUtils::hash(""), 0);
        // Java "hello".hashCode() = 99162322
        assert_eq!(HashUtils::hash("hello"), 99_162_322);
    }

    #[test]
    fn hash_deterministic() {
        assert_eq!(HashUtils::hash("test"), HashUtils::hash("test"));
    }
}
