//! 网关连接管理。
//!
//! 状态机：`Disconnected` → `Connecting` → `Login` → `Online` → `Disconnected`
//!
//! 负责：
//! - 构造 WS URL（带 code / platform / os / ver query）
//! - 连接、登录成功切换状态
//! - 接收循环：调用 codec 解密 + dispatch
//! - 异步 sendMsg（关联 clientSeq）
//! - 主动 / 被动断开清理
//!
//! 阶段 1A 范围：基础连接 + 状态机 + send/recv + sendMsgAsync 机制。
//! 登录流程（ACE runtime / WASM 握手）留到阶段 1B 业务模块。

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::network::client::{ConnectOptions, WsClient};
use crate::network::encryptor::Encryptor;
use crate::network::error::{NetworkError, Result};
use crate::network::frame::{FrameBuilder, FrameParser};
use crate::network::notify::NotifyEvent;
use crate::network::request::RequestManager;
use crate::proto::generated::gamepb::userpb::{DeviceInfo, HeartbeatReply, HeartbeatRequest, LoginReply, LoginRequest, ReportData};
use crate::proto::generated::gatepb::MessageType;

/// 连接阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionPhase {
    /// 未连接
    Disconnected,
    /// 正在连接
    Connecting,
    /// 等待登录响应
    Login,
    /// 已登录，可收发业务消息
    Online,
    /// 正在关闭
    Closing,
}

/// 网关配置
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// 网关 URL（不含 query）
    pub server_url: String,
    /// 平台（"android" / "ios" / "windows"）
    pub platform: String,
    /// 操作系统描述
    pub os: String,
    /// 客户端版本
    pub client_version: String,
    /// 一次性登录 code
    pub auth_code: String,
    /// 自定义 HTTP headers
    pub headers: HashMap<String, String>,
}

impl GatewayConfig {
    /// 构造完整 WS URL（含 query）
    pub fn build_ws_url(&self) -> String {
        let separator = if self.server_url.contains('?') { '&' } else { '?' };
        format!(
            "{}{}platform={}&os={}&ver={}&code={}",
            self.server_url,
            separator,
            urlencoding(&self.platform),
            urlencoding(&self.os),
            urlencoding(&self.client_version),
            urlencoding(&self.auth_code),
        )
    }
}

fn urlencoding(s: &str) -> String {
    // 简单 URL 编码（只处理 ASCII 非字母数字）
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// 网关连接（对外接口）
pub struct Gateway {
    inner: Arc<Inner>,
}

struct Inner {
    config: GatewayConfig,
    phase: RwLock<ConnectionPhase>,
    client_seq: AtomicI64,
    server_seq: AtomicI64,
    requests: RequestManager,
    /// 加密器（外部注入）
    encryptor: Arc<dyn Encryptor>,
    /// 收到的 Notify 事件订阅者
    notify_subscribers: RwLock<Vec<mpsc::Sender<NotifyEvent>>>,
    /// WS 发送端（connect 时设置）
    ws_sender: parking_lot::Mutex<Option<mpsc::Sender<Vec<u8>>>>,
}

impl Gateway {
    /// 创建（不连接）
    #[must_use]
    pub fn new(config: GatewayConfig, encryptor: Arc<dyn Encryptor>) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                phase: RwLock::new(ConnectionPhase::Disconnected),
                client_seq: AtomicI64::new(1),
                server_seq: AtomicI64::new(0),
                requests: RequestManager::new(),
                encryptor,
                notify_subscribers: RwLock::new(Vec::new()),
                ws_sender: parking_lot::Mutex::new(None),
            }),
        }
    }

    /// 当前阶段
    #[must_use]
    pub fn phase(&self) -> ConnectionPhase {
        *self.inner.phase.read()
    }

    /// 连接到服务器（不含登录）
    pub async fn connect(&self) -> Result<WsClient> {
        {
            let mut phase = self.inner.phase.write();
            if *phase != ConnectionPhase::Disconnected {
                return Err(NetworkError::Phase(format!("already in {phase:?}")));
            }
            *phase = ConnectionPhase::Connecting;
        }

        let url = self.inner.config.build_ws_url();
        let mut options = ConnectOptions::default();
        for (k, v) in &self.inner.config.headers {
            options.headers.insert(k.clone(), v.clone());
        }
        let (client, rx) = WsClient::connect(&url, options).await?;

        // 创建 frame 发送 channel（业务调用 request() 通过这里发）
        let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(64);
        *self.inner.ws_sender.lock() = Some(frame_tx);

        // 启动一个 task：从 channel 读 frame 通过 WsClient 发
        let client_for_sender = client.clone();
        tokio::spawn(async move {
            while let Some(frame) = frame_rx.recv().await {
                if client_for_sender.send(&frame).await.is_err() {
                    break;
                }
            }
        });

        // 更新阶段为 Login（待登录响应）
        *self.inner.phase.write() = ConnectionPhase::Login;

        // 启动接收 dispatch loop
        let inner = self.inner.clone();
        tokio::spawn(dispatch_loop(rx, inner));

        Ok(client)
    }

    /// 主动断开
    pub async fn close(&self, ws: &WsClient) -> Result<()> {
        {
            let mut phase = self.inner.phase.write();
            *phase = ConnectionPhase::Closing;
        }
        ws.close().await?;
        // 拒绝所有待处理请求
        let n = self.inner.requests.reject_all();
        if n > 0 {
            tracing::warn!(count = n, "rejected pending requests on close");
        }
        *self.inner.phase.write() = ConnectionPhase::Disconnected;
        Ok(())
    }

    /// 发送一个业务请求
    ///
    /// 返回 `client_seq` + 响应 receiver
    pub fn begin_request(&self, service: &str, method: &str) -> (i64, oneshot::Receiver<crate::network::request::Response>) {
        self.inner.requests.call(service, method)
    }

    /// 编码一个请求帧（业务层负责发送）
    pub fn encode_request(
        &self,
        service: &str,
        method: &str,
        body: &[u8],
        client_seq: i64,
        token: &str,
    ) -> Result<Vec<u8>> {
        // body 加密（如果非空）
        let encrypted_body = if body.is_empty() {
            Vec::new()
        } else {
            self.inner
                .encryptor
                .encrypt(body)
                .map_err(|e| NetworkError::Encrypt(e.to_string()))?
        };
        let frame = FrameBuilder::request(service, method)
            .with_client_seq(client_seq)
            .with_server_seq(self.inner.server_seq.load(Ordering::SeqCst))
            .with_body(encrypted_body)
            .with_token(token);
        frame.encode().map_err(|e| NetworkError::Frame(format!("encode: {e}")))
    }

    /// 高阶 API：发请求 + 等响应（带超时）
    ///
    /// 业务层最常用：`let resp = gateway.request("gamepb.plantpb.PlantService", "AllLands", &body).await?;`
    ///
    /// 流程：
    /// 1. 分配 client_seq
    /// 2. encode frame（body 加密）
    /// 3. 通过 WsClient 发送
    /// 4. 等 receiver，timeout = `timeout_ms`
    ///
    /// # Errors
    /// - 阶段错误（未在 Online）
    /// - 帧编码失败
    /// - 发送失败
    /// - 超时
    /// - 网关错误（error_code != 0）
    pub async fn request(
        &self,
        service: &str,
        method: &str,
        body: &[u8],
        timeout_ms: u64,
    ) -> Result<Vec<u8>> {
        // 1. 阶段检查
        {
            let phase = *self.inner.phase.read();
            if phase != ConnectionPhase::Online {
                return Err(NetworkError::Phase(format!(
                    "request requires Online, current: {phase:?}"
                )));
            }
        }

        // 2. 分配 seq
        let (seq, rx) = self.inner.requests.call(service, method);

        // 3. 编码 frame
        let frame_bytes = self.encode_request(service, method, body, seq, "")?;
        let _ = body; // 抑制未使用警告

        // 4. 发送（需要 WsClient handle —— 通过 channel 找到）
        // —— 简化：直接通过 Gateway 内部保存的 ws_sender
        let ws_tx = self
            .inner
            .ws_sender
            .lock()
            .as_ref()
            .ok_or_else(|| NetworkError::Phase("ws not connected".into()))?
            .clone();
        ws_tx
            .send(frame_bytes)
            .await
            .map_err(|_| NetworkError::WebSocket("ws sender closed".into()))?;

        // 5. 等响应（带超时）
        match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(resp)) => Ok(resp.body),
            Ok(Err(_)) => Err(NetworkError::Phase("response channel cancelled".into())),
            Err(_) => {
                // 超时 —— 清理 pending
                self.inner.requests.cancel(seq);
                Err(NetworkError::Timeout {
                    client_seq: seq,
                    service_name: service.to_string(),
                    method_name: method.to_string(),
                })
            }
        }
    }

    /// 订阅 Notify 事件
    pub fn subscribe_notify(&self) -> mpsc::Receiver<NotifyEvent> {
        let (tx, rx) = mpsc::channel(32);
        self.inner.notify_subscribers.write().push(tx);
        rx
    }

    /// 标记登录完成（阶段 1A 外部调用；阶段 1B 由业务模块在收到登录响应后调用）
    pub fn mark_online(&self) {
        *self.inner.phase.write() = ConnectionPhase::Online;
    }

    /// 完整登录流程：发 LoginRequest → 等 LoginReply → bindUser → mark_online
    ///
    /// 1:1 对应原 `network.ts:sendLogin()`
    ///
    /// # Arguments
    /// - `device_info`: 客户端版本 / 系统 / 屏幕等
    /// - `report_data`: 上报数据（minigame_channel / minigame_platid）
    /// - `tsdk`: TSDK runtime（用于 bindUser）
    pub async fn login(
        &self,
        device_info: &DeviceInfo,
        report_data: &ReportData,
        tsdk: &Arc<crate::crypto::tsdk::TsdkRuntime>,
    ) -> Result<LoginReply> {
        // 1. 阶段检查：必须在 Login 阶段
        {
            let phase = *self.inner.phase.read();
            if phase != ConnectionPhase::Login {
                return Err(NetworkError::Phase(format!(
                    "login requires Login phase, current: {phase:?}"
                )));
            }
        }

        // 2. 构造 LoginRequest
        let req = LoginRequest {
            sharer_id: 0,
            sharer_open_id: String::new(),
            device_info: Some(device_info.clone()),
            share_cfg_id: 0,
            scene_id: "1234567".to_string(),
            report_data: Some(report_data.clone()),
        };
        let body = prost::Message::encode_to_vec(&req);

        // 3. 发请求（阶段会自动从 Login → Online 在收到响应后）
        let reply_bytes = self
            .request("gamepb.userpb.UserService", "Login", &body, 15_000)
            .await?;

        // 4. 解码 LoginReply
        let reply = LoginReply::decode(reply_bytes.as_slice())
            .map_err(|e| NetworkError::Frame(format!("decode LoginReply: {e}")))?;

        // 5. 校验 basic 字段
        let Some(basic) = &reply.basic else {
            return Err(NetworkError::Frame("LoginReply 缺少 basic".to_string()));
        };

        // 6. bindUser(open_id) —— 客户端安全数据
        if !basic.open_id.is_empty() {
            if let Err(e) = tsdk.bind_user(&basic.open_id) {
                tracing::warn!(error = %e, "TSDK bindUser 失败");
            }
        }

        // 7. mark_online
        self.mark_online();

        // 8. 同步服务器时间
        if reply.time_now_millis > 0 {
            crate::utils::time::sync_server_time(reply.time_now_millis);
        }

        // 9. 日志
        let gid = basic.gid;
        let name = if basic.name.is_empty() { "未知".to_string() } else { basic.name.clone() };
        let level = basic.level;
        let gold = basic.gold;
        tracing::info!(
            gid = gid,
            name = %name,
            level = level,
            gold = gold,
            "登录成功"
        );

        Ok(reply)
    }

    /// 发 Heartbeat 请求（同步服务器时间 + 维持连接）
    pub async fn heartbeat(&self, gid: i64, client_version: &str) -> Result<HeartbeatReply> {
        let req = HeartbeatRequest {
            gid,
            client_version: client_version.to_string(),
        };
        let body = prost::Message::encode_to_vec(&req);
        let reply_bytes = self
            .request("gamepb.userpb.UserService", "Heartbeat", &body, 5_000)
            .await?;
        let reply = HeartbeatReply::decode(reply_bytes.as_slice())
            .map_err(|e| NetworkError::Frame(format!("decode HeartbeatReply: {e}")))?;
        if reply.server_time > 0 {
            crate::utils::time::sync_server_time(reply.server_time);
        }
        Ok(reply)
    }

    /// 拿服务器时间（防改时间作弊）
    pub fn now_ms(&self) -> i64 {
        crate::utils::time::now_ms()
    }
}

/// 接收 dispatch loop
async fn dispatch_loop(mut rx: mpsc::Receiver<crate::network::client::ReceivedFrame>, inner: Arc<Inner>) {
    while let Some(frame) = rx.recv().await {
        // 1. 解析外层 GateMessage
        let parsed = match FrameParser::parse(&frame.bytes) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "frame decode failed");
                continue;
            }
        };

        // 2. 更新 server_seq
        let server_seq = parsed.server_seq();
        if server_seq > inner.server_seq.load(Ordering::SeqCst) {
            inner.server_seq.store(server_seq, Ordering::SeqCst);
        }

        // 3. 分发
        match parsed.message_type() {
            Some(MessageType::Response) => {
                handle_response(&inner, &parsed);
            }
            Some(MessageType::Notify) => {
                handle_notify(&inner, &parsed);
            }
            _ => {
                tracing::debug!(
                    service = parsed.service_name(),
                    method = parsed.method_name(),
                    "received non-response/notify message"
                );
            }
        }
    }
    tracing::debug!("dispatch loop exited");
}

fn handle_response(inner: &Arc<Inner>, parsed: &FrameParser) {
    let client_seq = parsed.client_seq();
    let error_code = parsed.error_code();

    if error_code != 0 {
        // 网关错误
        let err = NetworkError::Gateway {
            code: error_code,
            service_name: parsed.service_name().to_string(),
            method_name: parsed.method_name().to_string(),
            error_message: parsed.error_message().to_string(),
            client_seq,
        };
        let _ = inner.requests.fail(client_seq, err);
    } else {
        // 业务成功
        let body_encrypted = parsed.body();
        let body_plain = match inner.encryptor.decrypt(body_encrypted) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "decrypt body failed");
                return;
            }
        };
        inner.requests.complete(client_seq, body_plain, parsed.server_seq());
    }
}

fn handle_notify(inner: &Arc<Inner>, parsed: &FrameParser) {
    // body 里是 EventMessage，已加密
    let body_encrypted = parsed.body();
    let body_plain = match inner.encryptor.decrypt(body_encrypted) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "decrypt notify body failed");
            return;
        }
    };
    let event_msg = match crate::proto::generated::gatepb::EventMessage::decode(&*body_plain) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "decode EventMessage failed");
            return;
        }
    };
    let event = crate::network::notify::parse_event(&event_msg);
    // 广播给所有订阅者
    let subs = inner.notify_subscribers.read().clone();
    for tx in subs {
        if tx.try_send(event.clone()).is_err() {
            // channel 满或已关闭 —— 静默丢弃
        }
    }
}

// 引入 decode trait
use prost::Message as _;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_ws_url() {
        let cfg = GatewayConfig {
            server_url: "wss://gate.example.com/ws".into(),
            platform: "android".into(),
            os: "linux".into(),
            client_version: "1.0.0".into(),
            auth_code: "abc123".into(),
            headers: HashMap::new(),
        };
        let url = cfg.build_ws_url();
        assert!(url.contains("platform=android"));
        assert!(url.contains("os=linux"));
        assert!(url.contains("ver=1.0.0"));
        assert!(url.contains("code=abc123"));
    }

    #[test]
    fn urlencoding_spaces() {
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("a-b_c.d~e"), "a-b_c.d~e");
    }
}
