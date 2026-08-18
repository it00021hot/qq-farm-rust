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
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::client::generate_key;
use tokio_tungstenite::tungstenite::protocol::{
    CloseFrame, Message as WsMessage, Role, WebSocketConfig,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::network::error::{NetworkError, Result};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

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
    pub async fn connect(
        url: &str,
        options: ConnectOptions,
    ) -> Result<(Self, mpsc::Receiver<ReceivedFrame>)> {
        // 对齐 Go gorilla / Node `ws`：握手头用规范大小写（Origin / User-Agent）。
        // tungstenite 会把额外头写成小写，腾讯网关经常直接 400。
        #[allow(deprecated)]
        let mut ws_config = WebSocketConfig::default();
        ws_config.max_message_size = Some(100 * 1024 * 1024);
        ws_config.max_frame_size = Some(100 * 1024 * 1024);
        ws_config.write_buffer_size = 0;
        let stream = dial_gateway_ws(url, &options, ws_config)
            .await
            .map_err(|e| NetworkError::WebSocket(format!("connect: {e}")))?;

        let rx_capacity = if options.rx_capacity == 0 { 256 } else { options.rx_capacity };
        let (frame_tx, frame_rx) = mpsc::channel::<ReceivedFrame>(rx_capacity);
        let (cmd_tx, cmd_rx) = mpsc::channel::<WsCommand>(32);

        // Spawn IO task
        tokio::spawn(run_io_task(stream, frame_tx, cmd_rx));

        Ok((Self { tx: cmd_tx }, frame_rx))
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

const WS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

/// 对齐 Go gorilla / Node `ws`：自己写握手字节，保留 `Origin` / `User-Agent` 规范大小写。
/// tungstenite 会把额外头写成小写（只特判 Origin），腾讯网关经常直接 400。
async fn dial_gateway_ws(
    url: &str,
    options: &ConnectOptions,
    ws_config: WebSocketConfig,
) -> std::result::Result<WsStream, String> {
    tokio::time::timeout(WS_HANDSHAKE_TIMEOUT, dial_gateway_ws_inner(url, options, ws_config))
        .await
        .map_err(|_| "handshake timeout".to_string())?
}

async fn dial_gateway_ws_inner(
    url: &str,
    options: &ConnectOptions,
    ws_config: WebSocketConfig,
) -> std::result::Result<WsStream, String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid url: {e}"))?;
    let tls = match parsed.scheme() {
        "wss" | "https" => true,
        "ws" | "http" => false,
        other => return Err(format!("unsupported scheme: {other}")),
    };
    let host = parsed.host_str().ok_or_else(|| "missing host".to_string())?;
    let default_port = if tls { 443 } else { 80 };
    let port = parsed.port().unwrap_or(default_port);
    let path = {
        let p = parsed.path();
        let path = if p.is_empty() { "/" } else { p };
        match parsed.query() {
            Some(q) => format!("{path}?{q}"),
            None => path.to_string(),
        }
    };
    let host_header = host_header_value(&parsed, tls, port);
    let key = generate_key();
    let request =
        build_client_handshake(&host_header, &path, &key, &options.headers, &options.subprotocols);

    let addrs = parsed.socket_addrs(|| Some(default_port)).map_err(|e| format!("resolve: {e}"))?;
    if addrs.is_empty() {
        return Err(format!("no address for {host}"));
    }
    let tcp = TcpStream::connect(&*addrs).await.map_err(|e| format!("tcp: {e}"))?;
    let _ = tcp.set_nodelay(true);

    let mut stream = if tls {
        let connector = native_tls::TlsConnector::new().map_err(|e| format!("tls: {e}"))?;
        let connector = tokio_native_tls::TlsConnector::from(connector);
        let tls_stream = connector.connect(host, tcp).await.map_err(|e| format!("tls: {e}"))?;
        MaybeTlsStream::NativeTls(tls_stream)
    } else {
        MaybeTlsStream::Plain(tcp)
    };

    stream.write_all(request.as_bytes()).await.map_err(|e| format!("write handshake: {e}"))?;
    stream.flush().await.map_err(|e| format!("flush handshake: {e}"))?;

    let (head, leftover) = read_http_head(&mut stream).await?;
    parse_switch_status(&head)?;

    Ok(WebSocketStream::from_partially_read(stream, leftover, Role::Client, Some(ws_config)).await)
}

fn host_header_value(parsed: &url::Url, tls: bool, port: u16) -> String {
    let host = match parsed.host() {
        Some(url::Host::Ipv6(addr)) => format!("[{addr}]"),
        Some(url::Host::Domain(d)) => d.to_string(),
        Some(url::Host::Ipv4(addr)) => addr.to_string(),
        None => String::new(),
    };
    let default_port = if tls { 443 } else { 80 };
    if port == default_port {
        host
    } else {
        format!("{host}:{port}")
    }
}

fn build_client_handshake(
    host_header: &str,
    path_and_query: &str,
    key: &str,
    headers: &HashMap<String, String>,
    subprotocols: &[String],
) -> String {
    let mut req = format!(
        "GET {path_and_query} HTTP/1.1\r\nHost: {host_header}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n"
    );

    let mut origin = None;
    let mut user_agent = None;
    let mut extras = Vec::new();
    for (k, v) in headers {
        if header_is_reserved(k) || v.contains('\r') || v.contains('\n') {
            continue;
        }
        if k.eq_ignore_ascii_case("origin") {
            origin = Some(v.as_str());
        } else if k.eq_ignore_ascii_case("user-agent") {
            user_agent = Some(v.as_str());
        } else {
            extras.push((k.as_str(), v.as_str()));
        }
    }
    if let Some(v) = origin {
        req.push_str("Origin: ");
        req.push_str(v);
        req.push_str("\r\n");
    }
    if let Some(v) = user_agent {
        req.push_str("User-Agent: ");
        req.push_str(v);
        req.push_str("\r\n");
    }
    for (k, v) in extras {
        req.push_str(k);
        req.push_str(": ");
        req.push_str(v);
        req.push_str("\r\n");
    }
    for sp in subprotocols {
        if sp.contains('\r') || sp.contains('\n') {
            continue;
        }
        req.push_str("Sec-WebSocket-Protocol: ");
        req.push_str(sp);
        req.push_str("\r\n");
    }
    req.push_str("\r\n");
    req
}

fn header_is_reserved(name: &str) -> bool {
    name.eq_ignore_ascii_case("host")
        || name.eq_ignore_ascii_case("upgrade")
        || name.eq_ignore_ascii_case("connection")
        || name.eq_ignore_ascii_case("sec-websocket-key")
        || name.eq_ignore_ascii_case("sec-websocket-version")
        || name.eq_ignore_ascii_case("sec-websocket-protocol")
}

async fn read_http_head<S: AsyncRead + Unpin>(
    stream: &mut S,
) -> std::result::Result<(Vec<u8>, Vec<u8>), String> {
    let mut buf = Vec::with_capacity(1024);
    let mut tmp = [0u8; 512];
    loop {
        if buf.len() > 64 * 1024 {
            return Err("handshake response too large".into());
        }
        let n = stream.read(&mut tmp).await.map_err(|e| format!("read handshake: {e}"))?;
        if n == 0 {
            return Err("connection closed during handshake".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let end = pos + 4;
            let leftover = buf.split_off(end);
            return Ok((buf, leftover));
        }
    }
}

fn parse_switch_status(head: &[u8]) -> std::result::Result<(), String> {
    let text = String::from_utf8_lossy(head);
    let line = text.lines().next().unwrap_or("").trim();
    let mut parts = line.splitn(3, ' ');
    let _http = parts.next();
    let code = parts.next().unwrap_or("");
    let reason = parts.next().unwrap_or("");
    if code == "101" {
        Ok(())
    } else if code.is_empty() {
        Err(format!("invalid handshake response: {line}"))
    } else {
        Err(format!("HTTP error: {code} {reason}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::protocol::Message;

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

        let (client, mut rx) =
            WsClient::connect(&url, ConnectOptions::default()).await.expect("connect");

        client.send(b"hello").await.expect("send");
        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("recv");
        assert_eq!(frame.bytes, b"ECHO:hello");

        client.close().await.expect("close");
    }

    #[test]
    fn handshake_uses_canonical_origin_and_user_agent() {
        let mut headers = HashMap::new();
        headers.insert("origin".into(), "https://gate-obt.nqf.qq.com".into());
        headers.insert("user-agent".into(), "Mozilla/5.0 Test".into());
        let req = build_client_handshake(
            "gate-obt.nqf.qq.com",
            "/prod/ws?platform=wx&os=Windows&ver=1&code=abc",
            "dGhlIHNhbXBsZSBrZXk=",
            &headers,
            &[],
        );
        assert!(req.starts_with("GET /prod/ws?platform=wx&os=Windows&ver=1&code=abc HTTP/1.1\r\n"));
        assert!(req.contains("\r\nHost: gate-obt.nqf.qq.com\r\n"));
        assert!(req.contains("\r\nOrigin: https://gate-obt.nqf.qq.com\r\n"));
        assert!(req.contains("\r\nUser-Agent: Mozilla/5.0 Test\r\n"));
        assert!(!req.contains("\r\nuser-agent:"));
        assert!(!req.contains("\r\norigin:"));
        assert!(req.ends_with("\r\n\r\n"));
    }

    #[tokio::test]
    async fn handshake_wire_bytes_keep_header_case() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 512];
            loop {
                let n = stream.read(&mut tmp).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
        });

        let mut opts = ConnectOptions::default();
        opts.headers.insert("Origin".into(), "https://gate-obt.nqf.qq.com".into());
        opts.headers.insert("User-Agent".into(), "Mozilla/5.0 Test".into());
        let url = format!("ws://127.0.0.1:{port}/prod/ws?platform=wx&code=abc");
        let _ = WsClient::connect(&url, opts).await;
        let req = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
            .await
            .expect("timeout")
            .expect("handshake");
        assert!(req.contains("GET /prod/ws?platform=wx&code=abc HTTP/1.1\r\n"));
        assert!(req.contains("\r\nUser-Agent: Mozilla/5.0 Test\r\n"));
        assert!(req.contains("\r\nOrigin: https://gate-obt.nqf.qq.com\r\n"));
        assert!(!req.contains("\r\nuser-agent:"));
    }
}
