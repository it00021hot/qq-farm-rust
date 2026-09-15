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
//!
//! ## 与 bot 的有意差异
//!
//! - 请求班次模型移植自 bot `request-priority.ts` / `low-priority-gate.ts`
//!   （对照 go `protocol/priority.go`），纯逻辑见 [`crate::network::priority`]。
//!   心跳 / ACE 不再绕开业务槽，而是走 critical 的两条独立保留通道；
//! - 排队超时：bot 的 20s 是「从调用起算、覆盖排队+回包」的单窗口；rust 拆成
//!   两段（排队 20s + 回包 20s），background 排队 8s 即让路（`GatewayBusy`）；
//! - `heartbeat_misses` 用「入站静默 > 心跳阈值」近似（bot 由心跳循环显式计数），
//!   见 [`Gateway::gateway_load`]。

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::{mpsc, oneshot, watch};

use crate::network::client::{ConnectOptions, WsClient};
use crate::network::encryptor::Encryptor;
use crate::network::error::{NetworkError, Result};
use crate::network::frame::{FrameBuilder, FrameParser};
use crate::network::notify::NotifyEvent;
use crate::network::priority::{CriticalLane, InFlightGuard, RequestClass, RpcScheduler};
use crate::network::request::RequestManager;
use crate::proto::generated::gamepb::userpb::{HeartbeatReply, LoginReply};
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

tokio::task_local! {
    /// 环境请求班次（对齐 bot request-context.ts 的 AsyncLocalStorage）：
    /// 调度器在任务入口按命名空间注入 farm / friend，补数据链路注入 background；
    /// 未注入（面板 / IPC / 登录链路）默认按 foreground 处理——那些路径上
    /// 确实有人在等结果。
    static AMBIENT_RPC_CLASS: RequestClass;
}

/// 当前调用链的环境班次；未注入 = `None`（默认按 foreground 处理）。
fn ambient_rpc_class() -> Option<RequestClass> {
    AMBIENT_RPC_CLASS.try_with(|c| *c).ok()
}

/// 把 future 标记为指定 RPC 班次（对齐 bot `runWithRequestClass`）：定时任务入口
/// 打一次标记，任务内所有请求自动继承该班次，不必把班次参数一路透传到每个 API。
pub fn request_class_scope<F: Future>(
    class: RequestClass,
    fut: F,
) -> impl Future<Output = F::Output> {
    AMBIENT_RPC_CLASS.scope(class, fut)
}

/// 补数据任务（宠物同步等）标记为 background 班次：只在网关完全空闲时才发，
/// 前台请求排队时让路（对齐 bot `runWithRequestClass('background')`）。
pub fn background_scope<F: Future>(fut: F) -> impl Future<Output = F::Output> {
    request_class_scope(RequestClass::Background, fut)
}

/// 临时 Notify 订阅（Drop 时自动退订）。
///
/// 供"操作窗口捕获"使用：订阅 → 执行 RPC → 收集紧随其后的 ItemNotify → Drop 退订。
pub struct NotifySubscription {
    id: u64,
    rx: mpsc::Receiver<NotifyEvent>,
    inner: Arc<Inner>,
}

impl NotifySubscription {
    /// 等待下一条事件（None = 所有发送端都已消失，连接已关闭）
    pub async fn recv(&mut self) -> Option<NotifyEvent> {
        self.rx.recv().await
    }
}

impl Drop for NotifySubscription {
    fn drop(&mut self) {
        self.inner.notify_subscribers.write().retain(|(id, _)| *id != self.id);
    }
}

struct Inner {
    config: GatewayConfig,
    phase: RwLock<ConnectionPhase>,
    server_seq: AtomicI64,
    requests: RequestManager,
    /// 加密器（外部注入）。`RwLock<Arc<dyn Encryptor>>` 支持 TSDK 重建时
    /// 原子替换：业务 RPC 的 `encryptor.read().clone()` 拿到当前实例的 Arc，
    /// 后续调用都用新 TSDK；旧 TSDK 的 wasm 内存随旧 Arc 引用计数归零析构。
    /// 用 parking_lot::RwLock 是因为 dyn Encryptor 不是 Sized，arc-swap 需要 Sized。
    /// 读路径在 fast path 用 `read()`（无等待），写路径（TSDK 重建时）极短。
    encryptor: parking_lot::RwLock<Arc<dyn Encryptor>>,
    /// 收到 Notify 事件订阅者（id 用于临时订阅退订）
    notify_subscribers: RwLock<Vec<(u64, mpsc::Sender<NotifyEvent>)>>,
    next_notify_sub_id: AtomicU64,
    /// WS 发送端（connect 时设置）
    ws_sender: parking_lot::Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    /// 当前连接的 client handle（connect 时设置；force_disconnect 时硬关闭）
    ws_client: parking_lot::Mutex<Option<crate::network::client::WsClient>>,
    /// 当前会话是否已结束（dispatch 退出 / 主动断开）
    session_end: watch::Sender<bool>,
    /// 会话结束原因（心跳超时 / kickout / ws_close 等），供 worker 日志对齐 TS source
    disconnect_reason: parking_lot::Mutex<Option<String>>,
    /// 最近一次收到任意 WS 帧的时间（ms）。大包 GetAll 下载期间心跳 RPC 可能超时，但连接仍活。
    last_rx_ms: AtomicI64,
    /// TSDK 重建中标志（worker rebuild 期间置 true，WorkerLoop 据此放宽 silence 阈值）
    rebuilding: AtomicBool,
    /// 五班次请求调度器（对齐 bot request-priority.ts 的队列模型，替换旧的
    /// 「共享 4 槽 + 前台保留 1 槽」信号量）：
    /// critical(heartbeat/ace 各一保留槽) > foreground > farm > friend > background。
    /// 业务流量总在途 ≤3、其中非前台 ≤1；background 只在连接彻底空闲时发。
    rpc_scheduler: Arc<RpcScheduler>,
    /// 出站 token 提供器：登录后暂存一次性 TSDK 初始化凭据，由下一条消息携带
    /// （对齐 bot `GatewayTokenProvider.stageInitToken/next/clear`）。
    token_provider: crate::utils::random::GatewayTokenProvider,
    /// 发送顺序锁：seq 分配 → 加密 → 入发送通道必须原子完成。bot 是单线程
    /// drain 队列、帧严格按 client_seq 递增上wire；rust 并发下三步可交错，
    /// 乱序帧（或加密序 ≠ seq 序）会被服务端丢弃，反复即触发静默断供
    /// （2026-09-11 慢性掉线排查：登录爆发期并发最高，最容易撞出乱序）。
    send_order: tokio::sync::Mutex<()>,
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
                encryptor: parking_lot::RwLock::new(encryptor),
                notify_subscribers: RwLock::new(Vec::new()),
                next_notify_sub_id: AtomicU64::new(1),
                ws_sender: parking_lot::Mutex::new(None),
                ws_client: parking_lot::Mutex::new(None),
                session_end,
                disconnect_reason: parking_lot::Mutex::new(None),
                last_rx_ms: AtomicI64::new(0),
                rebuilding: AtomicBool::new(false),
                rpc_scheduler: RpcScheduler::new(),
                token_provider: crate::utils::random::GatewayTokenProvider::new(),
                send_order: tokio::sync::Mutex::new(()),
            }),
        }
    }

    /// 原子替换 encryptor（TSDK 重建时调用）。新 encryptor 对所有后续
    /// `request_with_timeout`/`request` 调用立即可见；替换瞬间的 in-flight
    /// 调用仍持有旧 Arc 引用，析构时机由引用计数决定。
    /// 同时短暂置位 `rebuilding`，让 WorkerLoop 把 silence 阈值放宽到 90s。
    pub fn replace_encryptor(&self, new_encryptor: Arc<dyn Encryptor>) {
        *self.inner.encryptor.write() = new_encryptor;
    }

    /// 进入 TSDK 重建期：rebuilding=true，让 WorkerLoop 放宽心跳静默阈值
    pub fn begin_rebuild(&self) {
        self.inner.rebuilding.store(true, std::sync::atomic::Ordering::Release);
    }

    /// 退出 TSDK 重建期
    pub fn end_rebuild(&self) {
        self.inner.rebuilding.store(false, std::sync::atomic::Ordering::Release);
    }

    /// 是否正在重建 TSDK
    #[must_use]
    pub fn is_rebuilding(&self) -> bool {
        self.inner.rebuilding.load(std::sync::atomic::Ordering::Acquire)
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
        *self.inner.ws_client.lock() = Some(client.clone());

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
        // 对齐 bot startHeartbeat 的 lastInboundAt=now：连接建立即算活跃，
        // 避免登录后 25s 内入站稀疏导致 inbound_silence 虚高误判。
        self.inner.last_rx_ms.store(crate::utils::time::now_ms(), Ordering::Release);

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

    /// 带原因断开（对齐 TS `finalizeConnection({ source })` + `socket.terminate()`）
    pub fn force_disconnect_with_reason(&self, reason: &str) {
        {
            let mut guard = self.inner.disconnect_reason.lock();
            if guard.is_none() {
                *guard = Some(reason.to_string());
            }
        }
        // 硬关闭：丢弃发送通道并 abort 底层读写 task，TCP 立即断开。
        // 只发 watch 信号的话 socket 可能继续挂 30s+，服务端旧 session 未释放，
        // 重连登录会被判"已在其他终端登录"踢下线。
        *self.inner.ws_sender.lock() = None;
        if let Some(client) = self.inner.ws_client.lock().take() {
            client.terminate();
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
            // 通过 RwLock::read() 拿到当前 encryptor 的 Arc 克隆：
            // - 读路径不阻塞其他读，多线程并发 RPC 安全
            // - 替换瞬间的 in-flight 调用仍持有旧 Arc 引用，旧 TSDK 随旧 Arc 引用计数归零自动析构
            self.inner
                .encryptor
                .read()
                .clone()
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

    /// 高阶 API：发请求 + 等响应。默认 20s 超时（对齐 bot `sendMsgAsync`）：
    /// 无超时会让服务端漏回的请求永久占用并发槽，5 槽漏满后业务全堵死。
    /// 班次由方法名 + 环境标记决定（`resolve_request_class`）。
    pub async fn request(&self, service: &str, method: &str, body: &[u8]) -> Result<Vec<u8>> {
        self.send_rpc(service, method, body, Some(crate::constants::DEFAULT_RPC_TIMEOUT_MS), true)
            .await
    }

    /// 与 [`Self::request`] 同路径（历史上 ACE AntiData 用它绕开业务槽；
    /// 五班次模型下 AntiData 按方法名走 critical 的 ace 保留通道，无需特判）。
    pub async fn request_unlocked(
        &self,
        service: &str,
        method: &str,
        body: &[u8],
    ) -> Result<Vec<u8>> {
        self.send_rpc(service, method, body, Some(crate::constants::DEFAULT_RPC_TIMEOUT_MS), true)
            .await
    }

    /// 带自定义回包超时的请求（Heartbeat / AntiData 用）。同样过五班次调度：
    /// Heartbeat → critical/heartbeat 保留通道，AntiData → critical/ace 保留通道。
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
        // 对齐 go SendNoReply / bot sendMsgNoReply：no-reply 帧也过五班次调度器
        // （发送完成即还槽，不等待回包）
        let (class, lane) =
            crate::network::priority::resolve_request_class(method, ambient_rpc_class());
        let _slot = self.acquire_dispatch(class, lane, service, method).await?;
        // 与 send_rpc 同一把发送顺序锁，保证 no-reply 帧也不破坏 seq 递增
        let _order = self.inner.send_order.lock().await;
        let seq = self.inner.requests.next_seq();
        let (token, staged) = self.inner.token_provider.next_marked();
        if staged {
            tracing::info!(service, method, "TSDK 初始化凭据已随本条请求发送");
        }
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

    /// 入队并等待班次调度授权（对齐 bot 队列模型的「入队 → drain 授权」）。
    ///
    /// 排队超时口径（对齐 go priority.go / 现有 rust 20s 常量）：
    /// - background：[`LOW_PRIORITY_QUEUE_WAIT_MS`]（8s）内拿不到空闲就让路
    ///   （go `GatewayBusyError` / bot「网关繁忙，后台请求已让路」）——排队本身
    ///   会拖长队列把 pending 拉满，把剩下的活留给下一轮更健康；
    /// - 其余班次：[`RPC_QUEUE_TIMEOUT_MS`]（20s，沿用 rust 既有口径；bot 的
    ///   20s 是覆盖排队+回包的单窗口，rust 拆成两段，见模块注释的有意差异）。
    ///
    /// 返回的 [`InFlightGuard`] 持有该班次的一个在途槽位，Drop 时归还并触发
    /// 重新调度；等待中被取消（future drop）也不会漏账。
    async fn acquire_dispatch(
        &self,
        class: RequestClass,
        lane: Option<CriticalLane>,
        service: &str,
        method: &str,
    ) -> Result<InFlightGuard> {
        let scheduler = Arc::clone(&self.inner.rpc_scheduler);
        let ticket = match scheduler.try_enqueue(class, lane) {
            Ok(ticket) => ticket,
            Err(info) => {
                // 该班次排队配额已满：入队前直接拒绝（对齐 bot isClassQueueFull）
                return Err(NetworkError::QueueFull {
                    pending: info.pending,
                    queued: info.queued_total,
                });
            }
        };
        let wait_ms = if class == RequestClass::Background {
            crate::network::priority::LOW_PRIORITY_QUEUE_WAIT_MS
        } else {
            crate::constants::RPC_QUEUE_TIMEOUT_MS
        };
        match tokio::time::timeout(std::time::Duration::from_millis(wait_ms), ticket.granted())
            .await
        {
            Ok(Some(guard)) => Ok(guard),
            // 队列被清空（连接断开 / 会话结束）：对齐 go「连接未打开: %s」
            Ok(None) => Err(NetworkError::Phase(format!("连接未打开: {method}"))),
            Err(_) => {
                // ticket 已随本 future 丢弃，队列条目会在下次 drain 时回收
                if class == RequestClass::Background {
                    Err(NetworkError::GatewayBusy {
                        method_name: method.to_string(),
                        waited_ms: wait_ms,
                        pending: self.inner.requests.pending_count(),
                        queued: scheduler.queued_count(),
                    })
                } else {
                    Err(NetworkError::Timeout {
                        client_seq: 0,
                        service_name: service.to_string(),
                        method_name: method.to_string(),
                        pending: self.inner.requests.pending_count(),
                    })
                }
            }
        }
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
        // 五班次调度（对齐 bot request-priority.ts）：心跳 / ACE 按方法名走
        // critical 的两条保留通道；面板 / 登录链路默认 foreground；定时任务由
        // scheduler 注入 farm / friend；补数据任务注入 background。业务高峰时
        // 心跳和 AntiData 依然有保留槽位，不会被业务流量挤到超时掉线。
        let (class, lane) =
            crate::network::priority::resolve_request_class(method, ambient_rpc_class());
        let _slot = self.acquire_dispatch(class, lane, service, method).await?;

        // 发送顺序锁：seq → 加密 → 入队原子化，保证上 wire 的帧严格按
        // client_seq 递增（对齐 bot 单线程 drain 队列）。等回包在锁外。
        let _order = self.inner.send_order.lock().await;
        let (seq, rx) = self.inner.requests.call(service, method);
        let (token, staged) = self.inner.token_provider.next_marked();
        if staged {
            tracing::info!(service, method, "TSDK 初始化凭据已随本条请求发送");
        }
        let frame_bytes = match self.encode_request(service, method, body, seq, &token) {
            Ok(f) => f,
            Err(e) => {
                // 发送失败路径清掉 pending 条目（对齐 bot sendMsg encode 失败即删回调）
                let _ = self.inner.requests.cancel(seq);
                return Err(e);
            }
        };

        let ws_tx = match self.inner.ws_sender.lock().as_ref().map(|tx| tx.clone()) {
            Some(tx) => tx,
            None => {
                let _ = self.inner.requests.cancel(seq);
                return Err(NetworkError::Phase("ws not connected".into()));
            }
        };
        if ws_tx.send(frame_bytes).await.is_err() {
            let _ = self.inner.requests.cancel(seq);
            return Err(NetworkError::WebSocket("ws sender closed".into()));
        }
        drop(_order);

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

    /// 当前网关负载快照（对齐 bot `getGatewayLoad` / go `GatewayLoad`）。
    ///
    /// `heartbeat_misses` 用「入站静默超过心跳阈值」近似（bot 由心跳循环显式计数，
    /// rust 的 miss 计数在 worker_loop 内部）：服务端一旦不回包，心跳必然漏拍，
    /// 两种口径对「连接可疑」的判定等价。
    #[must_use]
    pub fn gateway_load(&self) -> crate::network::priority::GatewayLoadSnapshot {
        let mut load = self.inner.rpc_scheduler.load();
        let now = crate::utils::time::now_ms();
        let last_rx = self.last_rx_ms();
        if last_rx > 0
            && now.saturating_sub(last_rx) > crate::constants::HEARTBEAT_SILENCE_MS as i64
        {
            load.heartbeat_misses = 1;
        }
        load
    }

    /// background 请求现在是否可以发（低优先空闲门，对齐 bot `isGatewayIdleForLowPriority`）
    #[must_use]
    pub fn is_gateway_idle_for_background(&self) -> bool {
        crate::network::priority::is_gateway_idle_for_low_priority(&self.gateway_load())
    }

    /// farm / friend 定时任务健康度闸门（对齐 bot `isGatewayHealthyForBusiness`）：
    /// 只要求连接还在回包，正常排队竞争不算不健康。
    #[must_use]
    pub fn is_gateway_healthy_for_business(&self) -> bool {
        crate::network::priority::is_gateway_healthy_for_business(&self.gateway_load())
    }

    /// 是否已有指定方法名的 RPC 在路上（心跳避免叠发）
    #[must_use]
    pub fn has_pending_method(&self, method: &str) -> bool {
        self.inner.requests.has_pending_method(method)
    }

    /// 所有 pending RPC 的 method 名（掉线诊断：看服务端卡住了哪些请求）
    #[must_use]
    pub fn pending_methods(&self) -> Vec<String> {
        self.inner.requests.pending_methods()
    }

    /// 最近一次入站帧时间（ms）。0 表示本会话尚未收到帧。
    #[must_use]
    pub fn last_rx_ms(&self) -> i64 {
        self.inner.last_rx_ms.load(Ordering::Acquire)
    }

    /// 订阅 Notify 事件（永久订阅；worker 主循环用）
    pub fn subscribe_notify(&self) -> mpsc::Receiver<NotifyEvent> {
        let (tx, rx) = mpsc::channel(32);
        let id = self.inner.next_notify_sub_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.notify_subscribers.write().push((id, tx));
        rx
    }

    /// 订阅 Notify 事件（临时订阅；Drop 时自动退订，供操作窗口捕获用）
    #[must_use]
    pub fn subscribe_notify_scoped(&self) -> NotifySubscription {
        let (tx, rx) = mpsc::channel(32);
        let id = self.inner.next_notify_sub_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.notify_subscribers.write().push((id, tx));
        NotifySubscription { id, rx, inner: Arc::clone(&self.inner) }
    }

    /// 当前 Notify 订阅数（测试断言退订用）
    #[cfg(test)]
    pub(crate) fn notify_subscriber_count(&self) -> usize {
        self.inner.notify_subscribers.read().len()
    }

    /// 标记登录完成（阶段 1A 外部调用；阶段 1B 由业务模块在收到登录响应后调用）
    pub fn mark_online(&self) {
        *self.inner.phase.write() = ConnectionPhase::Online;
    }

    /// 完整登录流程：发 LoginRequest → 等 LoginReply → bindUser → mark_online
    ///
    /// 1:1 对应原 `network.ts:sendLogin()`。请求体由
    /// [`crate::network::login_body::build_login_body`] 逐字节对齐官方抓包。
    ///
    /// # Arguments
    /// - `client_version`: 生效的客户端版本（如 `1.14.0.4_20260911`）
    /// - `sys_software`: 系统标识（如 `Windows`）
    /// - `tsdk`: TSDK runtime（用于 bindUser）
    pub async fn login(
        &self,
        client_version: &str,
        sys_software: &str,
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

        // 2. 构造 LoginRequest（逐字节对齐官方 73 字节抓包）
        let body = crate::network::login_body::build_login_body(client_version, sys_software);

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
            match tsdk.bind_user(&basic.open_id) {
                Ok(()) => {
                    // 对齐 bot network.ts:770-774：bindUser 后把加密初始化凭据
                    // stage 进 token provider，由下一条出站消息携带（恰好一次）。
                    // 缺这一步服务端 ACE 会话不完整，会不定时静默丢弃连接。
                    match tsdk.get_encrypted_init_info() {
                        Ok(info) => match self.inner.token_provider.stage_init_token(&info) {
                            Ok(0) => {}
                            Ok(len) => {
                                tracing::info!(len, "TSDK 初始化凭据已就绪，将随下一条请求发送")
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "TSDK 初始化凭据暂存失败");
                            }
                        },
                        Err(e) => {
                            tracing::warn!(error = %e, "TSDK get_encrypted_init_info 失败");
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, "TSDK bindUser 失败"),
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
        // 逐字节对齐官方 27 字节抓包（field_3 显式写 0）
        let body = crate::network::login_body::build_heartbeat_body(gid, client_version);
        // 对齐 network.ts：Heartbeat 走 sendMsgAsync 默认 20s，不能用 5s（忙时易误超时→掉线）
        let reply_bytes = self
            .request_with_timeout(
                "gamepb.userpb.UserService",
                "Heartbeat",
                &body,
                crate::constants::HEARTBEAT_RPC_TIMEOUT_MS,
            )
            .await?;
        // 对齐 bot network.ts:822-827：心跳回包到达即算成功（先记账再解码），
        // 解码失败只 warn 不计 miss——连接活着就不该因本地解码问题判死。
        let reply = match HeartbeatReply::decode(reply_bytes.as_slice()) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "decode HeartbeatReply 失败（按成功处理）");
                HeartbeatReply::default()
            }
        };
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
    // 丢弃未消费的一次性初始化凭据（对齐 bot clearNetworkRuntime → gatewayTokens.clear()）
    inner.token_provider.clear();
    *inner.phase.write() = ConnectionPhase::Disconnected;
    *inner.ws_sender.lock() = None;
    // 会话结束后不再保留 client handle（连接已由对端/force_disconnect 关闭）
    inner.ws_client.lock().take();
    let n = inner.requests.reject_all();
    if n > 0 {
        tracing::warn!(count = n, "rejected pending requests on disconnect");
    }
    // 排队中的请求一并失败（对齐 go rejectAll）：掉 drop 授权发送端，
    // 等待方拿到「连接未打开」而不是熬到排队超时
    let queued = inner.rpc_scheduler.reject_all_queued();
    if queued > 0 {
        tracing::warn!(count = queued, "rejected queued requests on disconnect");
    }
    let _ = inner.session_end.send(true);
}

/// 接收 dispatch loop
async fn dispatch_loop(
    mut rx: mpsc::Receiver<crate::network::client::ReceivedFrame>,
    inner: Arc<Inner>,
) {
    while let Some(frame) = rx.recv().await {
        // 1. 解析外层 GateMessage（对齐 bot：decode 成功才计入入站活跃时间）
        let parsed = match FrameParser::parse(&frame.bytes) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, bytes = frame.bytes.len(), "frame decode failed");
                continue;
            }
        };
        inner.last_rx_ms.store(crate::utils::time::now_ms(), Ordering::Release);

        // 2. 更新 server_seq
        let server_seq = parsed.server_seq();
        if server_seq > inner.server_seq.load(Ordering::SeqCst) {
            inner.server_seq.store(server_seq, Ordering::SeqCst);
        }

        // 3. 分发（对齐 bot network.ts:396-424：严格按 type 分发）。
        // 显式 Notify 一律走 handle_notify，绝不当回包消费（曾把推送吞成 pending 回复）。
        // 仅当类型缺失/未知且 client_seq 命中 pending 时按回包容错完成
        // （部分大包如 FriendService.GetAll 不带标准 Response type）。
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
            Some(MessageType::Notify) => {
                handle_notify(&inner, &parsed);
            }
            _ if is_pending_reply => {
                handle_response(&inner, &parsed);
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
    for (_, tx) in subs {
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

    fn test_gateway() -> Gateway {
        Gateway::new(
            GatewayConfig {
                server_url: "wss://gate.example.com/ws".into(),
                platform: "qq".into(),
                os: "windows".into(),
                client_version: "1.0.0".into(),
                auth_code: "test".into(),
                headers: HashMap::new(),
            },
            std::sync::Arc::new(crate::network::encryptor::NoopEncryptor),
        )
    }

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

    /// 前台保留槽位回归：后台自动化（farm/friend 班次）占满非前台额度后，
    /// 前台请求依然能立即拿到授权（对齐 bot「前台至少保留两个业务槽位」）。
    /// start_paused：background 8s 让路时限用虚拟时钟自动推进，不必真等 8 秒。
    #[tokio::test(start_paused = true)]
    async fn foreground_survives_non_foreground_saturation() {
        let gateway = test_gateway();
        // farm 占住唯一的非前台在途槽（MAX_NON_FOREGROUND_BUSINESS_IN_FLIGHT = 1）
        let farm = gateway
            .acquire_dispatch(RequestClass::Farm, None, "svc", "FarmOp")
            .await
            .expect("farm slot");
        // friend 也想飞：非前台额度已满 → 只能排队
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(50),
                gateway.acquire_dispatch(RequestClass::Friend, None, "svc", "FriendOp")
            )
            .await
            .is_err(),
            "friend must queue while the single non-foreground slot is taken"
        );
        // 前台不受影响：两个保留槽位都能立即拿到（业务总预算 3 = farm + 前台×2）
        let fg1 = gateway
            .acquire_dispatch(RequestClass::Foreground, None, "svc", "PanelOp1")
            .await
            .expect("foreground slot 1");
        let fg2 = gateway
            .acquire_dispatch(RequestClass::Foreground, None, "svc", "PanelOp2")
            .await
            .expect("foreground slot 2");
        // 业务总预算占满后，background 让路：8s 时限到点返回 GatewayBusy 而非无限排队
        let busy = gateway
            .acquire_dispatch(RequestClass::Background, None, "svc", "PetSync")
            .await
            .unwrap_err();
        assert!(matches!(busy, NetworkError::GatewayBusy { .. }), "actual: {busy:?}");
        drop((farm, fg1, fg2));
    }

    /// 心跳保留通道回归：业务流量占满总预算时，Heartbeat 仍立即拿到
    /// critical 独立保留槽位（对齐 bot「心跳业务排满也挤不掉」）。
    #[tokio::test]
    async fn heartbeat_lane_survives_business_saturation() {
        let gateway = test_gateway();
        let _slots = futures::future::join_all([
            gateway.acquire_dispatch(RequestClass::Foreground, None, "svc", "A"),
            gateway.acquire_dispatch(RequestClass::Foreground, None, "svc", "B"),
            gateway.acquire_dispatch(RequestClass::Foreground, None, "svc", "C"),
        ])
        .await;
        assert_eq!(gateway.inner.rpc_scheduler.in_flight_count(), 3);
        // Heartbeat 方法名自动解析为 critical 保留通道
        let (class, lane) =
            crate::network::priority::resolve_request_class("Heartbeat", ambient_rpc_class());
        assert_eq!(class, RequestClass::Critical);
        let hb = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            gateway.acquire_dispatch(class, lane, "svc", "Heartbeat"),
        )
        .await
        .expect("heartbeat granted immediately")
        .expect("heartbeat guard");
        assert_eq!(gateway.gateway_load().critical_pending, 1);
        drop(hb);
    }

    #[tokio::test]
    async fn ambient_class_defaults_to_foreground_and_scope_overrides() {
        // 缺省（面板 / IPC / 登录链路）= foreground
        assert_eq!(ambient_rpc_class(), None);
        assert_eq!(
            crate::network::priority::resolve_request_class("Purchase", ambient_rpc_class()).0,
            RequestClass::Foreground
        );
        // request_class_scope 注入后整条调用链继承班次
        let seen = request_class_scope(RequestClass::Friend, async { ambient_rpc_class() }).await;
        assert_eq!(seen, Some(RequestClass::Friend));
        // background_scope 标记补数据任务
        let seen = background_scope(async { ambient_rpc_class() }).await;
        assert_eq!(seen, Some(RequestClass::Background));
        // scope 结束后恢复默认
        assert_eq!(ambient_rpc_class(), None);
    }

    #[test]
    fn urlencoding_spaces() {
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("a-b_c.d~e"), "a-b_c.d~e");
    }

    /// 关键回归测试：replace_encryptor 必须能原子替换 RwLock<Arc<dyn Encryptor>>，
    /// 后续 load 拿到新实例，原 Arc 引用计数归零后析构（释放旧 TSDK 内存）。
    #[test]
    fn replace_encryptor_swaps_atomic_and_drops_old() {
        use crate::network::encryptor::Encryptor;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc as StdArc;

        struct TestEncryptor {
            id: usize,
            drops: StdArc<AtomicUsize>,
        }
        impl Drop for TestEncryptor {
            fn drop(&mut self) {
                self.drops.fetch_add(1, Ordering::Relaxed);
            }
        }
        impl Encryptor for TestEncryptor {
            fn encrypt(&self, _p: &[u8]) -> crate::error::Result<Vec<u8>> {
                Ok(vec![self.id as u8])
            }
            fn decrypt(&self, _p: &[u8]) -> crate::error::Result<Vec<u8>> {
                Ok(vec![self.id as u8])
            }
        }

        // 直接验证 parking_lot::RwLock<Arc<dyn Encryptor>> 的语义。
        let drops = StdArc::new(AtomicUsize::new(0));
        let lock =
            parking_lot::RwLock::new(
                StdArc::new(TestEncryptor { id: 1, drops: drops.clone() }) as StdArc<dyn Encryptor>
            );

        // load 拿当前
        assert_eq!(lock.read().encrypt(b"").unwrap(), vec![1u8]);

        // 替换为新实例
        let old = lock.read().clone();
        *lock.write() = StdArc::new(TestEncryptor { id: 2, drops: drops.clone() });
        // 旧实例还活着（我们持有了 old）
        assert_eq!(old.encrypt(b"").unwrap(), vec![1u8]);
        // 新实例 load 拿到
        assert_eq!(lock.read().encrypt(b"").unwrap(), vec![2u8]);
        // drop 旧引用
        drop(old);
        // 旧实例被 drop
        assert_eq!(drops.load(Ordering::Relaxed), 1);

        // 再换一次
        let old2 = lock.read().clone();
        *lock.write() = StdArc::new(TestEncryptor { id: 3, drops: drops.clone() });
        drop(old2);
        assert_eq!(drops.load(Ordering::Relaxed), 2);
    }
}
