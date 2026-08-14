//! 扫码登录 — QQ 小程序开发者工具登录态获取。
//!
//! 1:1 翻译原 `core/src/services/qrlogin.ts`（129 行）。
//!
//! ## 流程
//!
//! 1. 调用 `https://q.qq.com/ide/devtoolAuth/GetLoginCode` 拿登录码
//! 2. 用户用手机 QQ 扫码 `https://h5.qzone.qq.com/qqq/code/{code}?_proxy=1&from=ide`
//! 3. 轮询 `https://q.qq.com/ide/devtoolAuth/syncScanSateGetTicket?code={code}` 拿 ticket
//! 4. 用 ticket 调 `https://q.qq.com/ide/login` 拿 auth code（用于登录游戏）
//!
//! 注：本模块是协议层封装，真实 HTTP 请求需要外部 `reqwest::Client`。
//! 单元测试主要覆盖 URL 构造和状态码归一化。

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Chrome 浏览器 User-Agent（用于模拟 IDE 请求）
pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// QUA 标识（QQ 平台版本）
pub const QUA: &str = "V1_HT5_QDT_0.70.2209190_x64_0_DEV_D";

const HOST: &str = "q.qq.com";
const LOGIN_CODE_URL: &str = "https://q.qq.com/ide/devtoolAuth/GetLoginCode";
const SYNC_STATUS_URL: &str = "https://q.qq.com/ide/devtoolAuth/syncScanSateGetTicket";
const LOGIN_EXCHANGE_URL: &str = "https://q.qq.com/ide/login";

/// 小程序预设
#[derive(Debug, Clone, Serialize)]
pub struct MpPreset {
    pub name: &'static str,
    pub description: &'static str,
    pub appid: &'static str,
}

/// 默认内置预设（1:1 对齐原 TS `MiniProgramLoginSession.Presets`）
pub const PRESETS: &[(&str, MpPreset)] = &[(
    "farm",
    MpPreset {
        name: "QQ经典农场 (Farm)",
        description: "QQ经典农场小程序",
        appid: "1112386029",
    },
)];

/// 通过 appid 查找预设
#[must_use]
pub fn find_preset(appid: &str) -> Option<&'static MpPreset> {
    PRESETS
        .iter()
        .map(|(_, p)| p)
        .find(|p| p.appid == appid)
}

/// 通过 name 查找预设
#[must_use]
pub fn find_preset_by_name(name: &str) -> Option<&'static MpPreset> {
    PRESETS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, p)| p)
}

/// 构造请求头（1:1 对齐 `MiniProgramLoginSession.getHeaders`）
#[must_use]
pub fn get_headers() -> [(&'static str, &'static str); 5] {
    [
        ("qua", QUA),
        ("host", HOST),
        ("accept", "application/json"),
        ("content-type", "application/json"),
        ("user-agent", CHROME_UA),
    ]
}

/// 登录码响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpLoginCodeResult {
    pub code: String,
    pub url: String,
    pub image: String,
}

/// 登录状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MpStatus {
    /// 已扫码 / 等待用户操作
    Wait,
    /// 成功
    OK,
    /// 二维码已使用
    Used,
    /// 错误
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpStatusResult {
    pub status: MpStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}

impl MpStatusResult {
    #[must_use]
    pub fn ok(ticket: String, uin: String, nickname: String) -> Self {
        Self {
            status: MpStatus::OK,
            ticket: Some(ticket),
            uin: Some(uin),
            nickname: Some(nickname),
            msg: None,
        }
    }

    #[must_use]
    pub fn wait() -> Self {
        Self {
            status: MpStatus::Wait,
            ticket: None,
            uin: None,
            nickname: None,
            msg: None,
        }
    }

    #[must_use]
    pub fn used() -> Self {
        Self {
            status: MpStatus::Used,
            ticket: None,
            uin: None,
            nickname: None,
            msg: None,
        }
    }

    #[must_use]
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            status: MpStatus::Error,
            ticket: None,
            uin: None,
            nickname: None,
            msg: Some(msg.into()),
        }
    }
}

/// 登录态管理
pub struct MiniProgramLoginSession {
    client: reqwest::Client,
}

impl Default for MiniProgramLoginSession {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniProgramLoginSession {
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// 暴露内部 client（供单元测试 / 特殊场景）
    #[must_use]
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// 构造登录 URL（用户扫码的地址）
    ///
    /// `code` — `request_login_code` 返回的 code
    #[must_use]
    pub fn build_login_url(code: &str) -> String {
        format!("https://h5.qzone.qq.com/qqq/code/{code}?_proxy=1&from=ide")
    }

    /// 调用 `q.qq.com` 拿登录码
    ///
    /// # Errors
    /// - 网络错误
    /// - 业务码非 0
    pub async fn request_login_code(&self) -> Result<MpLoginCodeResult, String> {
        let mut headers = reqwest::header::HeaderMap::new();
        for (k, v) in get_headers() {
            headers.insert(
                reqwest::header::HeaderName::from_static(k),
                reqwest::header::HeaderValue::from_static(v),
            );
        }
        let resp = self
            .client
            .get(LOGIN_CODE_URL)
            .headers(headers)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let code = body.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        if code != 0 {
            return Err("获取登录码失败".to_string());
        }
        let login_code = body
            .get("data")
            .and_then(|d| d.get("code"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        let url = Self::build_login_url(&login_code);
        // 生成 PNG data URL（对齐 TS `QRCode.toDataURL(url, {width:300, margin:1, level:M})`）
        let image = qr_png_data_url(&url);
        Ok(MpLoginCodeResult {
            code: login_code,
            url,
            image,
        })
    }

    /// 查询扫码状态
    ///
    /// # Errors
    /// - 网络错误
    pub async fn query_status(&self, code: &str) -> Result<MpStatusResult, String> {
        let mut headers = reqwest::header::HeaderMap::new();
        for (k, v) in get_headers() {
            headers.insert(
                reqwest::header::HeaderName::from_static(k),
                reqwest::header::HeaderValue::from_static(v),
            );
        }
        let url = format!("{SYNC_STATUS_URL}?code={code}");
        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status() != reqwest::StatusCode::OK {
            return Ok(MpStatusResult::error("non-200 status"));
        }
        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let res_code = body.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        match res_code {
            0 => {
                let data = body.get("data");
                let ok = data
                    .and_then(|d| d.get("ok"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                if ok != 1 {
                    Ok(MpStatusResult::wait())
                } else {
                    let ticket = data
                        .and_then(|d| d.get("ticket"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let uin = data
                        .and_then(|d| d.get("uin"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let nickname = data
                        .and_then(|d| d.get("nick"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Ok(MpStatusResult::ok(ticket, uin, nickname))
                }
            }
            -10003 => Ok(MpStatusResult::used()),
            other => Ok(MpStatusResult::error(format!("Code: {other}"))),
        }
    }

    /// 用 ticket 换 auth code
    ///
    /// # Errors
    /// - 网络错误
    pub async fn get_auth_code(
        &self,
        ticket: &str,
        appid: &str,
    ) -> Result<String, String> {
        let mut headers = reqwest::header::HeaderMap::new();
        for (k, v) in get_headers() {
            headers.insert(
                reqwest::header::HeaderName::from_static(k),
                reqwest::header::HeaderValue::from_static(v),
            );
        }
        let resp = self
            .client
            .post(LOGIN_EXCHANGE_URL)
            .headers(headers)
            .json(&serde_json::json!({
                "appid": appid,
                "ticket": ticket,
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status() != reqwest::StatusCode::OK {
            return Ok(String::new());
        }
        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(body
            .get("code")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string())
    }
}

/// 生成 PNG 二维码 data URL（对齐 TS `QRCode.toDataURL(url, {width:300, margin:1, level:'M'})`）
///
/// 输出格式：`data:image/png;base64,<base64>`。
/// 编码失败时回退为空字符串（调用方已有 `url` 兜底）。
#[must_use]
pub fn qr_png_data_url(text: &str) -> String {
    use base64::Engine;
    use qrcode::QrCode;

    let Ok(code) = QrCode::with_error_correction_level(text, qrcode::EcLevel::M) else {
        return String::new();
    };
    // 生成 300x300 的 PNG（scale 尽量大以接近 300px，再放大到目标尺寸）
    let img = code.render::<image::Luma<u8>>().min_dimensions(300, 300).build();
    let img = image::DynamicImage::ImageLuma8(img)
        .resize_exact(300, 300, image::imageops::FilterType::Nearest);
    let mut buf = std::io::Cursor::new(Vec::new());
    if img.write_to(&mut buf, image::ImageFormat::Png).is_err() {
        return String::new();
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    format!("data:image/png;base64,{b64}")
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qua_constant() {
        assert_eq!(QUA, "V1_HT5_QDT_0.70.2209190_x64_0_DEV_D");
    }

    #[test]
    fn chrome_ua_constant() {
        assert!(CHROME_UA.starts_with("Mozilla/5.0"));
        assert!(CHROME_UA.contains("Chrome/120"));
    }

    #[test]
    fn headers_include_qua_and_ua() {
        let h = get_headers();
        assert!(h.iter().any(|(k, _)| *k == "qua"));
        assert!(h.iter().any(|(k, _)| *k == "user-agent"));
        assert!(h.iter().any(|(k, _)| *k == "host"));
    }

    #[test]
    fn find_preset_farm() {
        let p = find_preset("1112386029").unwrap();
        assert_eq!(p.appid, "1112386029");
        assert!(p.name.contains("Farm"));
    }

    #[test]
    fn find_preset_missing() {
        assert!(find_preset("nonexistent").is_none());
    }

    #[test]
    fn find_preset_by_name_basic() {
        let p = find_preset_by_name("farm").unwrap();
        assert_eq!(p.appid, "1112386029");
    }

    #[test]
    fn find_preset_by_name_missing() {
        assert!(find_preset_by_name("nope").is_none());
    }

    #[test]
    fn build_login_url_format() {
        let url = MiniProgramLoginSession::build_login_url("abc123");
        assert_eq!(url, "https://h5.qzone.qq.com/qqq/code/abc123?_proxy=1&from=ide");
    }

    #[test]
    fn status_result_ok_default() {
        let r = MpStatusResult::ok("t".to_string(), "u".to_string(), "n".to_string());
        assert_eq!(r.status, MpStatus::OK);
        assert_eq!(r.ticket.as_deref(), Some("t"));
    }

    #[test]
    fn status_result_wait() {
        let r = MpStatusResult::wait();
        assert_eq!(r.status, MpStatus::Wait);
        assert!(r.ticket.is_none());
    }

    #[test]
    fn status_result_used() {
        let r = MpStatusResult::used();
        assert_eq!(r.status, MpStatus::Used);
    }

    #[test]
    fn status_result_error() {
        let r = MpStatusResult::error("boom");
        assert_eq!(r.status, MpStatus::Error);
        assert_eq!(r.msg.as_deref(), Some("boom"));
    }

    #[test]
    fn mp_status_eq() {
        assert_eq!(MpStatus::OK, MpStatus::OK);
        assert_ne!(MpStatus::OK, MpStatus::Wait);
    }

    #[test]
    fn mp_status_serialize_pascal_case() {
        let json = serde_json::to_string(&MpStatus::OK).unwrap();
        assert_eq!(json, "\"OK\"");
    }

    #[test]
    fn login_code_result_serializable() {
        let r = MpLoginCodeResult {
            code: "abc".to_string(),
            url: "https://example.com".to_string(),
            image: "data:".to_string(),
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["code"], "abc");
    }

    #[test]
    fn status_result_skips_none_fields() {
        let r = MpStatusResult::wait();
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        // ticket / uin / nickname / msg 都应为 null 或 absent
        assert_eq!(v["status"], "Wait");
        assert!(v.get("ticket").map_or(true, |x| x.is_null()));
    }

    #[test]
    fn session_default_works() {
        let s = MiniProgramLoginSession::default();
        // 仅验证不 panic
        let _ = s.client();
    }

    #[test]
    fn session_new_works() {
        let s = MiniProgramLoginSession::new();
        let _ = s.client();
    }

    #[test]
    fn qr_png_data_url_generates_png() {
        use base64::Engine;
        let data_url = qr_png_data_url("https://example.com/qr");
        assert!(data_url.starts_with("data:image/png;base64,"));
        let b64 = data_url.trim_start_matches("data:image/png;base64,");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("应解码出有效 PNG");
        // PNG 魔数
        assert_eq!(&bytes[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    #[test]
    fn qr_png_data_url_empty_text_still_encodes() {
        // 空文本也能编码（不 panic），返回空串或合法 data URL 均可接受
        let data_url = qr_png_data_url("");
        assert!(data_url.is_empty() || data_url.starts_with("data:image/png;base64,"));
    }
}
