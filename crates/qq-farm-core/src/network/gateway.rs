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
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::{mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore};

use crate::network::client::{ConnectOptions, WsClient};
use crate::network::encryptor::Encryptor;
use crate::network::error::{NetworkError, Result};
use crate::network::frame::{FrameBuilder, FrameParser};
use crate::network::notify::NotifyEvent;
use crate::network::request::RequestManager;
use crate::proto::generated::gamepb::userpb::{
    DeviceInfo, HeartbeatReply, HeartbeatRequest, LoginReply, LoginRequest, ReportData,
};
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

fn header_missing(headers: &HashMap<String, String>, name: &str) -> bool {
    !headers.keys().any(|k| k.eq_ignore_ascii_case(name))
}

fn rpc_phase_ok(phase: ConnectionPhase, require_online: bool) -> Result<()> {
    if require_online {
        if phase != ConnectionPhase::Online {
            return Err(NetworkError::Phase(format!(
                "request requires Online, current: {phase:?}"
            )));
        }
    } else if !matches!(phase, ConnectionPhase::Login | ConnectionPhase::Online) {
        return Err(NetworkError::Phase(format!("connection not open: {phase:?}")));
    }
    Ok(())
}

#[allow(dead_code)]
fn origin_from_ws_url(server_url: &str) -> String {
    let (scheme, rest) = if let Some(rest) = server_url.strip_prefix("wss://") {
        ("https", rest)
    } else if let Some(rest) = server_url.strip_prefix("ws://") {
        ("http", rest)
    } else {
        return "https://gate-obt.nqf.qq.com".to_string();
    };
    let host = rest.split('/').next().unwrap_or("gate-obt.nqf.qq.com");
    if host.is_empty() {
        "https://gate-obt.nqf.qq.com".to_string()
    } else {
        format!("{scheme}://{host}")
    }
}

fn apply_default_ws_headers(headers: &mut HashMap<String, String>, _server_url: &str) {
    if header_missing(headers, "Origin") {
        // 对齐 network.ts：Origin 固定为游戏网关，不随自定义 serverUrl 变
        headers.insert("Origin".to_string(), "https://gate-obt.nqf.qq.com".to_string());
    }
    if header_missing(headers, "User-Agent") {
        let ua = crate::config::get_runtime_config().device_info.user_agent;
        let ua =
            if ua.is_empty() { crate::config::DeviceInfo::windows_pc().user_agent } else { ua };
        headers.insert("User-Agent".to_string(), ua);
    }
}

/// 网关连接（对外接口）
pub struct Gateway {
    inner: Arc<Inner>,
}

struct Inner {
    config: GatewayConfig,
    phase: RwLock<ConnectionPhase>,
    server_seq: AtomicI64,
    requests: RequestManager,
    /// 加密器（外部注入）
    encryptor: Arc<dyn Encryptor>,
    /// 收到的 Notify 事件订阅者
    notify_subscribers: RwLock<Vec<mpsc::Sender<NotifyEvent>>>,
    /// WS 发送端（connect 时设置）
    ws_sender: parking_lot::Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    /// 当前会话是否已结束（dispatch 退出 / 主动断开）
    session_end: watch::Sender<bool>,
    /// 会话结束原因（心跳超时 / kickout / ws_close 等），供 worker 日志对齐 TS source
    disconnect_reason: parking_lot::Mutex<Option<String>>,
    /// 最近一次收到任意 WS 帧的时间（ms）。大包 GetAll 下载期间心跳 RPC 可能超时，但连接仍活。
    last_rx_ms: AtomicI64,
    /// 业务 RPC 并发槽（对齐 bot 5 in-flight / 100 排队）。Heartbeat 不占槽。
    rpc_slots: Arc<Semaphore>,
    rpc_queued: AtomicUsize,
}

impl Gateway {
    /// 创建（不连接）
    #[must_use]
    pub fn new(config: GatewayConfig, encryptor: Arc<dyn Encryptor>) -> Self {
        let (session_end, _) = watch::channel(false);
        Self {
            inner: Arc::new(Inner {
                config,
                phase: RwLock::new(ConnectionPhase::Disconnected),
                server_seq: AtomicI64::new(0),
                requests: RequestManager::new(),
                encryptor,
                notify_subscribers: RwLock::new(Vec::new()),
                ws_sender: parking_lot::Mutex::new(None),
                session_end,
                disconnect_reason: parking_lot::Mutex::new(None),
                last_rx_ms: AtomicI64::new(0),
                rpc_slots: Arc::new(Semaphore::new(crate::constants::MAX_IN_FLIGHT_REQUESTS)),
                rpc_queued: AtomicUsize::new(0),
            }),
        }
    }

    /// 当前阶段
    #[must_use]
    pub fn phase(&self) -> ConnectionPhase {
        *self.inner.phase.read()
    }

    /// 当前连接使用的平台（qq / wx），对齐 TS worker 内 `CONFIG.platform`
    #[must_use]
    pub fn platform(&self) -> String {
        self.inner.config.platform.clone()
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

        let _ = self.inner.session_end.send(false);
        *self.inner.disconnect_reason.lock() = None;
        self.inner.last_rx_ms.store(0, Ordering::Release);

        let url = self.inner.config.build_ws_url();
        tracing::info!(
            platform = %self.inner.config.platform,
            os = %self.inner.config.os,
            ver = %self.inner.config.client_version,
            "farm gateway dial"
        );
        let mut options = ConnectOptions::default();
        for (k, v) in &self.inner.config.headers {
            options.headers.insert(k.clone(), v.clone());
        }
        apply_default_ws_headers(&mut options.headers, &self.inner.config.server_url);
        let (client, rx) = match WsClient::connect(&url, options).await {
            Ok(v) => v,
            Err(e) => {
                *self.inner.phase.write() = ConnectionPhase::Disconnected;
                let _ = self.inner.session_end.send(true);
                return Err(e);
            }
        };

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
        self.mark_session_ended();
        Ok(())
    }

    /// 被动/超时断开：结束会话，worker 主循环据此退出，不再用旧 Code 重连
    pub fn force_disconnect(&self) {
        self.force_disconnect_with_reason("ws_close");
    }

    /// 带原因断开（对齐 TS `finalizeConnection({ source })`）
    pub fn force_disconnect_with_reason(&self, reason: &str) {
        {
            let mut guard = self.inner.disconnect_reason.lock();
            if guard.is_none() {
                *guard = Some(reason.to_string());
            }
        }
        self.mark_session_ended();
    }

    /// 取出并清空断开原因；无显式原因时视为远端 `ws_close`
    pub fn take_disconnect_reason(&self) -> String {
        self.inner.disconnect_reason.lock().take().unwrap_or_else(|| "ws_close".to_string())
    }

    /// 当前会话结束后返回（dispatch 退出或 `force_disconnect`）
    pub async fn wait_session_end(&self) {
        let mut rx = self.inner.session_end.subscribe();
        if *rx.borrow() {
            return;
        }
        while rx.changed().await.is_ok() {
            if *rx.borrow() {
                return;
            }
        }
    }

    fn mark_session_ended(&self) {
        end_session(&self.inner, None);
    }

    /// 发送一个业务请求
    ///
    /// 返回 `client_seq` + 响应 receiver
    pub fn begin_request(
        &self,
        service: &str,
        method: &str,
    ) -> (
        i64,
        oneshot::Receiver<std::result::Result<crate::network::request::Response, NetworkError>>,
    ) {
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
            self.inner.encryptor.encrypt(body).map_err(|e| NetworkError::Encrypt(e.to_string()))?
        };
        let frame = FrameBuilder::request(service, method)
            .with_client_seq(client_seq)
            .with_server_seq(self.inner.server_seq.load(Ordering::SeqCst))
            .with_body(encrypted_body)
            .with_token(token);
        frame.encode().map_err(|e| NetworkError::Frame(format!("encode: {e}")))
    }

    /// 高阶 API：发请求 + 等响应，直到回包或会话断开。
    ///
    /// 对齐 Go 本田/巡查：不在业务 RPC 上套 10s/20s 硬切。登录走 [`login`]，心跳走 [`request_with_timeout`]。
    pub async fn request(&self, service: &str, method: &str, body: &[u8]) -> Result<Vec<u8>> {
        self.send_rpc(service, method, body, None, true).await
    }

    /// 不等待业务锁（ACE AntiData）。同样等到回包或断线。
    pub async fn request_unlocked(
        &self,
        service: &str,
        method: &str,
        body: &[u8],
    ) -> Result<Vec<u8>> {
        self.send_rpc(service, method, body, None, true).await
    }

    /// 仅 Login / Heartbeat：带超时。不占业务锁，避免大包把心跳堵住。
    pub async fn request_with_timeout(
        &self,
        service: &str,
        method: &str,
        body: &[u8],
        timeout_ms: u64,
    ) -> Result<Vec<u8>> {
        self.send_rpc(service, method, body, Some(timeout_ms), true).await
    }

    /// 对齐原 `sendMsgNoReply`：必须已经 Online，只发送不等待回包。
    pub async fn send_no_reply(&self, service: &str, method: &str, body: &[u8]) -> Result<()> {
        {
            let phase = *self.inner.phase.read();
            rpc_phase_ok(phase, true)?;
        }
        let seq = self.inner.requests.next_seq();
        let token = crate::utils::random::create_gateway_token();
        let frame_bytes = self.encode_request(service, method, body, seq, &token)?;
        let ws_tx = self
            .inner
            .ws_sender
            .lock()
            .as_ref()
            .ok_or_else(|| NetworkError::Phase("ws not connected".into()))?
            .clone();
        ws_tx.send(frame_bytes).await.map_err(|_| NetworkError::WebSocket("send failed".into()))?;
        Ok(())
    }

    async fn acquire_rpc_slot(&self) -> Result<OwnedSemaphorePermit> {
        if let Ok(permit) = Arc::clone(&self.inner.rpc_slots).try_acquire_owned() {
            return Ok(permit);
        }
        let queued = self.inner.rpc_queued.fetch_add(1, Ordering::SeqCst);
        if queued >= crate::constants::MAX_QUEUED_REQUESTS {
            self.inner.rpc_queued.fetch_sub(1, Ordering::SeqCst);
            return Err(NetworkError::QueueFull {
                pending: self.inner.requests.pending_count(),
                queued,
            });
        }
        let permit = Arc::clone(&self.inner.rpc_slots)
            .acquire_owned()
            .await
            .map_err(|_| NetworkError::Phase("rpc limiter closed".into()))?;
        self.inner.rpc_queued.fetch_sub(1, Ordering::SeqCst);
        Ok(permit)
    }

    /// `sendMsg` / `sendMsgAsync` 共用发送路径。
    /// `require_online=true` 对齐 `sendMsgAsync`；`false` 对齐登录用的 `sendMsg`。
    async fn send_rpc(
        &self,
        service: &str,
        method: &str,
        body: &[u8],
        timeout_ms: Option<u64>,
        require_online: bool,
    ) -> Result<Vec<u8>> {
        {
            let phase = *self.inner.phase.read();
            rpc_phase_ok(phase, require_online)?;
        }
        let is_heartbeat = method.eq_ignore_ascii_case("Heartbeat");
        let _slot = if require_online && !is_heartbeat {
            Some(self.acquire_rpc_slot().await?)
        } else {
            None
        };

        let (seq, rx) = self.inner.requests.call(service, method);
        let token = crate::utils::random::create_gateway_token();
        let frame_bytes = self.encode_request(service, method, body, seq, &token)?;

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

        let waited = if let Some(ms) = timeout_ms {
            match tokio::time::timeout(std::time::Duration::from_millis(ms), rx).await {
                Ok(inner) => inner,
                Err(_) => {
                    self.inner.requests.cancel(seq);
                    return Err(NetworkError::Timeout {
                        client_seq: seq,
                        service_name: service.to_string(),
                        method_name: method.to_string(),
                        pending: self.inner.requests.pending_count(),
                    });
                }
            }
        } else {
            rx.await
        };
        match waited {
            Ok(Ok(resp)) => Ok(resp.body),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(NetworkError::Phase("response channel cancelled".into())),
        }
    }

    /// 当前 pending RPC 数（心跳告警对齐 bot `pendingCallbacks.size`）
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.inner.requests.pending_count()
    }

    /// 是否已有指定方法名的 RPC 在路上（心跳避免叠发）
    #[must_use]
    pub fn has_pending_method(&self, method: &str) -> bool {
        self.inner.requests.has_pending_method(method)
    }

    /// 最近一次入站帧时间（ms）。0 表示本会话尚未收到帧。
    #[must_use]
    pub fn last_rx_ms(&self) -> i64 {
        self.inner.last_rx_ms.load(Ordering::Acquire)
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
            extra: Default::default(),
        };
        let body = prost::Message::encode_to_vec(&req);

        // 3. 对齐 sendLogin：用 sendMsg（Login 阶段可发），不是 sendMsgAsync
        let reply_bytes = self
            .send_rpc(
                "gamepb.userpb.UserService",
                "Login",
                &body,
                Some(crate::constants::LOGIN_TIMEOUT_MS),
                false,
            )
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

    /// 登录后拉用户设置（对齐 `fetchUserSettings`，失败忽略）
    pub async fn fetch_user_settings(&self) -> Result<()> {
        let req = crate::proto::generated::gamepb::userpb::GetUserSettingsRequest {};
        let body = prost::Message::encode_to_vec(&req);
        let reply_bytes =
            self.request("gamepb.userpb.UserService", "GetUserSettings", &body).await?;
        let reply = crate::proto::generated::gamepb::userpb::GetUserSettingsReply::decode(
            reply_bytes.as_slice(),
        )
        .map_err(|e| NetworkError::Frame(format!("decode GetUserSettingsReply: {e}")))?;
        if reply.settings.is_some() {
            tracing::info!("用户设置已同步");
        }
        Ok(())
    }

    /// 发 Heartbeat 请求（同步服务器时间 + 维持连接）
    pub async fn heartbeat(&self, gid: i64, client_version: &str) -> Result<HeartbeatReply> {
        let req = HeartbeatRequest { gid, client_version: client_version.to_string(), field_3: 0 };
        let body = prost::Message::encode_to_vec(&req);
        // 对齐 network.ts：Heartbeat 走 sendMsgAsync 默认 20s，不能用 5s（忙时易误超时→掉线）
        let reply_bytes = self
            .request_with_timeout(
                "gamepb.userpb.UserService",
                "Heartbeat",
                &body,
                crate::constants::HEARTBEAT_RPC_TIMEOUT_MS,
            )
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

fn end_session(inner: &Inner, reason: Option<&str>) {
    if let Some(reason) = reason {
        let mut guard = inner.disconnect_reason.lock();
        if guard.is_none() {
            *guard = Some(reason.to_string());
        }
    }
    *inner.phase.write() = ConnectionPhase::Disconnected;
    *inner.ws_sender.lock() = None;
    let n = inner.requests.reject_all();
    if n > 0 {
        tracing::warn!(count = n, "rejected pending requests on disconnect");
    }
    let _ = inner.session_end.send(true);
}

/// 接收 dispatch loop
async fn dispatch_loop(
    mut rx: mpsc::Receiver<crate::network::client::ReceivedFrame>,
    inner: Arc<Inner>,
) {
    while let Some(frame) = rx.recv().await {
        inner.last_rx_ms.store(crate::utils::time::now_ms(), Ordering::Release);
        // 1. 解析外层 GateMessage
        let parsed = match FrameParser::parse(&frame.bytes) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, bytes = frame.bytes.len(), "frame decode failed");
                continue;
            }
        };

        // 2. 更新 server_seq
        let server_seq = parsed.server_seq();
        if server_seq > inner.server_seq.load(Ordering::SeqCst) {
            inner.server_seq.store(server_seq, Ordering::SeqCst);
        }

        // 3. 分发。部分大包（如 FriendService.GetAll）可能不带标准 Response type，
        // 只要 client_seq 对得上 pending 就按回包完成，避免 20s 空等超时。
        let client_seq = parsed.client_seq();
        let pending_method = inner.requests.peek(client_seq);
        let is_pending_reply = client_seq != 0
            && pending_method.as_ref().is_some_and(|(_, method)| {
                parsed.method_name().is_empty() || parsed.method_name() == method
            });
        match parsed.message_type() {
            Some(MessageType::Response) => {
                handle_response(&inner, &parsed);
            }
            Some(MessageType::Notify) if !is_pending_reply => {
                handle_notify(&inner, &parsed);
            }
            _ if is_pending_reply => {
                handle_response(&inner, &parsed);
            }
            Some(MessageType::Notify) => {
                handle_notify(&inner, &parsed);
            }
            _ => {
                tracing::debug!(
                    service = parsed.service_name(),
                    method = parsed.method_name(),
                    client_seq,
                    msg_type = parsed.message_type().map(|t| t as i32),
                    bytes = frame.bytes.len(),
                    "received non-response/notify message"
                );
            }
        }
    }
    end_session(&inner, Some("ws_close"));
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
    } else if !inner.requests.complete(client_seq, parsed.body().to_vec(), parsed.server_seq()) {
        tracing::debug!(
            client_seq,
            service = parsed.service_name(),
            method = parsed.method_name(),
            body_len = parsed.body().len(),
            "response for unknown seq"
        );
    }
}

fn handle_notify(inner: &Arc<Inner>, parsed: &FrameParser) {
    let body = parsed.body();
    if body.is_empty() {
        return;
    }
    let event_msg = match crate::proto::generated::gatepb::EventMessage::decode(body) {
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
    fn login_send_allowed_in_login_phase() {
        assert!(rpc_phase_ok(ConnectionPhase::Login, false).is_ok());
        assert!(rpc_phase_ok(ConnectionPhase::Online, false).is_ok());
        assert!(rpc_phase_ok(ConnectionPhase::Login, true).is_err());
        assert!(rpc_phase_ok(ConnectionPhase::Online, true).is_ok());
        assert!(rpc_phase_ok(ConnectionPhase::Connecting, false).is_err());
        assert!(rpc_phase_ok(ConnectionPhase::Disconnected, false).is_err());
    }

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
