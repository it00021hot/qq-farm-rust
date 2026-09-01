//! 通用推送渠道 —— 钉钉群机器人 webhook（含加签）。
//!
//! 对齐 bot `core/src/services/push.ts` 的 `buildDingTalkWebhook` / 钉钉发送：
//! `sign = base64(HMAC-SHA256(secret, "{timestamp}\\n{secret}"))`；
//! endpoint 必须是 `https://oapi.dingtalk.com/robot/send?access_token=...`，
//! 或直接提交裸 access_token。

use hmac::{Hmac, Mac};
use sha2::Sha256;

const DINGTALK_WEBHOOK_PREFIX: &str = "https://oapi.dingtalk.com/robot/send?access_token=";

/// 构造带加签的钉钉 webhook URL。
///
/// - `endpoint`：完整 webhook URL 或裸 access_token（此时可再由 `token` 提供）
/// - `secret` 非空时追加 `&timestamp=...&sign=...`（URL 编码后的签名）
pub fn build_dingtalk_webhook(endpoint: &str, token: &str, secret: &str) -> Result<String, String> {
    let endpoint_text = endpoint.trim();
    let token_text = token.trim();
    let base = if endpoint_text.starts_with("https://") {
        if !endpoint_text.starts_with(DINGTALK_WEBHOOK_PREFIX) {
            return Err("钉钉 Webhook 地址格式无效".to_string());
        }
        endpoint_text.to_string()
    } else if !token_text.is_empty() {
        format!("{DINGTALK_WEBHOOK_PREFIX}{token_text}")
    } else {
        return Err("钉钉 Webhook 地址格式无效".to_string());
    };
    let secret_text = secret.trim();
    if secret_text.is_empty() {
        return Ok(base);
    }
    let timestamp = crate::utils::time::now_ms();
    let string_to_sign = format!("{timestamp}\n{secret_text}");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret_text.as_bytes())
        .map_err(|e| format!("钉钉加签失败: {e}"))?;
    mac.update(string_to_sign.as_bytes());
    use base64::Engine as _;
    let sign = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    let sign_encoded: String = urlencoding_like(&sign);
    Ok(format!("{base}&timestamp={timestamp}&sign={sign_encoded}"))
}

/// 只编码签名里会出现的字符（+ / =），避免引入完整 urlencoding 依赖
fn urlencoding_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'+' => out.push_str("%2B"),
            b'/' => out.push_str("%2F"),
            b'=' => out.push_str("%3D"),
            b'\n' => out.push_str("%0A"),
            _ => out.push(byte as char),
        }
    }
    out
}

/// 发送钉钉文本消息（title 与正文按官方 text 消息拼接）。
pub async fn send_dingtalk(
    endpoint: &str,
    token: &str,
    secret: &str,
    title: &str,
    content: &str,
) -> Result<(), String> {
    let url = build_dingtalk_webhook(endpoint, token, secret)?;
    let text = if title.trim().is_empty() {
        content.to_string()
    } else {
        format!("{title}\n{content}")
    };
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "msgtype": "text", "text": { "content": text } }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("钉钉发送失败: {e}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        return Err(format!("钉钉发送失败: HTTP {status}"));
    }
    // 钉钉 200 也可能回 errcode != 0（如 IP 白名单 / 加签错误）
    let errcode = body.get("errcode").and_then(|v| v.as_i64()).unwrap_or(0);
    if errcode != 0 {
        let errmsg = body.get("errmsg").and_then(|v| v.as_str()).unwrap_or("");
        return Err(format!("钉钉发送失败: errcode={errcode} {errmsg}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_webhook_accepts_bare_token() {
        let url = build_dingtalk_webhook("", "abc123", "").expect("url");
        assert_eq!(url, "https://oapi.dingtalk.com/robot/send?access_token=abc123");
    }

    #[test]
    fn build_webhook_rejects_invalid_endpoint() {
        assert!(build_dingtalk_webhook("https://evil.example.com/hook", "", "").is_err());
        assert!(build_dingtalk_webhook("", "", "").is_err());
    }

    #[test]
    fn build_webhook_signs_when_secret_present() {
        let url =
            build_dingtalk_webhook("", "tok", "SEC123").expect("url");
        assert!(url.starts_with(
            "https://oapi.dingtalk.com/robot/send?access_token=tok&timestamp="
        ));
        assert!(url.contains("&sign="), "signed url must contain sign: {url}");
    }

    #[test]
    fn build_webhook_keeps_full_url_without_secret() {
        let full = "https://oapi.dingtalk.com/robot/send?access_token=xyz";
        let url = build_dingtalk_webhook(full, "", "").expect("url");
        assert_eq!(url, full);
    }
}
