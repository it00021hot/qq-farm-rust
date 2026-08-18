//! 登录 URL 解析。
//!
//! 1:1 翻译原 `core/src/utils/login-url.ts`。
//!
//! 用法：从完整 WS/HTTP 登录 URL 或裸 code 中提取 code / client hints。

const FALLBACK_GATE_HOST: &str = "gate-obt.nqf.qq.com";

/// 客户端信息
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoginClientHints {
    pub platform: String,
    pub os: String,
    pub ver: String,
}

/// 标准化 login platform
#[must_use]
pub fn normalize_login_platform(platform: &str) -> &'static str {
    match platform.trim().to_ascii_lowercase().as_str() {
        "qq" => "qq",
        "wx" => "wx",
        _ => "",
    }
}

/// 推算 href（处理裸 path / 裸 query / 裸 code）
fn to_href(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    // 完整 URL
    if raw.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && raw.contains(':')
        && !raw.contains(' ')
    {
        return raw.to_string();
    }
    // path 形式
    if raw.starts_with('/') || raw.contains('?') {
        if raw.starts_with('/') {
            return format!("wss://{FALLBACK_GATE_HOST}{raw}");
        }
        return format!("wss://{FALLBACK_GATE_HOST}/?{raw}");
    }
    // 裸 query
    let has_query_param = raw.split(|c: char| c == '?' || c == '&').skip(1).any(|s| {
        let lower = s.to_ascii_lowercase();
        lower.starts_with("platform=")
            || lower.starts_with("os=")
            || lower.starts_with("ver=")
            || lower.starts_with("code=")
    });
    if has_query_param {
        let q = raw.trim_start_matches('?');
        return format!("wss://{FALLBACK_GATE_HOST}/prod/ws?{q}");
    }
    String::new()
}

fn decode_param(value: &str) -> String {
    let raw = value.trim();
    if raw.is_empty() {
        return String::new();
    }
    match percent_decode(raw) {
        Ok(s) => s,
        Err(_) => raw.to_string(),
    }
}

fn percent_decode(s: &str) -> Result<String, std::str::Utf8Error> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(a), Some(b)) = (h1, h2) {
                out.push((a * 16 + b) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).map_err(|e| e.utf8_error())
}

/// 从完整登录 URL 或裸 code 提取登录 code
#[must_use]
pub fn extract_code(raw_input: &str) -> String {
    let raw = raw_input.trim();
    if raw.is_empty() {
        return String::new();
    }

    // 尝试作 URL 解析
    if let Some(code) = extract_code_as_url(raw) {
        return code;
    }

    // 尝试 query 形式
    if let Some(captures) = query_param(raw, "code") {
        return decode_param(&captures);
    }

    // 裸字符串（无 / ? & = 空白）
    if !raw.chars().any(|c| c.is_whitespace() || c == '/' || c == '?' || c == '&' || c == '=') {
        return raw.to_string();
    }

    String::new()
}

fn extract_code_as_url(raw: &str) -> Option<String> {
    let href = to_href(raw);
    if href.is_empty() {
        return None;
    }
    // 取 query 中的 code=
    let code = query_param(&href, "code")?;
    Some(decode_param(&code))
}

fn query_param(href: &str, key: &str) -> Option<String> {
    let q_start = href.find('?')?;
    let query = &href[q_start + 1..];
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k.eq_ignore_ascii_case(key) {
            return Some(v.to_string());
        }
    }
    None
}

/// 从完整登录 URL 提取 client hints (platform / os / ver)
#[must_use]
pub fn extract_client_hints(raw_input: &str) -> LoginClientHints {
    let raw = raw_input.trim();
    let mut hints = LoginClientHints::default();
    if raw.is_empty() || !raw.contains('?') {
        return hints;
    }

    // URL 形式
    let href = to_href(raw);
    if !href.is_empty() {
        hints.platform = query_param(&href, "platform")
            .map(|v| decode_param(&v).to_ascii_lowercase())
            .unwrap_or_default();
        hints.os = query_param(&href, "os").map(|v| decode_param(&v)).unwrap_or_default();
        hints.ver = query_param(&href, "ver").map(|v| decode_param(&v)).unwrap_or_default();
        return hints;
    }

    // query 形式
    hints.platform = query_param(raw, "platform")
        .map(|v| decode_param(&v).to_ascii_lowercase())
        .unwrap_or_default();
    hints.os = query_param(raw, "os").map(|v| decode_param(&v)).unwrap_or_default();
    hints.ver = query_param(raw, "ver").map(|v| decode_param(&v)).unwrap_or_default();
    hints
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_login_platform_basic() {
        assert_eq!(normalize_login_platform("qq"), "qq");
        assert_eq!(normalize_login_platform("QQ"), "qq");
        assert_eq!(normalize_login_platform("Wx"), "wx");
        assert_eq!(normalize_login_platform("wechat"), "");
        assert_eq!(normalize_login_platform(""), "");
    }

    #[test]
    fn extract_code_from_bare_code() {
        assert_eq!(extract_code("abc123"), "abc123");
    }

    #[test]
    fn extract_code_from_path() {
        assert_eq!(extract_code("/prod/ws?code=hello123&platform=qq"), "hello123");
    }

    #[test]
    fn extract_code_from_full_url() {
        assert_eq!(extract_code("wss://gate-obt.nqf.qq.com/prod/ws?code=ABC&platform=qq"), "ABC");
    }

    #[test]
    fn extract_code_from_query_only() {
        assert_eq!(extract_code("?code=XYZ&os=android"), "XYZ");
    }

    #[test]
    fn extract_code_url_encoded() {
        assert_eq!(extract_code("?code=hello%20world"), "hello world");
    }

    #[test]
    fn extract_code_empty() {
        assert_eq!(extract_code(""), "");
        assert_eq!(extract_code("   "), "");
    }

    #[test]
    fn extract_client_hints_from_url() {
        let h = extract_client_hints(
            "wss://gate-obt.nqf.qq.com/prod/ws?code=X&platform=QQ&os=android&ver=1.0",
        );
        assert_eq!(h.platform, "qq");
        assert_eq!(h.os, "android");
        assert_eq!(h.ver, "1.0");
    }

    #[test]
    fn extract_client_hints_from_path() {
        let h = extract_client_hints("/prod/ws?platform=WX&os=ios&ver=2.0");
        assert_eq!(h.platform, "wx");
        assert_eq!(h.os, "ios");
        assert_eq!(h.ver, "2.0");
    }

    #[test]
    fn extract_client_hints_empty() {
        let h = extract_client_hints("");
        assert_eq!(h.platform, "");
        assert_eq!(h.os, "");
        assert_eq!(h.ver, "");
    }
}
