//! WebSocket 实时推送。
//!
//! 1:1 对应原 `controllers/admin/socket.ts`（185 行）。
//!
//! ## 推送事件
//!
//! - `status` — worker 状态变化
//! - `log` — 全局日志
//! - `account_log` — 账号日志
//! - `subscribed` — 订阅确认
//!
//! ## 协议
//!
//! 用 axum WebSocket upgrade；客户端发 `{type: "subscribe", accountId: "..."}` 订阅。

use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;

use qq_farm_core::runtime::runtime_state::{LogEntry, RuntimeEvent};
use crate::context::AdminContext;

/// WS handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(ctx): State<Arc<AdminContext>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, ctx))
}

#[derive(Debug, Deserialize)]
struct SubscribeMsg {
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    account_id: Option<String>,
}

async fn handle_socket(socket: WebSocket, ctx: Arc<AdminContext>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = ctx.engine.runtime_state().subscribe();
    let mut subscribed_account: Option<String> = None;

    // 接收 runtime 事件并转发
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);
    let forward_task = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // 接收 runtime 事件
    let tx_clone = tx.clone();
    let event_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
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
                subscribed_account = parsed.account_id.clone();
                let text = serde_json::to_string(&json!({
                    "type": "subscribed",
                    "accountId": subscribed_account.clone().unwrap_or_else(|| "all".to_string())
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
