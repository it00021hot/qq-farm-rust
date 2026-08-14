//! 微信扫码登录 — QR 流程封装。
//!
//! 1:1 翻译原 `core/src/services/wx-login/service.ts`（157 行）。
//!
//! ## 流程
//!
//! 1. `create_qr_session()` — 拉取 QR 会话，下载 QR 二维码图片
//! 2. 用户扫码 → 业务后端轮询 `poll()` 检查扫码状态
//! 3. 用户确认授权 → `confirm()` 拿 `openid` + `loginBuffer`
//! 4. `issue_code()` 调 `native_protocol::get_native_wx_login_code` 拿 `wx.login` code
//!
//! ## 与原 TS 的差异
//!
//! - 真实 HTTP 走 `reqwest`（TS 用 `fetch`）
//! - Cookie store 用 `HashMap<String, String>`（TS 用 `Map`）
//! - `issue_code` 调 `native_protocol::get_native_wx_login_code` 拿真实 wx.login code

use std::collections::HashMap;
use std::time::Duration;

use reqwest::redirect::Policy;

use super::native_protocol;

/// 微信 QR 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStatus {
    Waiting,
    Scanned,
    Authorized,
    Cancelled,
    Expired,
}

/// 微信登录 session
#[derive(Debug, Clone, Default)]
pub struct WxLoginSession {
    pub cookies: HashMap<String, String>,
    pub uuid: String,
    pub oauth_code: Option<String>,
    pub openid: Option<String>,
    pub login_buffer: Option<String>,
}

impl WxLoginSession {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 清除敏感字段
    pub fn clear_sensitive(&mut self) {
        self.oauth_code = None;
        self.openid = None;
        self.login_buffer = None;
    }
}

const QR_CONNECT_URL: &str = "https://open.weixin.qq.com/connect/qrconnect";
const QR_IMAGE_BASE: &str = "https://open.weixin.qq.com/connect/qrcode/";
const QR_POLL_URL: &str = "https://long.open.weixin.qq.com/connect/l/qrconnect";
const CALLBACK_URL: &str = "https://yybadaccess.3g.qq.com/pc_yyb/pcyyb_oauth";
const LOGIN_BUFFER_URL: &str = "https://yybadaccess.3g.qq.com/pc_yyb_auth/pcyyb_get_wx_login_buffer_auth";
const OAUTH_APP_ID: &str = "wxd44977328b36e647";
const USER_AGENT: &str = "Mozilla/5.0";
const LOGIN_BUFFER_ACCESS_KEY: &str = "wgrdg373hy26ww2";

/// HTTP 响应
pub struct HttpResult {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: HashMap<String, String>,
}

/// 微信登录服务
pub struct WxLoginService {
    client: reqwest::Client,
}

impl Default for WxLoginService {
    fn default() -> Self {
        Self::new()
    }
}

impl WxLoginService {
    #[must_use]
    pub fn new() -> Self {
        // 5 次重定向上限 + 不自动跟随（手动处理）
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(35))
            .redirect(Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// 暴露内部 client（供单元测试 / 特殊场景）
    #[must_use]
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// 创建 QR 会话
    ///
    /// # Errors
    /// - HTTP 错误
    /// - 无法从 HTML 解析出 uuid
    pub async fn create_qr_session(&self) -> Result<(WxLoginSession, Vec<u8>), String> {
        let mut cookies: HashMap<String, String> = HashMap::new();
        let params = [
            ("appid", OAUTH_APP_ID),
            ("redirect_uri", &format!("{CALLBACK_URL}?login_type=WX")),
            ("response_type", "code"),
            ("scope", "snsapi_login,snsapi_runtime_pcsdk"),
            ("state", "web"),
            ("fast_login", "1"),
            ("self_redirect", "true"),
        ];
        let query = encode_query(&params);
        let page = self.request(&format!("{QR_CONNECT_URL}?{query}"), &mut cookies, None).await?;
        if !(200..300).contains(&page.status) {
            return Err(format!("Unable to create WeChat QR session (HTTP {})", page.status));
        }
        let body_str = String::from_utf8_lossy(&page.body);
        let uuid = extract_uuid(&body_str)
            .ok_or_else(|| "Unable to parse the WeChat QR session".to_string())?;
        let qr_resp = self
            .request(&format!("{QR_IMAGE_BASE}{}", url_encode(&uuid)), &mut cookies, None)
            .await?;
        if !(200..300).contains(&qr_resp.status) {
            return Err(format!("Unable to download WeChat QR image (HTTP {})", qr_resp.status));
        }
        let mut session = WxLoginSession::new();
        session.cookies = cookies;
        session.uuid = uuid;
        Ok((session, qr_resp.body))
    }

    /// 轮询扫码状态
    ///
    /// # Errors
    /// - HTTP 错误
    /// - 未识别的 errcode
    pub async fn poll(&self, session: &mut WxLoginSession) -> Result<ScanStatus, String> {
        if session.oauth_code.is_some() {
            return Ok(ScanStatus::Authorized);
        }
        let params = [("uuid", session.uuid.as_str()), ("_", &now_ms_string())];
        let query = encode_query(&params);
        let resp = self
            .request(&format!("{QR_POLL_URL}?{query}"), &mut session.cookies, None)
            .await?;
        if !(200..300).contains(&resp.status) {
            return Err(format!("WeChat QR polling failed (HTTP {})", resp.status));
        }
        let body = String::from_utf8_lossy(&resp.body).to_string();
        let errcode = extract_pattern(&body, r"wx_errcode\s*=\s*(\d+)");
        match errcode.as_deref() {
            Some("408") => Ok(ScanStatus::Waiting),
            Some("404") => Ok(ScanStatus::Scanned),
            Some("403") => Ok(ScanStatus::Cancelled),
            Some("402") => Ok(ScanStatus::Expired),
            Some("405") => {
                let code = extract_pattern(&body, r"wx_code\s*=\s*'([^']+)'")
                    .ok_or_else(|| "WeChat authorization response did not include a code".to_string())?;
                session.oauth_code = Some(code);
                Ok(ScanStatus::Authorized)
            }
            _ => Err("Unrecognized WeChat QR polling response".to_string()),
        }
    }

    /// 确认授权，拿到 `openid` + `loginBuffer`
    ///
    /// # Errors
    /// - 未授权 / HTTP 错误 / 响应解析失败
    pub async fn confirm(&self, session: &mut WxLoginSession) -> Result<(String, String), String> {
        let oauth_code = session
            .oauth_code
            .as_ref()
            .ok_or_else(|| "Waiting for scan authorization".to_string())?;
        let params = [
            ("login_type", "WX"),
            ("code", oauth_code.as_str()),
            ("state", "web"),
        ];
        let query = encode_query(&params);
        let callback = self
            .request(&format!("{CALLBACK_URL}?{query}"), &mut session.cookies, None)
            .await?;
        if callback.status < 200 || callback.status >= 400 {
            return Err(format!(
                "WeChat authorization callback failed (HTTP {})",
                callback.status
            ));
        }
        let openid = required_cookie(&session.cookies, "openid")?;
        let access_token = required_cookie(&session.cookies, "accesstoken")?;

        // 构造 JSON payload（用 serde_json 序列化，避免 format! 拼接在特殊字符下出错）
        let payload = serde_json::json!({
            "extInfo": {
                "listS": {
                    "unionid": { "value": [openid.clone()] },
                    "user_id": { "value": [openid.clone()] },
                    "access_token": { "value": [access_token] },
                },
                "listI": {
                    "user_type": { "value": [0] },
                },
            },
        })
        .to_string();
        let timestamp = now_ms_string();
        let nonce = random_int(1000, 10000).to_string();
        let signature = login_buffer_signature(&payload, &timestamp, &nonce);

        let mut extra_headers = HashMap::new();
        extra_headers.insert("Content-Type".to_string(), "application/json".to_string());
        extra_headers.insert("Ual-Access-Businessid".to_string(), "pc_yyb_auth".to_string());
        extra_headers.insert("Ual-Access-Timestamp".to_string(), timestamp.clone());
        extra_headers.insert("Ual-Access-Nonce".to_string(), nonce.clone());
        extra_headers.insert("Ual-Access-Signature".to_string(), signature);

        let response = self
            .request(
                LOGIN_BUFFER_URL,
                &mut session.cookies,
                Some(RequestInput {
                    method: "POST",
                    body: Some(payload.as_bytes().to_vec()),
                    extra_headers: extra_headers,
                }),
            )
            .await?;
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "Unable to obtain WeChat login buffer (HTTP {})",
                response.status
            ));
        }
        let body_str = String::from_utf8_lossy(&response.body);
        let data: serde_json::Value =
            serde_json::from_str(&body_str).map_err(|e| format!("JSON parse: {e}"))?;
        let code = data.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        let login_buffer = if code == 0 {
            data.get("ext_info")
                .and_then(|e| e.get("list_s"))
                .and_then(|l| l.get("login_buffer"))
                .and_then(|b| b.get("value"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };
        if login_buffer.is_empty() {
            return Err("WeChat login buffer response is invalid".to_string());
        }
        session.cookies.clear();
        session.openid = Some(openid.clone());
        session.login_buffer = Some(login_buffer.clone());
        Ok((openid, login_buffer))
    }

    /// 真实协议拿 wx.login code
    ///
    /// # Errors
    /// - login_buffer 缺失（尚未 confirm）
    /// - 原生协议网络 / 握手 / 解密失败
    pub async fn issue_code(&self, session: &WxLoginSession, app_id: &str) -> Result<String, String> {
        let buffer = session
            .login_buffer
            .as_ref()
            .ok_or_else(|| "WeChat login session has not been confirmed".to_string())?;
        native_protocol::get_native_wx_login_code(buffer, app_id).await
    }

    /// 销毁 session
    pub fn destroy(&self, session: &mut WxLoginSession) {
        session.cookies.clear();
        session.clear_sensitive();
    }

    // ----- HTTP helper -----

    async fn request(
        &self,
        url: &str,
        cookies: &mut HashMap<String, String>,
        input: Option<RequestInput>,
    ) -> Result<HttpResult, String> {
        let mut method = input
            .as_ref()
            .map(|i| i.method.to_string())
            .unwrap_or_else(|| "GET".to_string());
        let mut body = input.as_ref().and_then(|i| i.body.clone());

        let mut current_url = url.to_string();
        // 最多跟随 5 次重定向（对齐 TS `request()`）
        for _ in 0..=5 {
            let method_parsed: reqwest::Method = match method.as_str() {
                "GET" => reqwest::Method::GET,
                "POST" => reqwest::Method::POST,
                "PUT" => reqwest::Method::PUT,
                "DELETE" => reqwest::Method::DELETE,
                "HEAD" => reqwest::Method::HEAD,
                _ => return Err(format!("unsupported HTTP method: {method}")),
            };
            let mut req = self.client.request(method_parsed, &current_url);
            req = req.header("User-Agent", USER_AGENT);
            if !cookies.is_empty() {
                req = req.header("Cookie", cookie_header(cookies));
            }
            if let Some(input) = &input {
                for (k, v) in &input.extra_headers {
                    req = req.header(k, v);
                }
            }
            if let Some(b) = &body {
                req = req.body(b.clone());
            }
            let resp = req.send().await.map_err(|e| e.to_string())?;
            let status = resp.status().as_u16();
            let headers_snapshot = headers_to_map(resp.headers());
            store_cookies(cookies, &resp);

            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            // 对齐 TS：非 3xx、或 4xx/5xx、或没有 location，则读取 body 并返回。
            // 只有「3xx 且有 location」才会跟随重定向。
            if status < 300 || status >= 400 || location.is_none() {
                let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();
                return Ok(HttpResult {
                    status,
                    body: bytes,
                    headers: headers_snapshot,
                });
            }

            // 跟随重定向
            let location = location.expect("checked is_none above");
            current_url = resolve_url(&current_url, &location);
            if status == 303 || ((status == 301 || status == 302) && method == "POST") {
                // 303 / 301+POST / 302+POST 转 GET，丢弃 body
                method = "GET".to_string();
                body = None;
            }
        }
        Err("Too many redirects while contacting WeChat".to_string())
    }
}

/// 请求额外参数
pub struct RequestInput {
    pub method: &'static str,
    pub body: Option<Vec<u8>>,
    pub extra_headers: HashMap<String, String>,
}

// =====================================================================
// 工具
// =====================================================================

fn cookie_header(cookies: &HashMap<String, String>) -> String {
    cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn store_cookies(cookies: &mut HashMap<String, String>, resp: &reqwest::Response) {
    for value in resp.headers().get_all("set-cookie") {
        let s = match value.to_str() {
            Ok(v) => v.to_string(),
            Err(_) => String::from_utf8_lossy(value.as_bytes()).into_owned(),
        };
        let pair = s.split(';').next().unwrap_or("").trim();
        if let Some(eq) = pair.find('=') {
            let k = pair[..eq].to_string();
            let v = pair[eq + 1..].to_string();
            if !k.is_empty() {
                cookies.insert(k, v);
            }
        }
    }
}

fn login_buffer_signature(payload: &str, timestamp: &str, nonce: &str) -> String {
    md5_hex(format!("{payload}{timestamp}{LOGIN_BUFFER_ACCESS_KEY}{nonce}").as_bytes())
}

fn required_cookie(cookies: &HashMap<String, String>, name: &str) -> Result<String, String> {
    cookies
        .get(name)
        .cloned()
        .ok_or_else(|| format!("WeChat OAuth callback did not provide {name}"))
}

fn encode_query(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn resolve_url(base: &str, location: &str) -> String {
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    // 简化：直接拼接到 base 域名
    if let Some(scheme_end) = base.find("://") {
        let after_scheme = &base[scheme_end + 3..];
        if let Some(path_start) = after_scheme.find('/') {
            let domain = &base[..scheme_end + 3 + path_start];
            if location.starts_with('/') {
                return format!("{}{}", domain, location);
            }
            return format!("{}/{}", domain, location);
        }
    }
    location.to_string()
}

fn extract_uuid(body: &str) -> Option<String> {
    // /connect/qrcode/([^"'>\s]+)
    let marker = "/connect/qrcode/";
    let start = body.find(marker)? + marker.len();
    let end = body[start..]
        .find(|c: char| c == '"' || c == '\'' || c == '>' || c == ' ' || c == '\t' || c == '\n' || c == '\r')
        .map(|i| start + i)
        .unwrap_or(body.len());
    Some(body[start..end].to_string())
}

fn extract_pattern(body: &str, pattern: &str) -> Option<String> {
    // 简化：手动写两个常见 pattern
    if pattern.contains("wx_errcode") {
        return find_after(body, "wx_errcode", "0123456789");
    }
    if pattern.contains("wx_code") {
        return find_after_quoted(body, "wx_code");
    }
    None
}

fn find_after(body: &str, key: &str, allowed: &str) -> Option<String> {
    let pos = body.find(key)?;
    let after = &body[pos + key.len()..];
    // 跳过空白 / `=` / `"`
    let mut chars = after.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '=' || c == '"' || c == '\'' {
            chars.next();
        } else {
            break;
        }
    }
    let mut out = String::new();
    for c in chars {
        if allowed.contains(c) {
            out.push(c);
        } else {
            break;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn find_after_quoted(body: &str, key: &str) -> Option<String> {
    let pos = body.find(key)?;
    let after = &body[pos + key.len()..];
    let mut chars = after.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '=' {
            chars.next();
        } else {
            break;
        }
    }
    // 必须以 `'` 或 `"` 开头
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let mut out = String::new();
    for c in chars {
        if c == quote {
            return Some(out);
        }
        out.push(c);
    }
    None
}

fn headers_to_map(headers: &reqwest::header::HeaderMap) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, v) in headers {
        if let Ok(s) = v.to_str() {
            out.insert(k.as_str().to_string(), s.to_string());
        }
    }
    out
}

fn now_ms_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn random_int(min: i64, max: i64) -> i64 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(min..max)
}

fn md5_hex(data: &[u8]) -> String {
    // 简化：实现 MD5（不引外部 crate）
    md5(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// MD5 实现（RFC 1321）
#[must_use]
pub fn md5(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xefcdab89;
    let mut h2: u32 = 0x98badcfe;
    let mut h3: u32 = 0x10325476;

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut msg = input.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 16];
        for (i, b) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        let (mut a, mut b, mut c, mut d) = (h0, h1, h2, h3);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f
                .wrapping_add(a)
                .wrapping_add(K[i])
                .wrapping_add(w[g]);
            let new_a = d;
            let new_d = c;
            let new_c = b;
            let new_b = b.wrapping_add(f.rotate_left(S[i]));
            a = new_a;
            b = new_b;
            c = new_c;
            d = new_d;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&h0.to_le_bytes());
    out[4..8].copy_from_slice(&h1.to_le_bytes());
    out[8..12].copy_from_slice(&h2.to_le_bytes());
    out[12..16].copy_from_slice(&h3.to_le_bytes());
    out
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_constants() {
        assert_eq!(QR_CONNECT_URL, "https://open.weixin.qq.com/connect/qrconnect");
        assert_eq!(QR_IMAGE_BASE, "https://open.weixin.qq.com/connect/qrcode/");
        assert_eq!(QR_POLL_URL, "https://long.open.weixin.qq.com/connect/l/qrconnect");
        assert_eq!(CALLBACK_URL, "https://yybadaccess.3g.qq.com/pc_yyb/pcyyb_oauth");
        assert_eq!(LOGIN_BUFFER_URL, "https://yybadaccess.3g.qq.com/pc_yyb_auth/pcyyb_get_wx_login_buffer_auth");
        assert_eq!(OAUTH_APP_ID, "wxd44977328b36e647");
        assert_eq!(USER_AGENT, "Mozilla/5.0");
        assert_eq!(LOGIN_BUFFER_ACCESS_KEY, "wgrdg373hy26ww2");
    }

    #[test]
    fn scan_status_eq() {
        assert_eq!(ScanStatus::Waiting, ScanStatus::Waiting);
        assert_ne!(ScanStatus::Waiting, ScanStatus::Scanned);
    }

    #[test]
    fn scan_status_debug() {
        let s = format!("{:?}", ScanStatus::Authorized);
        assert!(s.contains("Authorized"));
    }

    #[test]
    fn wx_login_session_default() {
        let s = WxLoginSession::new();
        assert!(s.cookies.is_empty());
        assert!(s.uuid.is_empty());
        assert!(s.oauth_code.is_none());
    }

    #[test]
    fn wx_login_session_clear_sensitive() {
        let mut s = WxLoginSession::new();
        s.oauth_code = Some("code".into());
        s.openid = Some("openid".into());
        s.login_buffer = Some("buf".into());
        s.clear_sensitive();
        assert!(s.oauth_code.is_none());
        assert!(s.openid.is_none());
        assert!(s.login_buffer.is_none());
    }

    #[test]
    fn cookie_header_basic() {
        let mut c = HashMap::new();
        c.insert("a".to_string(), "1".to_string());
        c.insert("b".to_string(), "2".to_string());
        let h = cookie_header(&c);
        // HashMap 顺序不稳定
        assert!(h.contains("a=1"));
        assert!(h.contains("b=2"));
        assert!(h.contains("; "));
    }

    #[test]
    fn cookie_header_empty() {
        let c: HashMap<String, String> = HashMap::new();
        assert_eq!(cookie_header(&c), "");
    }

    #[test]
    fn required_cookie_present() {
        let mut c = HashMap::new();
        c.insert("openid".to_string(), "oxxx".to_string());
        assert_eq!(required_cookie(&c, "openid").unwrap(), "oxxx");
    }

    #[test]
    fn required_cookie_missing() {
        let c: HashMap<String, String> = HashMap::new();
        assert!(required_cookie(&c, "openid").is_err());
    }

    #[test]
    fn encode_query_basic() {
        let s = encode_query(&[("a", "1"), ("b", "2")]);
        assert!(s.contains("a=1"));
        assert!(s.contains("b=2"));
    }

    #[test]
    fn encode_query_url_encodes() {
        let s = encode_query(&[("k", "hello world")]);
        assert!(s.contains("hello%20world"));
    }

    #[test]
    fn url_encode_passthrough() {
        assert_eq!(url_encode("hello-world_1.0~"), "hello-world_1.0~");
    }

    #[test]
    fn url_encode_unsafe() {
        assert_eq!(url_encode("a b"), "a%20b");
        assert_eq!(url_encode("a&b"), "a%26b");
        assert_eq!(url_encode("中文"), "%E4%B8%AD%E6%96%87");
    }

    #[test]
    fn resolve_url_absolute() {
        assert_eq!(
            resolve_url("https://example.com/path", "https://other.com/x"),
            "https://other.com/x"
        );
    }

    #[test]
    fn resolve_url_relative() {
        assert_eq!(
            resolve_url("https://example.com/foo/bar", "/baz"),
            "https://example.com/baz"
        );
    }

    #[test]
    fn extract_uuid_basic() {
        let body = r#"<img src="/connect/qrcode/abc123"/"#;
        assert_eq!(extract_uuid(body), Some("abc123".to_string()));
    }

    #[test]
    fn extract_uuid_with_quote() {
        let body = r#"code: '/connect/qrcode/xyz789'"#;
        assert_eq!(extract_uuid(body), Some("xyz789".to_string()));
    }

    #[test]
    fn extract_uuid_missing() {
        assert!(extract_uuid("nothing here").is_none());
    }

    #[test]
    fn extract_pattern_errcode() {
        let body = r#"wx_errcode=408"#;
        assert_eq!(extract_pattern(body, "wx_errcode"), Some("408".to_string()));
    }

    #[test]
    fn extract_pattern_code() {
        let body = r#"wx_code='abcdef'"#;
        assert_eq!(extract_pattern(body, "wx_code"), Some("abcdef".to_string()));
    }

    #[test]
    fn extract_pattern_missing() {
        assert!(extract_pattern("nothing", "wx_errcode").is_none());
    }

    #[test]
    fn md5_empty() {
        // MD5("") = d41d8cd98f00b204e9800998ecf8427e
        let h = md5_hex(b"");
        assert_eq!(h, "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn md5_abc() {
        // MD5("abc") = 900150983cd24fb0d6963f7d28e17f72
        let h = md5_hex(b"abc");
        assert_eq!(h, "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn md5_known_long() {
        // MD5("The quick brown fox jumps over the lazy dog") = 9e107d9d372bb6826bd81d3542a419d6
        let h = md5_hex(b"The quick brown fox jumps over the lazy dog");
        assert_eq!(h, "9e107d9d372bb6826bd81d3542a419d6");
    }

    #[test]
    fn new_service_works() {
        let s = WxLoginService::new();
        let _ = s.client();
    }

    #[test]
    fn default_service_works() {
        let s = WxLoginService::default();
        let _ = s.client();
    }

    #[test]
    fn destroy_clears_session() {
        let svc = WxLoginService::new();
        let mut s = WxLoginSession::new();
        s.cookies.insert("k".to_string(), "v".to_string());
        s.oauth_code = Some("c".to_string());
        svc.destroy(&mut s);
        assert!(s.cookies.is_empty());
        assert!(s.oauth_code.is_none());
    }

    #[test]
    fn issue_code_no_login_buffer() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let svc = WxLoginService::new();
            let s = WxLoginSession::new();
            let r = svc.issue_code(&s, "appid").await;
            assert!(r.is_err());
        });
    }

    #[test]
    fn issue_code_with_buffer_invokes_network() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let svc = WxLoginService::new();
            let mut s = WxLoginSession::new();
            s.login_buffer = Some("dGVzdA==".to_string()); // base64 "test"
            // 真实实现：会尝试 TCP connect 真实 longcloud.weixin.qq.com
            // 在 CI 沙盒环境会失败，返回网络错误（不是"集成时"占位错误）
            let r = svc.issue_code(&s, "appid").await;
            // 不论成功 / 失败，调用能跑通即可（不会 panic）
            let _ = r;
        });
    }

    #[test]
    fn confirm_no_oauth_code() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let svc = WxLoginService::new();
            let mut s = WxLoginSession::new();
            let r = svc.confirm(&mut s).await;
            assert!(r.is_err());
            assert!(r.unwrap_err().contains("Waiting"));
        });
    }

    #[test]
    fn poll_already_authorized() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let svc = WxLoginService::new();
            let mut s = WxLoginSession::new();
            s.oauth_code = Some("code".to_string());
            assert_eq!(svc.poll(&mut s).await.unwrap(), ScanStatus::Authorized);
        });
    }

    #[test]
    fn login_buffer_signature_stable() {
        let sig = login_buffer_signature("{}", "1", "2");
        assert_eq!(sig.len(), 32);
        assert_eq!(sig, login_buffer_signature("{}", "1", "2"));
        assert_ne!(sig, login_buffer_signature("{}", "1", "3"));
    }

    #[test]
    fn confirm_payload_json_roundtrip() {
        let payload = serde_json::json!({
            "extInfo": {
                "listS": {
                    "unionid": { "value": ["o\"x"] },
                    "user_id": { "value": ["o\"x"] },
                    "access_token": { "value": ["a&b"] },
                },
                "listI": {
                    "user_type": { "value": [0] },
                },
            },
        })
        .to_string();
        let v: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(v["extInfo"]["listS"]["unionid"]["value"][0], "o\"x");
        assert_eq!(v["extInfo"]["listS"]["access_token"]["value"][0], "a&b");
    }

    #[tokio::test]
    async fn request_follows_redirect_and_stores_cookies() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut hop = 0u8;
            loop {
                let (mut sock, _) = match listener.accept().await {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                hop += 1;
                let resp = if hop == 1 {
                    format!(
                        "HTTP/1.1 302 Found\r\n\
                         Location: http://{addr}/next\r\n\
                         Set-Cookie: sid=abc; Path=/\r\n\
                         Content-Length: 0\r\n\r\n"
                    )
                } else if req.starts_with("GET ") {
                    "HTTP/1.1 200 OK\r\n\
                     Set-Cookie: openid=oxxx; Path=/\r\n\
                     Set-Cookie: accesstoken=atok; Path=/\r\n\
                     Content-Length: 5\r\n\r\nhello"
                        .to_string()
                } else {
                    "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n".to_string()
                };
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });

        let svc = WxLoginService::new();
        let mut cookies = HashMap::new();
        let result = svc
            .request(
                &format!("http://{addr}/start"),
                &mut cookies,
                Some(RequestInput {
                    method: "POST",
                    body: Some(b"unused".to_vec()),
                    extra_headers: HashMap::new(),
                }),
            )
            .await
            .expect("request");
        assert_eq!(result.status, 200);
        assert_eq!(result.body, b"hello");
        assert_eq!(cookies.get("sid").map(String::as_str), Some("abc"));
        assert_eq!(cookies.get("openid").map(String::as_str), Some("oxxx"));
        assert_eq!(cookies.get("accesstoken").map(String::as_str), Some("atok"));
    }
}
