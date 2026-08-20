//! QQ 官方机器人主动通知。

mod bind;
mod error;

pub use bind::{BindPollResult, BindPollStatus, BindSessionManager, BindStartResult, SharedBindSessionManager};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::sync::{watch, Mutex};
use tokio::time::{interval_at, timeout, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

pub use error::{QqBotError, Result};

use crate::models::store::global_config::QqBotConfig;

const BIND_SUCCESS_REPLY: &str = "绑定成功，后续账号下线会通知到这里。";
const UNBIND_SUCCESS_REPLY: &str = "已解绑 QQ 下线通知。";

const TOKEN_URL: &str = "https://api.bot.qq.com/app/getAppAccessToken";
const API_BASE: &str = "https://api.bot.qq.com";
const TOKEN_REFRESH_AHEAD_SECS: u64 = 60;
const GATEWAY_READY_TIMEOUT_SECS: u64 = 15;
const RECONNECT_DELAY_SECS: u64 = 2;
const GROUP_AND_C2C_EVENT_INTENT: u64 = 1 << 25;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QqBotSendResult {
    pub ok: bool,
    pub code: String,
    pub msg: String,
}

impl QqBotSendResult {
    fn success() -> Self {
        Self { ok: true, code: "ok".into(), msg: "ok".into() }
    }

    fn failure(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self { ok: false, code: code.into(), msg: msg.into() }
    }
}

#[derive(Debug, Clone)]
struct CachedToken {
    value: String,
    expires_at: i64,
    expires_in: u64,
}

impl CachedToken {
    fn is_fresh(&self) -> bool {
        now_secs() + (TOKEN_REFRESH_AHEAD_SECS as i64) < self.expires_at
    }
}

#[derive(Debug)]
struct GatewayHandle {
    credentials_key: String,
    ready: watch::Receiver<bool>,
    cancel: CancellationToken,
}

#[derive(Debug)]
struct QqBotInner {
    http: reqwest::Client,
    token_url: String,
    api_base: String,
    tokens: Mutex<HashMap<String, CachedToken>>,
    gateways: Mutex<HashMap<String, GatewayHandle>>,
    bind_sessions: SharedBindSessionManager,
}

#[derive(Debug, Clone)]
pub struct QqBotService {
    inner: Arc<QqBotInner>,
}

impl Default for QqBotService {
    fn default() -> Self {
        Self::new()
    }
}

impl QqBotService {
    #[must_use]
    pub fn new() -> Self {
        Self::with_endpoints(TOKEN_URL, API_BASE)
    }

    #[must_use]
    pub fn with_endpoints(token_url: &str, api_base: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            inner: Arc::new(QqBotInner {
                http,
                token_url: token_url.trim_end_matches('/').to_string(),
                api_base: api_base.trim_end_matches('/').to_string(),
                tokens: Mutex::new(HashMap::new()),
                gateways: Mutex::new(HashMap::new()),
                bind_sessions: Arc::new(BindSessionManager::default()),
            }),
        }
    }

    #[must_use]
    pub fn bind_sessions(&self) -> SharedBindSessionManager {
        self.inner.bind_sessions.clone()
    }

    pub async fn send_text(
        &self,
        config: &QqBotConfig,
        title: &str,
        content: &str,
    ) -> QqBotSendResult {
        if !config.is_complete() {
            return QqBotSendResult::failure("incomplete_config", "QQ Bot 配置不完整");
        }
        if let Err(error) = self.ensure_gateway(config).await {
            return QqBotSendResult::failure("gateway_not_ready", error.to_string());
        }
        let text = if title.trim().is_empty() {
            content.trim().to_string()
        } else {
            format!("{}\n{}", title.trim(), content.trim())
        };
        self.send_message_payload(
            config,
            &config.user_openid,
            serde_json::json!({ "msg_type": 0, "content": text }),
            true,
        )
        .await
    }

    pub async fn send_qr_image(&self, config: &QqBotConfig, image_url: &str) -> QqBotSendResult {
        if !config.is_complete() {
            return QqBotSendResult::failure("incomplete_config", "QQ Bot 配置不完整");
        }
        if image_url.trim().is_empty() {
            return QqBotSendResult::failure("missing_qr_url", "二维码地址为空");
        }
        if !image_url.starts_with("http://") && !image_url.starts_with("https://") {
            return QqBotSendResult::failure(
                "invalid_qr_url",
                "QQ Bot 富媒体接口要求公网 HTTP(S) 二维码地址",
            );
        }
        if let Err(error) = self.ensure_gateway(config).await {
            return QqBotSendResult::failure("gateway_not_ready", error.to_string());
        }
        let upload = self
            .request_json(
                config,
                reqwest::Method::POST,
                &format!("/v2/users/{}/files", encode_path(&config.user_openid)),
                Some(serde_json::json!({
                    "file_type": 1,
                    "url": image_url,
                    "srv_send_msg": false,
                })),
                true,
            )
            .await;
        let (_, body) = match upload {
            Ok(value) => value,
            Err(error) => return QqBotSendResult::failure("qr_upload_failed", error.to_string()),
        };
        let file_info = body.get("file_info").and_then(serde_json::Value::as_str).unwrap_or("");
        if file_info.is_empty() {
            return api_failure(&body, "qr_upload_failed", "QQ Bot 未返回 file_info");
        }
        self.send_message_payload(
            config,
            &config.user_openid,
            serde_json::json!({
                "msg_type": 7,
                "media": { "file_info": file_info },
            }),
            true,
        )
        .await
    }

    pub async fn stop_all(&self) {
        let mut gateways = self.inner.gateways.lock().await;
        for handle in gateways.values() {
            handle.cancel.cancel();
        }
        gateways.clear();
    }

    /// 对齐全局凭据：停止已移除的 Gateway，并在后台连接 QQ Bot。
    pub fn reconcile_background(&self, config: Option<QqBotConfig>) {
        let service = self.clone();
        crate::runtime::safe_spawn::spawn_logged("qq_bot_reconcile", async move {
            let desired_key = config.as_ref().filter(|cfg| cfg.has_credentials()).map(credentials_key);
            {
                let mut gateways = service.inner.gateways.lock().await;
                gateways.retain(|_, handle| {
                    let keep = desired_key.as_ref().is_some_and(|key| key == &handle.credentials_key);
                    if !keep {
                        handle.cancel.cancel();
                    }
                    keep
                });
            }
            if let Some(config) = config.filter(|cfg| cfg.has_credentials()) {
                if let Err(error) = service.ensure_gateway(&config).await {
                    tracing::warn!("QQ Bot Gateway 初始化失败: {error}");
                }
            }
        });
    }

    async fn send_message_payload(
        &self,
        config: &QqBotConfig,
        user_openid: &str,
        payload: serde_json::Value,
        retry_auth: bool,
    ) -> QqBotSendResult {
        if user_openid.trim().is_empty() {
            return QqBotSendResult::failure("missing_user_openid", "缺少 user_openid");
        }
        let result = self
            .request_json(
                config,
                reqwest::Method::POST,
                &format!("/v2/users/{}/messages", encode_path(user_openid)),
                Some(payload.clone()),
                retry_auth,
            )
            .await;
        match result {
            Ok((status, body)) if status.is_success() && !api_body_failed(&body) => {
                QqBotSendResult::success()
            }
            Ok((_, body)) => api_failure(&body, "send_failed", "QQ Bot 发送失败"),
            Err(error) => QqBotSendResult::failure("network_error", error.to_string()),
        }
    }

    async fn request_json(
        &self,
        config: &QqBotConfig,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        retry_auth: bool,
    ) -> Result<(reqwest::StatusCode, serde_json::Value)> {
        let token = self.access_token(config, false).await?;
        let mut request = self
            .inner
            .http
            .request(method.clone(), format!("{}{}", self.inner.api_base, path))
            .header("Authorization", format!("QQBot {}", token.value))
            .header("Content-Type", "application/json; charset=utf-8");
        if let Some(body) = body.as_ref() {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|e| QqBotError::Network(e.to_string()))?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        let value =
            serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({ "message": text }));
        if retry_auth && (status == reqwest::StatusCode::UNAUTHORIZED || auth_failed(&value)) {
            self.invalidate(config).await;
            self.ensure_gateway(config).await?;
            return Box::pin(self.request_json(config, method, path, body, false)).await;
        }
        Ok((status, value))
    }

    async fn access_token(&self, config: &QqBotConfig, force: bool) -> Result<CachedToken> {
        if !config.has_credentials() {
            return Err(QqBotError::IncompleteConfig);
        }
        let key = credentials_key(config);
        let mut tokens = self.inner.tokens.lock().await;
        if !force {
            if let Some(token) = tokens.get(&key).filter(|token| token.is_fresh()) {
                return Ok(token.clone());
            }
        }
        let response = self
            .inner
            .http
            .post(&self.inner.token_url)
            .json(&serde_json::json!({
                "appId": config.app_id.trim(),
                "clientSecret": config.client_secret.trim(),
            }))
            .send()
            .await
            .map_err(|e| QqBotError::Network(e.to_string()))?;
        let status = response.status();
        let value: serde_json::Value =
            response.json().await.map_err(|e| QqBotError::InvalidResponse(e.to_string()))?;
        let access_token =
            value.get("access_token").and_then(serde_json::Value::as_str).unwrap_or("");
        if !status.is_success() || access_token.is_empty() {
            return Err(QqBotError::InvalidResponse(api_message(&value, "获取 AccessToken 失败")));
        }
        let expires_in = value
            .get("expires_in")
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            .unwrap_or(7200);
        let token = CachedToken {
            value: access_token.to_string(),
            expires_at: now_secs().saturating_add(expires_in as i64),
            expires_in,
        };
        tokens.insert(key, token.clone());
        Ok(token)
    }

    async fn invalidate(&self, config: &QqBotConfig) {
        self.inner.tokens.lock().await.remove(&credentials_key(config));
        let mut gateways = self.inner.gateways.lock().await;
        if let Some(handle) = gateways.remove(config.app_id.trim()) {
            handle.cancel.cancel();
        }
    }

    async fn ensure_gateway(&self, config: &QqBotConfig) -> Result<()> {
        let credentials_key = credentials_key(config);
        let mut ready = {
            let mut gateways = self.inner.gateways.lock().await;
            if let Some(handle) = gateways.get(config.app_id.trim()) {
                if handle.credentials_key == credentials_key {
                    handle.ready.clone()
                } else {
                    handle.cancel.cancel();
                    gateways.remove(config.app_id.trim());
                    self.spawn_gateway_locked(&mut gateways, config.clone(), credentials_key)
                }
            } else {
                self.spawn_gateway_locked(&mut gateways, config.clone(), credentials_key)
            }
        };
        if *ready.borrow() {
            return Ok(());
        }
        timeout(Duration::from_secs(GATEWAY_READY_TIMEOUT_SECS), ready.wait_for(|value| *value))
            .await
            .map_err(|_| QqBotError::Gateway("等待 READY 超时".into()))?
            .map_err(|_| QqBotError::Gateway("Gateway 任务已停止".into()))?;
        Ok(())
    }

    fn spawn_gateway_locked(
        &self,
        gateways: &mut HashMap<String, GatewayHandle>,
        config: QqBotConfig,
        credentials_key: String,
    ) -> watch::Receiver<bool> {
        let app_id = config.app_id.trim().to_string();
        let (ready_tx, ready_rx) = watch::channel(false);
        let cancel = CancellationToken::new();
        let service = self.clone();
        let task_cancel = cancel.clone();
        crate::runtime::safe_spawn::spawn_logged("qq_bot_gateway", async move {
            service.gateway_supervisor(config, ready_tx, task_cancel).await;
        });
        gateways.insert(app_id, GatewayHandle { credentials_key, ready: ready_rx.clone(), cancel });
        ready_rx
    }

    async fn gateway_supervisor(
        &self,
        config: QqBotConfig,
        ready: watch::Sender<bool>,
        cancel: CancellationToken,
    ) {
        loop {
            if cancel.is_cancelled() {
                break;
            }
            let _ = ready.send(false);
            if let Err(error) = self.gateway_once(&config, &ready, &cancel).await {
                tracing::warn!(app_id = %config.app_id, "QQ Bot Gateway 断开: {error}");
            }
            let _ = ready.send(false);
            tokio::select! {
                () = cancel.cancelled() => break,
                () = tokio::time::sleep(Duration::from_secs(RECONNECT_DELAY_SECS)) => {}
            }
        }
    }

    async fn gateway_once(
        &self,
        config: &QqBotConfig,
        ready: &watch::Sender<bool>,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let token = self.access_token(config, false).await?;
        let response = self
            .inner
            .http
            .get(format!("{}/gateway", self.inner.api_base))
            .header("Authorization", format!("QQBot {}", token.value))
            .send()
            .await
            .map_err(|e| QqBotError::Network(e.to_string()))?;
        let gateway: serde_json::Value =
            response.json().await.map_err(|e| QqBotError::InvalidResponse(e.to_string()))?;
        let url = gateway.get("url").and_then(serde_json::Value::as_str).unwrap_or("");
        if url.is_empty() {
            return Err(QqBotError::InvalidResponse(api_message(&gateway, "缺少 Gateway URL")));
        }
        let (mut socket, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| QqBotError::Gateway(e.to_string()))?;
        let hello = timeout(Duration::from_secs(10), socket.next())
            .await
            .map_err(|_| QqBotError::Gateway("等待 Hello 超时".into()))?
            .ok_or_else(|| QqBotError::Gateway("Gateway 已关闭".into()))?
            .map_err(|e| QqBotError::Gateway(e.to_string()))?;
        let hello = message_json(hello)?;
        if hello.get("op").and_then(serde_json::Value::as_i64) != Some(10) {
            return Err(QqBotError::Gateway("首包不是 Hello".into()));
        }
        let heartbeat_ms = hello
            .pointer("/d/heartbeat_interval")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(41_250)
            .max(1_000);
        socket
            .send(Message::Text(
                serde_json::json!({
                    "op": 2,
                    "d": {
                        "token": format!("QQBot {}", token.value),
                        "intents": GROUP_AND_C2C_EVENT_INTENT,
                        "shard": [0, 1],
                        "properties": {
                            "$os": std::env::consts::OS,
                            "$browser": "qq-farm-rust",
                            "$device": "qq-farm-rust",
                        }
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .map_err(|e| QqBotError::Gateway(e.to_string()))?;

        let mut sequence: Option<i64> = None;
        let mut heartbeat = interval_at(
            Instant::now() + Duration::from_millis(heartbeat_ms),
            Duration::from_millis(heartbeat_ms),
        );
        let refresh_after = token.expires_in.saturating_sub(TOKEN_REFRESH_AHEAD_SECS).max(1);
        let refresh = tokio::time::sleep(Duration::from_secs(refresh_after));
        tokio::pin!(refresh);
        loop {
            tokio::select! {
                () = cancel.cancelled() => return Ok(()),
                () = &mut refresh => {
                    self.inner.tokens.lock().await.remove(&credentials_key(config));
                    return Ok(());
                }
                _ = heartbeat.tick() => {
                    socket
                        .send(Message::Text(
                            serde_json::json!({ "op": 1, "d": sequence }).to_string().into(),
                        ))
                        .await
                        .map_err(|e| QqBotError::Gateway(e.to_string()))?;
                }
                message = socket.next() => {
                    let message = message
                        .ok_or_else(|| QqBotError::Gateway("Gateway 已关闭".into()))?
                        .map_err(|e| QqBotError::Gateway(e.to_string()))?;
                    if message.is_close() {
                        return Ok(());
                    }
                    let value = message_json(message)?;
                    if let Some(seq) = value.get("s").and_then(serde_json::Value::as_i64) {
                        sequence = Some(seq);
                    }
                    match value.get("op").and_then(serde_json::Value::as_i64) {
                        Some(0) => {
                            let event = value.get("t").and_then(serde_json::Value::as_str).unwrap_or("");
                            if event == "READY" || event == "RESUMED" {
                                let _ = ready.send(true);
                            } else if event == "C2C_MESSAGE_CREATE" {
                                let service = self.clone();
                                let cfg = config.clone();
                                let payload = value.get("d").cloned().unwrap_or_default();
                                crate::runtime::safe_spawn::spawn_logged("qq_bot_c2c", async move {
                                    service.handle_c2c_message(&cfg, payload).await;
                                });
                            }
                        }
                        Some(7 | 9) => return Ok(()),
                        _ => {}
                    }
                }
            }
        }
    }

    async fn handle_c2c_message(&self, config: &QqBotConfig, payload: serde_json::Value) {
        let user_openid = payload
            .pointer("/author/user_openid")
            .or_else(|| payload.pointer("/author/id"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let nickname = payload
            .pointer("/author/username")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let content = payload.get("content").and_then(serde_json::Value::as_str).unwrap_or("");
        let msg_id = payload.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
        if user_openid.is_empty() {
            return;
        }

        let result = self
            .inner
            .bind_sessions
            .complete_from_message(user_openid, nickname, content);
        let Some((username, binding)) = result else {
            return;
        };

        if binding.is_bound() {
            crate::models::store::global_config::apply_qq_bot_binding(&username, binding.clone());
            self.inner.bind_sessions.register_binding(&username, &binding);
            if !msg_id.is_empty() {
                let _ = self
                    .reply_c2c_text(config, user_openid, msg_id, BIND_SUCCESS_REPLY)
                    .await;
            } else {
                let _ = self.send_text_to_user(config, user_openid, BIND_SUCCESS_REPLY).await;
            }
            tracing::info!(username = %username, "QQ Bot 绑定成功");
        } else {
            crate::models::store::global_config::clear_qq_bot_binding(&username);
            self.inner.bind_sessions.clear_user(&username);
            if !msg_id.is_empty() {
                let _ = self
                    .reply_c2c_text(config, user_openid, msg_id, UNBIND_SUCCESS_REPLY)
                    .await;
            }
            tracing::info!(username = %username, "QQ Bot 已解绑");
        }
    }

    async fn reply_c2c_text(
        &self,
        config: &QqBotConfig,
        user_openid: &str,
        msg_id: &str,
        content: &str,
    ) -> QqBotSendResult {
        self.send_message_payload(
            config,
            user_openid,
            serde_json::json!({
                "msg_type": 0,
                "content": content,
                "msg_id": msg_id,
                "msg_seq": 1,
            }),
            true,
        )
        .await
    }

    async fn send_text_to_user(
        &self,
        config: &QqBotConfig,
        user_openid: &str,
        content: &str,
    ) -> QqBotSendResult {
        self.send_message_payload(
            config,
            user_openid,
            serde_json::json!({ "msg_type": 0, "content": content }),
            true,
        )
        .await
    }
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

fn credentials_key(config: &QqBotConfig) -> String {
    format!("{}\0{}", config.app_id.trim(), config.client_secret.trim())
}

fn encode_path(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.trim().as_bytes()).collect()
}

fn message_json(message: Message) -> Result<serde_json::Value> {
    let text = message.into_text().map_err(|e| QqBotError::InvalidResponse(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| QqBotError::InvalidResponse(e.to_string()))
}

fn auth_failed(value: &serde_json::Value) -> bool {
    matches!(
        value.get("code").and_then(serde_json::Value::as_i64),
        Some(11242 | 11243 | 11251 | 11261 | 11275)
    )
}

fn api_body_failed(value: &serde_json::Value) -> bool {
    value
        .get("code")
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .is_some_and(|code| code != 0)
        || value.get("error").is_some()
}

fn api_message(value: &serde_json::Value, fallback: &str) -> String {
    value
        .get("message")
        .or_else(|| value.get("msg"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn api_failure(
    value: &serde_json::Value,
    fallback_code: &str,
    fallback_msg: &str,
) -> QqBotSendResult {
    let code = value
        .get("code")
        .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
        .unwrap_or_else(|| fallback_code.to_string());
    QqBotSendResult::failure(code, api_message(value, fallback_msg))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::extract::ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade};
    use axum::extract::State;
    use axum::routing::{get, post};
    use axum::{Json, Router};

    use super::*;

    #[test]
    fn config_completeness_and_path_encoding() {
        let mut config = QqBotConfig::default();
        assert!(!config.is_complete());
        config.app_id = "app".into();
        config.client_secret = "secret".into();
        config.user_openid = "user/open id".into();
        assert!(config.is_complete());
        assert_eq!(encode_path(&config.user_openid), "user%2Fopen+id");
    }

    #[test]
    fn recognizes_auth_errors() {
        assert!(auth_failed(&serde_json::json!({ "code": 11243 })));
        assert!(!auth_failed(&serde_json::json!({ "code": 304050 })));
    }

    #[derive(Clone)]
    struct MockState {
        base_url: String,
        token_requests: Arc<AtomicUsize>,
        messages: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    async fn token(State(state): State<MockState>) -> Json<serde_json::Value> {
        state.token_requests.fetch_add(1, Ordering::SeqCst);
        Json(serde_json::json!({ "access_token": "test-token", "expires_in": 7200 }))
    }

    async fn gateway(State(state): State<MockState>) -> Json<serde_json::Value> {
        Json(
            serde_json::json!({ "url": format!("{}/ws", state.base_url.replace("http://", "ws://")) }),
        )
    }

    async fn ws(upgrade: WebSocketUpgrade) -> axum::response::Response {
        upgrade.on_upgrade(mock_gateway)
    }

    async fn mock_gateway(mut socket: WebSocket) {
        let _ = socket
            .send(AxumMessage::Text(
                serde_json::json!({ "op": 10, "d": { "heartbeat_interval": 1000 } })
                    .to_string()
                    .into(),
            ))
            .await;
        if socket.recv().await.is_some() {
            let _ = socket
                .send(AxumMessage::Text(
                    serde_json::json!({ "op": 0, "s": 1, "t": "READY", "d": {} })
                        .to_string()
                        .into(),
                ))
                .await;
        }
        while let Some(Ok(message)) = socket.recv().await {
            if let AxumMessage::Text(text) = message {
                let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                if value.get("op").and_then(serde_json::Value::as_i64) == Some(1) {
                    let _ = socket
                        .send(AxumMessage::Text(
                            serde_json::json!({ "op": 11, "d": null }).to_string().into(),
                        ))
                        .await;
                }
            }
        }
    }

    async fn capture_message(
        State(state): State<MockState>,
        Json(body): Json<serde_json::Value>,
    ) -> Json<serde_json::Value> {
        state.messages.lock().await.push(body);
        Json(serde_json::json!({ "id": "message-id" }))
    }

    async fn upload_file() -> Json<serde_json::Value> {
        Json(serde_json::json!({ "file_info": "file-info", "ttl": 300 }))
    }

    #[tokio::test]
    async fn sends_text_and_qr_through_official_flow() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let state = MockState {
            base_url: base_url.clone(),
            token_requests: Arc::new(AtomicUsize::new(0)),
            messages: Arc::new(Mutex::new(Vec::new())),
        };
        let app = Router::new()
            .route("/token", post(token))
            .route("/gateway", get(gateway))
            .route("/ws", get(ws))
            .route("/v2/users/{openid}/messages", post(capture_message))
            .route("/v2/users/{openid}/files", post(upload_file))
            .with_state(state.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let service = QqBotService::with_endpoints(&format!("{base_url}/token"), &base_url);
        let config = QqBotConfig {
            app_id: "app".into(),
            client_secret: "secret".into(),
            user_openid: "user".into(),
        };
        assert!(service.send_text(&config, "title", "content").await.ok);
        assert!(service.send_qr_image(&config, "https://example.com/qr.png").await.ok);
        assert_eq!(state.token_requests.load(Ordering::SeqCst), 1);
        let messages = state.messages.lock().await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["msg_type"], 0);
        assert_eq!(messages[1]["msg_type"], 7);
        service.stop_all().await;
    }
}
