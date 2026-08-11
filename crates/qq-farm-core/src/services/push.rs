//! 推送服务 — 通知 / 告警推送。
//!
//! 1:1 翻译原 `core/src/services/push.ts`（62 行）。
//!
//! ## 渠道
//!
//! - `webhook` — 通用 webhook POST（实现）
//! - 其他渠道（serverchan / pushplus / 自定义）— 占位返回 `unsupported_channel` 错误
//!
//! ## 与原 TS 的差异
//!
//! - 原 TS 依赖 `pushoo` npm 包（多渠道聚合）
//!   本实现聚焦 webhook（实际使用最广泛的渠道），其它渠道暂未实现
//! - 返回结构与原 TS 一致：`{ ok, code, msg, raw }`

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// 推送 payload
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PushPayload {
    /// 推送渠道（`webhook` / `serverchan` / `pushplus` / ...）
    pub channel: String,
    /// webhook 接口地址（`channel=webhook` 时必填）
    pub endpoint: String,
    /// 推送 token（`channel=webhook` 时可选）
    pub token: String,
    /// 推送标题（必填）
    pub title: String,
    /// 推送内容（必填）
    pub content: String,
}

/// 推送结果
#[derive(Debug, Clone, Serialize)]
pub struct PushResult {
    pub ok: bool,
    pub code: String,
    pub msg: String,
    pub raw: serde_json::Value,
}

/// 推送服务
pub struct PushService {
    client: reqwest::Client,
}

impl Default for PushService {
    fn default() -> Self {
        Self::new()
    }
}

impl PushService {
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// 发送推送
    ///
    /// # Errors
    /// - 必填字段缺失
    /// - 渠道不支持
    /// - 网络错误
    pub async fn send(&self, payload: &PushPayload) -> PushResult {
        let channel = payload.channel.trim();
        if channel.is_empty() {
            return PushResult {
                ok: false,
                code: "missing_channel".to_string(),
                msg: "channel 不能为空".to_string(),
                raw: serde_json::Value::Null,
            };
        }
        let title = payload.title.trim();
        if title.is_empty() {
            return PushResult {
                ok: false,
                code: "missing_title".to_string(),
                msg: "title 不能为空".to_string(),
                raw: serde_json::Value::Null,
            };
        }
        let content = payload.content.trim();
        if content.is_empty() {
            return PushResult {
                ok: false,
                code: "missing_content".to_string(),
                msg: "content 不能为空".to_string(),
                raw: serde_json::Value::Null,
            };
        }
        let token = payload.token.trim();
        let endpoint = payload.endpoint.trim();

        match channel {
            "webhook" => self.send_webhook(endpoint, token, title, content).await,
            other => PushResult {
                ok: false,
                code: "unsupported_channel".to_string(),
                msg: format!("渠道 {} 暂未实现", other),
                raw: serde_json::Value::Null,
            },
        }
    }

    async fn send_webhook(
        &self,
        endpoint: &str,
        token: &str,
        title: &str,
        content: &str,
    ) -> PushResult {
        if endpoint.is_empty() {
            return PushResult {
                ok: false,
                code: "missing_endpoint".to_string(),
                msg: "webhook 模式需要 endpoint".to_string(),
                raw: serde_json::Value::Null,
            };
        }
        let mut body = serde_json::json!({
            "title": title,
            "content": content,
        });
        if !token.is_empty() {
            body["token"] = serde_json::Value::String(token.to_string());
        }
        match self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let raw_json: serde_json::Value =
                    serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({ "raw": text }));
                let has_error = raw_json.get("error").is_some();
                let code = raw_json
                    .get("code")
                    .or_else(|| raw_json.get("errcode"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        if has_error {
                            "error".to_string()
                        } else if status.is_success() {
                            "ok".to_string()
                        } else {
                            "http_error".to_string()
                        }
                    });
                let message = raw_json
                    .get("msg")
                    .or_else(|| raw_json.get("message"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        if has_error {
                            raw_json
                                .get("error")
                                .and_then(|e| e.get("message"))
                                .and_then(|m| m.as_str())
                                .map(str::to_string)
                                .unwrap_or_else(|| "push failed".to_string())
                        } else if status.is_success() {
                            "ok".to_string()
                        } else {
                            format!("HTTP {}", status.as_u16())
                        }
                    });
                let ok = !has_error
                    && status.is_success()
                    && (code == "ok" || code == "0" || code.is_empty());
                PushResult {
                    ok,
                    code,
                    msg: message,
                    raw: raw_json,
                }
            }
            Err(e) => PushResult {
                ok: false,
                code: "network_error".to_string(),
                msg: e.to_string(),
                raw: serde_json::Value::Null,
            },
        }
    }
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_service_builds() {
        let svc = PushService::new();
        // 仅验证不 panic
        let _ = svc;
    }

    #[test]
    fn default_trait_works() {
        let svc = PushService::default();
        let _ = svc;
    }

    #[tokio::test]
    async fn missing_channel() {
        let svc = PushService::new();
        let p = PushPayload {
            channel: "".to_string(),
            title: "t".to_string(),
            content: "c".to_string(),
            ..Default::default()
        };
        let r = svc.send(&p).await;
        assert!(!r.ok);
        assert_eq!(r.code, "missing_channel");
    }

    #[tokio::test]
    async fn missing_title() {
        let svc = PushService::new();
        let p = PushPayload {
            channel: "webhook".to_string(),
            title: "  ".to_string(),
            content: "c".to_string(),
            ..Default::default()
        };
        let r = svc.send(&p).await;
        assert!(!r.ok);
        assert_eq!(r.code, "missing_title");
    }

    #[tokio::test]
    async fn missing_content() {
        let svc = PushService::new();
        let p = PushPayload {
            channel: "webhook".to_string(),
            title: "t".to_string(),
            content: String::new(),
            ..Default::default()
        };
        let r = svc.send(&p).await;
        assert!(!r.ok);
        assert_eq!(r.code, "missing_content");
    }

    #[tokio::test]
    async fn unsupported_channel() {
        let svc = PushService::new();
        let p = PushPayload {
            channel: "serverchan".to_string(),
            title: "t".to_string(),
            content: "c".to_string(),
            ..Default::default()
        };
        let r = svc.send(&p).await;
        assert!(!r.ok);
        assert_eq!(r.code, "unsupported_channel");
    }

    #[tokio::test]
    async fn webhook_missing_endpoint() {
        let svc = PushService::new();
        let p = PushPayload {
            channel: "webhook".to_string(),
            title: "t".to_string(),
            content: "c".to_string(),
            ..Default::default()
        };
        let r = svc.send(&p).await;
        assert!(!r.ok);
        assert_eq!(r.code, "missing_endpoint");
    }

    #[tokio::test]
    async fn webhook_to_localhost_unreachable() {
        // 127.0.0.1:1 几乎一定连不上，但能验证 network_error 路径不 panic
        let svc = PushService::new();
        let p = PushPayload {
            channel: "webhook".to_string(),
            endpoint: "http://127.0.0.1:1/push".to_string(),
            title: "t".to_string(),
            content: "c".to_string(),
            token: "tok".to_string(),
        };
        let r = svc.send(&p).await;
        assert!(!r.ok);
        // 可能是 network_error 也可能是 http_error（取决于环境）
        assert!(r.code == "network_error" || r.code == "http_error");
    }
}
