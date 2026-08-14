//! 推送服务 — 通知 / 告警推送。
//!
//! 对齐原 `core/src/services/push.ts`（经 pushoo）与面板 `Settings.vue` `channelOptions`。
//!
//! ## 支持渠道（与面板一致）
//!
//! `webhook` / `qmsg` / `serverchan` / `pushplus` / `pushplushxtrip` / `dingtalk` /
//! `wecom` / `bark` / `gocqhttp` / `onebot` / `atri` / `pushdeer` / `igot` /
//! `telegram` / `feishu` / `ifttt` / `wecombot` / `discord` / `wxpusher`
//!
//! 原 TS 依赖 `pushoo`；这里用 reqwest 直接调各渠道 HTTP API。
//! 返回结构与原 TS 一致：`{ ok, code, msg, raw }`

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// 面板列出的全部渠道（顺序与 Settings.vue 一致）
pub const SUPPORTED_CHANNELS: &[&str] = &[
    "webhook",
    "qmsg",
    "serverchan",
    "pushplus",
    "pushplushxtrip",
    "dingtalk",
    "wecom",
    "bark",
    "gocqhttp",
    "onebot",
    "atri",
    "pushdeer",
    "igot",
    "telegram",
    "feishu",
    "ifttt",
    "wecombot",
    "discord",
    "wxpusher",
];

/// 推送 payload
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PushPayload {
    /// 推送渠道
    pub channel: String,
    /// webhook / 机器人接口地址（部分渠道必填）
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
            "qmsg" => {
                let url = if endpoint.is_empty() {
                    format!("https://qmsg.zendee.cn/send/{token}")
                } else {
                    endpoint.to_string()
                };
                self.send_form_or_json(&url, serde_json::json!({ "msg": format!("{title}\n{content}") }))
                    .await
            }
            "serverchan" => {
                let url = if endpoint.is_empty() {
                    format!("https://sctapi.ftqq.com/{token}.send")
                } else {
                    endpoint.to_string()
                };
                self.send_form_or_json(
                    &url,
                    serde_json::json!({ "title": title, "desp": content }),
                )
                .await
            }
            "pushplus" => {
                let url = if endpoint.is_empty() {
                    "https://www.pushplus.plus/send".to_string()
                } else {
                    endpoint.to_string()
                };
                self.send_form_or_json(
                    &url,
                    serde_json::json!({ "token": token, "title": title, "content": content }),
                )
                .await
            }
            "pushplushxtrip" => {
                let url = if endpoint.is_empty() {
                    "https://pushplus.hxtrip.com/send".to_string()
                } else {
                    endpoint.to_string()
                };
                self.send_form_or_json(
                    &url,
                    serde_json::json!({ "token": token, "title": title, "content": content }),
                )
                .await
            }
            "bark" => {
                let url = if !endpoint.is_empty() {
                    format!(
                        "{}/{}/{}",
                        endpoint.trim_end_matches('/'),
                        urlencoding(title),
                        urlencoding(content)
                    )
                } else {
                    format!(
                        "https://api.day.app/{}/{}/{}",
                        token,
                        urlencoding(title),
                        urlencoding(content)
                    )
                };
                self.send_get(&url).await
            }
            "gocqhttp" | "onebot" | "atri" => {
                if endpoint.is_empty() {
                    return missing_endpoint(channel);
                }
                self.send_form_or_json(
                    endpoint,
                    serde_json::json!({
                        "message": format!("{title}\n{content}"),
                    }),
                )
                .await
            }
            "pushdeer" => {
                let url = if endpoint.is_empty() {
                    "https://api2.pushdeer.com/message/push".to_string()
                } else {
                    endpoint.to_string()
                };
                self.send_form_or_json(
                    &url,
                    serde_json::json!({
                        "pushkey": token,
                        "text": title,
                        "desp": content,
                    }),
                )
                .await
            }
            "igot" => {
                let url = if endpoint.is_empty() {
                    format!("https://push.hellyw.com/{token}")
                } else {
                    endpoint.to_string()
                };
                self.send_form_or_json(
                    &url,
                    serde_json::json!({ "title": title, "content": content }),
                )
                .await
            }
            "telegram" => {
                let url = if endpoint.contains("api.telegram.org") {
                    endpoint.to_string()
                } else {
                    format!("https://api.telegram.org/bot{token}/sendMessage")
                };
                let chat_id = if endpoint.is_empty() || endpoint.contains("api.telegram.org") {
                    payload
                        .endpoint
                        .trim()
                        .split('/')
                        .next_back()
                        .unwrap_or("")
                } else {
                    endpoint
                };
                self.send_form_or_json(
                    &url,
                    serde_json::json!({
                        "chat_id": chat_id,
                        "text": format!("{title}\n{content}"),
                    }),
                )
                .await
            }
            "dingtalk" | "wecom" | "wecombot" | "feishu" => {
                if endpoint.is_empty() {
                    return missing_endpoint(channel);
                }
                let body = if channel == "feishu" {
                    serde_json::json!({
                        "msg_type": "text",
                        "content": { "text": format!("{title}\n{content}") },
                    })
                } else {
                    serde_json::json!({
                        "msgtype": "text",
                        "text": { "content": format!("{title}\n{content}") },
                    })
                };
                self.send_form_or_json(endpoint, body).await
            }
            "ifttt" => {
                let url = if endpoint.is_empty() {
                    format!("https://maker.ifttt.com/trigger/{title}/with/key/{token}")
                } else {
                    endpoint.to_string()
                };
                self.send_form_or_json(
                    &url,
                    serde_json::json!({
                        "value1": title,
                        "value2": content,
                    }),
                )
                .await
            }
            "discord" => {
                if endpoint.is_empty() {
                    return missing_endpoint(channel);
                }
                self.send_form_or_json(
                    endpoint,
                    serde_json::json!({
                        "content": format!("**{title}**\n{content}"),
                    }),
                )
                .await
            }
            "wxpusher" => {
                let url = if endpoint.is_empty() {
                    "https://wxpusher.zjiecode.com/api/send/message".to_string()
                } else {
                    endpoint.to_string()
                };
                self.send_form_or_json(
                    &url,
                    serde_json::json!({
                        "appToken": token,
                        "content": format!("{title}\n{content}"),
                        "summary": title,
                        "contentType": 1,
                    }),
                )
                .await
            }
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

    async fn send_form_or_json(&self, url: &str, body: serde_json::Value) -> PushResult {
        match self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => self.map_http_response(resp).await,
            Err(e) => PushResult {
                ok: false,
                code: "network_error".to_string(),
                msg: e.to_string(),
                raw: serde_json::Value::Null,
            },
        }
    }

    async fn send_get(&self, url: &str) -> PushResult {
        match self.client.get(url).send().await {
            Ok(resp) => self.map_http_response(resp).await,
            Err(e) => PushResult {
                ok: false,
                code: "network_error".to_string(),
                msg: e.to_string(),
                raw: serde_json::Value::Null,
            },
        }
    }

    async fn map_http_response(&self, resp: reqwest::Response) -> PushResult {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let raw_json: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({ "raw": text }));
        let ok = status.is_success();
        PushResult {
            ok,
            code: if ok { "ok".into() } else { "http_error".into() },
            msg: if ok {
                "ok".into()
            } else {
                format!("HTTP {}", status.as_u16())
            },
            raw: raw_json,
        }
    }
}

fn missing_endpoint(channel: &str) -> PushResult {
    PushResult {
        ok: false,
        code: "missing_endpoint".to_string(),
        msg: format!("渠道 {channel} 需要 endpoint"),
        raw: serde_json::Value::Null,
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
            channel: "not-a-real-channel".to_string(),
            title: "t".to_string(),
            content: "c".to_string(),
            ..Default::default()
        };
        let r = svc.send(&p).await;
        assert!(!r.ok);
        assert_eq!(r.code, "unsupported_channel");
    }

    #[tokio::test]
    async fn panel_channels_are_all_routed() {
        let svc = PushService::new();
        assert_eq!(SUPPORTED_CHANNELS.len(), 19);
        for ch in SUPPORTED_CHANNELS {
            let p = PushPayload {
                channel: (*ch).to_string(),
                title: "t".to_string(),
                content: "c".to_string(),
                endpoint: "http://127.0.0.1:1/push".to_string(),
                token: "tok".to_string(),
            };
            let r = svc.send(&p).await;
            assert_ne!(
                r.code, "unsupported_channel",
                "panel channel {ch} must be implemented"
            );
        }
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
