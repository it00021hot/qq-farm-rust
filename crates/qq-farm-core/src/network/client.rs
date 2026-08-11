//! WebSocket 客户端。
//!
//! 用 **actor 模式**：spawn 一个 task 负责所有 IO，外层通过 mpsc 命令信道控制。
//!
//! ## 用法
//!
//! ```ignore
//! use qq_farm_core::network::client::{WsClient, ConnectOptions};
//!
//! let (client, mut rx) = WsClient::connect("wss://example.com/ws", ConnectOptions::default()).await?;
//! client.send(&bytes).await?;
//! while let Some(frame) = rx.recv().await {
//!     // 处理 frame
//! }
//! client.close().await?;
//! ```

use std::collections::HashMap;

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::client::{generate_key, Request as WsRequest};
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message as WsMessage};
use tokio_tungstenite::{connect_async, WebSocketStream};

use crate::network::error::{NetworkError, Result};

type WsStream = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// 接收到的二进制帧
#[derive(Debug, Clone)]
pub struct ReceivedFrame {
    pub bytes: Vec<u8>,
}

/// 连接选项
#[derive(Debug, Clone, Default)]
pub struct ConnectOptions {
    /// 自定义 headers
    pub headers: HashMap<String, String>,
    /// 子协议
    pub subprotocols: Vec<String>,
    /// 接收 channel 容量
    pub rx_capacity: usize,
}

/// Actor 任务内部命令
enum WsCommand {
    /// 发送二进制
    Send(Vec<u8>),
    /// 主动关闭
    Close(Option<CloseFrame<'static>>),
}

/// WebSocket 客户端（轻量 handle）
#[derive(Clone)]
pub struct WsClient {
    tx: mpsc::Sender<WsCommand>,
}

impl WsClient {
    /// 连接到指定 URL
    ///
    /// # Errors
    /// - URL 非法、TCP/TLS 握手失败、WebSocket upgrade 失败
    pub async fn connect(url: &str, options: ConnectOptions) -> Result<(Self, mpsc::Receiver<ReceivedFrame>)> {
        // 构造带 headers 的 Request
        let mut request = WsRequest::builder()
            .method("GET")
            .uri(url)
            .header("Host", extract_host(url).unwrap_or_default())
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Key", generate_key())
            .header("Sec-WebSocket-Version", "13");

        for (k, v) in &options.headers {
            // 跳过 Host/Upgrade/Connection/Sec-WebSocket-*，这些上面已经设置
            if k.starts_with("Sec-WebSocket-") || k == "Upgrade" || k == "Connection" || k == "Host" {
                continue;
            }
            request = request.header(k.as_str(), v.as_str());
        }
        for sp in &options.subprotocols {
            request = request.header("Sec-WebSocket-Protocol", sp.as_str());
        }
        let request = request
            .body(())
            .map_err(|e| NetworkError::WebSocket(format!("build request: {e}")))?;

        let (stream, _response) = connect_async(request)
            .await
            .map_err(|e| NetworkError::WebSocket(format!("connect: {e}")))?;

        let rx_capacity = if options.rx_capacity == 0 { 64 } else { options.rx_capacity };
        let (frame_tx, frame_rx) = mpsc::channel::<ReceivedFrame>(rx_capacity);
        let (cmd_tx, cmd_rx) = mpsc::channel::<WsCommand>(32);

        // Spawn IO task
        tokio::spawn(run_io_task(stream, frame_tx, cmd_rx));

        Ok((
            Self { tx: cmd_tx },
            frame_rx,
        ))
    }

    /// 发送一帧
    ///
    /// # Errors
    /// - 内部 channel 已关闭（连接断开）
    pub async fn send(&self, bytes: &[u8]) -> Result<()> {
        self.tx
            .send(WsCommand::Send(bytes.to_vec()))
            .await
            .map_err(|_| NetworkError::WebSocket("send channel closed".into()))
    }

    /// 主动关闭
    pub async fn close(&self) -> Result<()> {
        self.tx
            .send(WsCommand::Close(None))
            .await
            .map_err(|_| NetworkError::WebSocket("close channel closed".into()))
    }
}

/// IO task：在独立 task 中跑读循环 + 写循环
async fn run_io_task(
    mut stream: WsStream,
    frame_tx: mpsc::Sender<ReceivedFrame>,
    mut cmd_rx: mpsc::Receiver<WsCommand>,
) {
    loop {
        tokio::select! {
            // 优先处理读
            biased;
            frame = stream.next() => {
                match frame {
                    Some(Ok(WsMessage::Binary(data))) => {
                        if frame_tx.send(ReceivedFrame { bytes: data.to_vec() }).await.is_err() {
                            // 上层已 drop
                            break;
                        }
                    }
                    Some(Ok(WsMessage::Close(frame))) => {
                        tracing::info!(?frame, "ws closed by server");
                        break;
                    }
                    Some(Ok(WsMessage::Ping(data))) => {
                        if stream.send(WsMessage::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(WsMessage::Pong(_))) => {
                        // ignore
                    }
                    Some(Ok(WsMessage::Text(_))) => {
                        tracing::warn!("ws received text frame, expected binary");
                    }
                    Some(Ok(WsMessage::Frame(_))) => {
                        // 裸 frame，tungstenite 内部一般不暴露给上层
                    }
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "ws stream error");
                        break;
                    }
                    None => {
                        // stream closed
                        break;
                    }
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(WsCommand::Send(bytes)) => {
                        if stream.send(WsMessage::Binary(bytes)).await.is_err() {
                            break;
                        }
                    }
                    Some(WsCommand::Close(frame)) => {
                        let _ = stream.send(WsMessage::Close(frame)).await;
                        let _ = stream.close(None).await;
                        break;
                    }
                    None => {
                        // 所有 handle 都 drop 了
                        let _ = stream.close(None).await;
                        break;
                    }
                }
            }
        }
    }
    tracing::debug!("ws IO task exited");
}

fn extract_host(url: &str) -> Option<String> {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::protocol::Message;
    use tokio_tungstenite::accept_async;

    /// 启动一个本地 mock WS server，回显收到的 binary 帧
    async fn start_mock_ws() -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let ws_stream = match accept_async(stream).await {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                tokio::spawn(async move {
                    let mut ws = ws_stream;
                    // 把收到的 binary 回显，前面加 "ECHO:" 标记
                    while let Some(msg) = ws.next().await {
                        if let Ok(Message::Binary(data)) = msg {
                            let mut echoed = b"ECHO:".to_vec();
                            echoed.extend_from_slice(&data);
                            if ws.send(Message::Binary(echoed)).await.is_err() {
                                break;
                            }
                        }
                    }
                });
            }
        });
        (port, handle)
    }

    #[tokio::test]
    async fn connect_and_echo() {
        let (port, _h) = start_mock_ws().await;
        let url = format!("ws://127.0.0.1:{port}/");

        let (client, mut rx) = WsClient::connect(&url, ConnectOptions::default())
            .await
            .expect("connect");

        client.send(b"hello").await.expect("send");
        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("recv");
        assert_eq!(frame.bytes, b"ECHO:hello");

        client.close().await.expect("close");
    }
}
