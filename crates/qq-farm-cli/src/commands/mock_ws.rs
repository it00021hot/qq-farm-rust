//! Shared mock WebSocket server helpers for CLI demos.

use futures::{SinkExt, StreamExt};
use prost::Message as _;
use qq_farm_core::proto::generated::gatepb::{Message as GateMessage, MessageType, Meta};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;

type GateHandler = Box<dyn Fn(&str) -> Vec<u8> + Send + Sync + 'static>;

/// Start a simple echo mock WS server (worker-demo).
pub async fn start_echo_mock_ws_server() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(ws) = accept_async(stream).await {
                let mut ws = ws;
                let _ = ws.send(WsMessage::Binary(b"welcome".to_vec())).await;
                while let Some(msg) = ws.next().await {
                    if let Ok(WsMessage::Binary(data)) = msg {
                        let mut echoed = b"DEMO:".to_vec();
                        echoed.extend_from_slice(&data);
                        if ws.send(WsMessage::Binary(echoed)).await.is_err() {
                            break;
                        }
                    } else if matches!(msg, Ok(WsMessage::Close(_))) {
                        break;
                    }
                }
            }
        }
    });
    (port, handle)
}

/// Start a gate-protocol mock WS server; `handler` maps RPC method name → response body.
pub async fn start_gate_mock_ws_server(handler: GateHandler) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(mut ws) = accept_async(stream).await {
                while let Some(msg) = ws.next().await {
                    let msg = match msg {
                        Ok(m) => m,
                        Err(_) => break,
                    };
                    if let WsMessage::Binary(data) = msg {
                        if let Ok(req) = GateMessage::decode(&data[..]) {
                            let method = req
                                .meta
                                .as_ref()
                                .map(|m| m.method_name.clone())
                                .unwrap_or_default();
                            let body = handler(&method);
                            let resp_bytes = build_gate_response(&req, body);
                            let _ = ws.send(WsMessage::Binary(resp_bytes)).await;
                        }
                    } else if matches!(msg, WsMessage::Close(_)) {
                        break;
                    }
                }
            }
        }
    });
    (port, handle)
}

/// Build a gate response frame from a request and response body bytes.
pub fn build_gate_response(req: &GateMessage, body: Vec<u8>) -> Vec<u8> {
    let method = req
        .meta
        .as_ref()
        .map(|m| m.method_name.clone())
        .unwrap_or_default();
    let client_seq = req.meta.as_ref().map(|m| m.client_seq).unwrap_or(0);

    let resp_meta = Meta {
        service_name: req
            .meta
            .as_ref()
            .map(|m| m.service_name.clone())
            .unwrap_or_default(),
        method_name: method,
        message_type: MessageType::Response as i32,
        client_seq,
        server_seq: 0,
        error_code: 0,
        error_message: String::new(),
        ..Default::default()
    };
    GateMessage {
        meta: Some(resp_meta),
        body: body.into(),
        token: String::new(),
    }
    .encode_to_vec()
}
