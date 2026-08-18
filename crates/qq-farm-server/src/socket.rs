//! 实时推送：Socket.IO（对齐原 `controllers/admin/socket.ts`）+ 兼容 `/ws`。
//!
//! 前端 `web/src/stores/status.ts` 走 `io('/', { path: '/socket.io' })`，事件：
//! - `status:update` `{ accountId, status }`
//! - `log:new`
//! - `account-log:new`
//! - `logs:snapshot` / `account-logs:snapshot`
//! - `subscribed` / `ready`

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        FromRequest, FromRequestParts, Query, State,
    },
    http::{request::Parts, Request, StatusCode},
    response::{IntoResponse, Response},
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use socketioxide::{
    extract::{Data, SocketRef, TryData},
    SocketIo,
};

use crate::context::AdminContext;
use qq_farm_core::runtime::runtime_state::{LogEntry, RuntimeEvent};

#[derive(Debug, Deserialize)]
struct HandshakeAuth {
    token: String,
    #[serde(default, rename = "accountId")]
    account_id: String,
}

#[derive(Debug, Deserialize)]
struct SubscribePayload {
    #[serde(default, rename = "accountId")]
    account_id: String,
}

/// 构造 Socket.IO layer + io 句柄（挂到 axum Router）
pub fn setup_socketio(ctx: Arc<AdminContext>) -> (socketioxide::layer::SocketIoLayer, SocketIo) {
    let (layer, io) = SocketIo::new_layer();
    let ctx_ns = ctx.clone();
    io.ns("/", move |s: SocketRef, TryData(auth): TryData<HandshakeAuth>| {
        let ctx = ctx_ns.clone();
        async move {
            let auth = match auth {
                Ok(a) if !a.token.is_empty() && ctx.sessions.get(&a.token).is_some() => a,
                _ => {
                    let _ = s.disconnect();
                    return;
                }
            };
            apply_subscription(&s, &ctx, &auth.account_id, &auth.token);
            let ready = json!({ "ok": true, "ts": chrono::Utc::now().timestamp_millis() });
            let _ = s.emit("ready", &ready);

            let ctx_sub = ctx.clone();
            let token = auth.token.clone();
            s.on("subscribe", move |s: SocketRef, Data::<SubscribePayload>(payload)| {
                let ctx_sub = ctx_sub.clone();
                let token = token.clone();
                async move {
                    apply_subscription(&s, &ctx_sub, &payload.account_id, &token);
                }
            });
        }
    });
    (layer, io)
}

/// 把 runtime 事件转到 Socket.IO 房间（对齐 emitRealtimeStatus / Log / AccountLog）。
///
/// 订阅 `RuntimeEngine::runtime_state().subscribe()` 的 broadcast 通道。
/// desktop 客户端应改用 `qq_farm_app::AppContext::subscribe_events` 并包装为 [`AppEvent`](qq_farm_app::events::AppEvent)。
pub fn spawn_socket_forwarder(io: SocketIo, ctx: Arc<AdminContext>) {
    let mut rx = ctx.engine.runtime_state().subscribe();
    tokio::spawn(async move {
        loop {
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = n, "socket forwarder lagged");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            match event {
                RuntimeEvent::Status { account_id, status, .. } => {
                    if account_id.is_empty() {
                        continue;
                    }
                    let room = format!("account:{account_id}");
                    let payload = json!({ "accountId": account_id, "status": status });
                    let _ = io.to(room).emit("status:update", &payload).await;
                }
                RuntimeEvent::Log(entry) => {
                    let id = entry.account_id.clone().unwrap_or_default();
                    if id.is_empty() {
                        continue;
                    }
                    let room = format!("account:{id}");
                    let _ = io.to(room).emit("log:new", &entry).await;
                }
                RuntimeEvent::AccountLog(entry) => {
                    if entry.account_id.is_empty() {
                        continue;
                    }
                    let room = format!("account:{}", entry.account_id);
                    let _ = io.to(room).emit("account-log:new", &entry).await;
                }
                RuntimeEvent::AccountStatus {
                    account_id,
                    account_name,
                    status,
                    detail,
                    wx_authorized,
                } => {
                    if account_id.is_empty() {
                        continue;
                    }
                    let room = format!("account:{account_id}");
                    let payload = json!({
                        "accountId": account_id,
                        "accountName": account_name,
                        "status": status,
                        "detail": detail,
                        "wxAuthorized": wx_authorized,
                    });
                    let _ = io.to(room).emit("account_status", &payload).await;
                }
                RuntimeEvent::WorkerLog { entry, account_id, .. } => {
                    if account_id.is_empty() {
                        continue;
                    }
                    let room = format!("account:{account_id}");
                    let _ = io.to(room).emit("log:new", &entry).await;
                }
            }
        }
    });
}

fn apply_subscription(socket: &SocketRef, ctx: &AdminContext, account_ref: &str, token: &str) {
    let incoming = account_ref.trim();
    let sess = ctx.sessions.get(token);
    let is_admin = sess.as_ref().is_some_and(|s| s.role == "admin");
    let username = sess.as_ref().map(|s| s.username.clone()).unwrap_or_default();
    let resolved =
        if incoming.is_empty() || incoming == "all" { String::new() } else { incoming.to_string() };

    if !resolved.is_empty() && !is_admin {
        let owned = qq_farm_core::models::store::accounts::get_accounts()
            .into_iter()
            .any(|a| a.id == resolved && a.username == username);
        if !owned {
            let payload = json!({ "accountId": resolved, "error": "forbidden" });
            let _ = socket.emit("subscribed", &payload);
            return;
        }
    }

    for room in socket.rooms() {
        let name = room.to_string();
        if name.starts_with("account:") {
            socket.leave(name);
        }
    }

    if resolved.is_empty() {
        socket.join("account:all");
        let payload = json!({ "accountId": "all" });
        let _ = socket.emit("subscribed", &payload);
        push_snapshots(socket, ctx, "", is_admin, &username);
        return;
    }

    socket.join(format!("account:{resolved}"));
    let payload = json!({ "accountId": resolved });
    let _ = socket.emit("subscribed", &payload);
    push_snapshots(socket, ctx, &resolved, is_admin, &username);
}

fn push_snapshots(
    socket: &SocketRef,
    ctx: &AdminContext,
    account_id: &str,
    is_admin: bool,
    username: &str,
) {
    if !account_id.is_empty() {
        let status = ctx.engine.panel_status(account_id);
        let payload = json!({ "accountId": account_id, "status": status });
        let _ = socket.emit("status:update", &payload);
    }
    let owned: std::collections::HashSet<String> = if is_admin {
        Default::default()
    } else {
        qq_farm_core::models::store::accounts::get_accounts()
            .into_iter()
            .filter(|a| a.username == username)
            .map(|a| a.id)
            .collect()
    };
    let state = ctx.engine.runtime_state();
    let logs = if account_id.is_empty() {
        let all = state.global_logs.lock().clone();
        if is_admin {
            all
        } else {
            all.into_iter()
                .filter(|l| l.account_id.as_deref().is_some_and(|id| owned.contains(id)))
                .collect()
        }
    } else {
        state
            .global_logs
            .lock()
            .iter()
            .filter(|l| l.account_id.as_deref() == Some(account_id))
            .cloned()
            .collect()
    };
    let logs: Vec<_> = logs.into_iter().rev().take(100).rev().collect();
    let logs_payload = json!({
        "accountId": if account_id.is_empty() { "all" } else { account_id },
        "logs": logs
    });
    let _ = socket.emit("logs:snapshot", &logs_payload);
    let account_logs: Vec<_> = state
        .account_logs
        .lock()
        .iter()
        .rev()
        .filter(|l| {
            if !account_id.is_empty() {
                l.account_id == account_id
            } else if is_admin {
                true
            } else {
                owned.contains(&l.account_id)
            }
        })
        .take(100)
        .cloned()
        .collect();
    let account_logs_payload = json!({ "logs": account_logs });
    let _ = socket.emit("account-logs:snapshot", &account_logs_payload);
}

/// 兼容旧 `/ws`（E2E / 调试）。须带 `?token=` 或 `x-admin-token`，与 Socket.IO 鉴权对齐。
///
/// 鉴权在 `WebSocketUpgrade` 提取之前完成，避免无 Upgrade 头时直接 426。
pub async fn ws_handler(State(ctx): State<Arc<AdminContext>>, req: Request<Body>) -> Response {
    let (mut parts, body) = req.into_parts();
    let token = match extract_ws_token(&mut parts).await {
        Ok(t) => t,
        Err(resp) => return resp,
    };
    if ctx.sessions.get(&token).is_none() {
        return (StatusCode::UNAUTHORIZED, "invalid or expired token").into_response();
    }
    let req = Request::from_parts(parts, body);
    match WebSocketUpgrade::from_request(req, &mut ()).await {
        Ok(ws) => ws.on_upgrade(move |socket| handle_socket(socket, ctx, token)),
        Err(rej) => rej.into_response(),
    }
}

async fn extract_ws_token(parts: &mut Parts) -> Result<String, Response> {
    let q = Query::<WsAuthQuery>::from_request_parts(parts, &mut ())
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad query").into_response())?;
    let token = q.token.clone().filter(|t| !t.is_empty()).or_else(|| {
        parts
            .headers
            .get("x-admin-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
            .filter(|t| !t.is_empty())
    });
    token.ok_or_else(|| (StatusCode::UNAUTHORIZED, "missing token").into_response())
}

#[derive(Debug, Deserialize)]
pub struct WsAuthQuery {
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubscribeMsg {
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    account_id: Option<String>,
}

async fn handle_socket(socket: WebSocket, ctx: Arc<AdminContext>, token: String) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = ctx.engine.runtime_state().subscribe();
    let subscribed_account: Arc<parking_lot::Mutex<Option<String>>> =
        Arc::new(parking_lot::Mutex::new(None));

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);
    let forward_task = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    let tx_clone = tx.clone();
    let subscribed_clone = subscribed_account.clone();
    let event_task = tokio::spawn(async move {
        loop {
            let event = match event_rx.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            let sub = subscribed_clone.lock().clone();
            if let Some(account_id) = sub.as_deref().filter(|s| !s.is_empty()) {
                if event_account_id(&event).as_deref() != Some(account_id) {
                    continue;
                }
            }
            if let Ok(text) = serde_json::to_string(&event) {
                if tx_clone.send(text).await.is_err() {
                    break;
                }
            }
        }
    });

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(_) => break,
        };
        let parsed: Result<SubscribeMsg, _> = serde_json::from_str(&msg);
        let Ok(parsed) = parsed else { continue };
        match parsed.msg_type.as_str() {
            "subscribe" => {
                let incoming = parsed.account_id.unwrap_or_default();
                let normalized = if incoming.is_empty() || incoming == "all" {
                    None
                } else if account_id_allowed(&ctx, &token, &incoming) {
                    Some(incoming)
                } else {
                    let text = serde_json::to_string(&json!({
                        "type": "error",
                        "error": "无权订阅该账号"
                    }))
                    .unwrap_or_default();
                    let _ = tx.send(text).await;
                    continue;
                };
                let ack_account = normalized.clone().unwrap_or_else(|| "all".to_string());
                *subscribed_account.lock() = normalized;
                let text = serde_json::to_string(&json!({
                    "type": "subscribed",
                    "accountId": ack_account
                }))
                .unwrap_or_default();
                let _ = tx.send(text).await;
            }
            "ping" => {
                let text = serde_json::to_string(&json!({"type": "pong"})).unwrap_or_default();
                let _ = tx.send(text).await;
            }
            _ => {}
        }
    }

    forward_task.abort();
    event_task.abort();
}

fn account_id_allowed(ctx: &AdminContext, token: &str, account_id: &str) -> bool {
    let Some(info) = ctx.sessions.get(token) else {
        return false;
    };
    if info.role == "admin" {
        return true;
    }
    qq_farm_core::models::store::accounts::get_accounts()
        .into_iter()
        .any(|a| a.id == account_id && a.username == info.username)
}

fn event_account_id(event: &RuntimeEvent) -> Option<String> {
    match event {
        RuntimeEvent::Log(e) => e.account_id.clone(),
        RuntimeEvent::AccountLog(e) => non_empty(e.account_id.clone()),
        RuntimeEvent::Status { account_id, .. } => non_empty(account_id.clone()),
        RuntimeEvent::AccountStatus { account_id, .. } => non_empty(account_id.clone()),
        RuntimeEvent::WorkerLog { account_id, .. } => non_empty(account_id.clone()),
    }
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 推送 status 变更（外部调用）
pub fn emit_status(ctx: &AdminContext, account_id: &str, status: serde_json::Value) {
    let _ = ctx.engine.runtime_state().events.send(RuntimeEvent::Status {
        account_id: account_id.to_string(),
        account_name: String::new(),
        status,
    });
}

/// 推送 log（外部调用）
#[allow(dead_code)]
pub fn emit_log(ctx: &AdminContext, entry: serde_json::Value) {
    let _ = ctx.engine.runtime_state().events.send(RuntimeEvent::Log(LogEntry {
        time: String::new(),
        tag: String::new(),
        msg: String::new(),
        meta: entry,
        ts: 0,
        search_text: String::new(),
        account_id: None,
        account_name: None,
        is_warn: false,
    }));
}
