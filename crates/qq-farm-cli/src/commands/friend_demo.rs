//! 阶段 1D 验证：完整好友操作端到端 demo。
//!
//! 流程：
//! 1. 启动 mock WS server（GetAll 返回 3 个 mock 好友 + Help/Visit mock OK）
//! 2. 构造 Gateway + FriendService
//! 3. 调 check_friends() 一轮
//! 4. 输出事件流（Checked + GidsSynced + Error）

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use futures::{SinkExt, StreamExt};
use prost::Message as _;
use qq_farm_core::network::encryptor::Encryptor;
use qq_farm_core::network::gateway::{Gateway, GatewayConfig};
use qq_farm_core::proto::generated::gamepb::friendpb::{GameFriend, GetAllReply};
use qq_farm_core::services::friend::scheduler::{FriendEvent, FriendService};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;

#[derive(Debug, ClapArgs)]
pub struct Args {
    /// 跑多少秒后停止
    #[arg(long, default_value_t = 3)]
    pub duration_secs: u64,

    /// 测试 host_gid
    #[arg(long, default_value_t = 1001)]
    pub host_gid: i64,

    /// 巡访 batch 大小
    #[arg(long, default_value_t = 5)]
    pub batch_size: usize,
}

pub fn execute(args: Args) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create tokio runtime")?;
    rt.block_on(run_demo(args))
}

async fn run_demo(args: Args) -> Result<()> {
    println!("[friend-demo] 启动 mock WS server（返回 3 个 mock 好友）...");
    let (port, _server_handle) = start_mock_ws_server().await;
    let mock_url = format!("ws://127.0.0.1:{port}/");
    println!("[friend-demo] mock URL: {mock_url}");

    let gateway = Arc::new(construct_gateway(&mock_url)?);
    gateway.connect().await.context("connect")?;
    gateway.mark_online();
    println!("[friend-demo] gateway connected + marked online");

    let friend = FriendService::new(gateway.clone(), args.batch_size);
    friend.set_host_gid(args.host_gid);
    let mut friend_events = friend.subscribe();

    // 订阅 GidManager 事件
    let mut gid_events = friend.gid_manager().subscribe();
    let gid_task = tokio::spawn(async move {
        while let Ok(ev) = gid_events.recv().await {
            println!("[gid] {ev:?}");
        }
    });

    // 订阅 FriendService 事件
    let friend_event_task = tokio::spawn(async move {
        while let Ok(ev) = friend_events.recv().await {
            match &ev {
                FriendEvent::Checked { batch_size, helped, stolen, banned } => {
                    println!(
                        "[friend] Checked: batch={batch_size} helped={helped} stolen={stolen} banned={banned}"
                    );
                }
                FriendEvent::GidsSynced { count } => {
                    println!("[friend] GidsSynced: count={count}");
                }
                FriendEvent::FarmBanned { host_gid } => {
                    println!("[friend] FarmBanned: host_gid={host_gid}");
                }
                FriendEvent::Error { message } => {
                    println!("[friend] Error: {message}");
                }
            }
        }
    });

    // 跑一次
    println!("[friend-demo] check_friends...");
    match friend.check_friends().await {
        Ok((batch_size, helped, stolen, banned)) => {
            println!(
                "[friend-demo] check_friends 完成: batch={batch_size} helped={helped} stolen={stolen} banned={banned}"
            );
        }
        Err(e) => println!("[friend-demo] check_friends 失败: {e}"),
    }

    // 加 1 个黑名单好友再跑
    friend.strategy().add_blacklist(2001);
    println!("[friend-demo] 加入黑名单 gid=2001，再跑一次...");
    let _ = friend.check_friends().await;

    // 跑 N 秒
    tokio::time::sleep(Duration::from_secs(args.duration_secs)).await;

    // 关闭
    println!("[friend-demo] shutdown...");
    friend.shutdown();
    friend_event_task.abort();
    gid_task.abort();
    println!("[friend-demo] 完成");

    Ok(())
}

fn construct_gateway(mock_url: &str) -> Result<Gateway> {
    let encryptor: Arc<dyn Encryptor> = Arc::new(NoopEncryptor);
    Ok(Gateway::new(
        GatewayConfig {
            server_url: mock_url.to_string(),
            platform: "test".into(),
            os: "linux".into(),
            client_version: "0.1.0".into(),
            auth_code: "demo".into(),
            headers: HashMap::new(),
        },
        encryptor,
    ))
}

struct NoopEncryptor;
impl Encryptor for NoopEncryptor {
    fn encrypt(&self, plaintext: &[u8]) -> qq_farm_core::error::Result<Vec<u8>> {
        Ok(plaintext.to_vec())
    }
    fn decrypt(&self, ciphertext: &[u8]) -> qq_farm_core::error::Result<Vec<u8>> {
        Ok(ciphertext.to_vec())
    }
}

/// Mock WS server：GetAll 返回 3 个 mock 好友；其他 OK
async fn start_mock_ws_server() -> (u16, tokio::task::JoinHandle<()>) {
    use qq_farm_core::proto::generated::gatepb::{Message as GateMessage, Meta};

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
                            let client_seq = req.meta.as_ref().map(|m| m.client_seq).unwrap_or(0);

                            let body = if method == "GetAll" {
                                GetAllReply {
                                    game_friends: vec![
                                        GameFriend { gid: 100, ..Default::default() },
                                        GameFriend { gid: 200, ..Default::default() },
                                        GameFriend { gid: 300, ..Default::default() },
                                    ],
                                    ..Default::default()
                                }
                                .encode_to_vec()
                            } else {
                                // Help / Visit / AcceptApplications：返回空 body
                                vec![]
                            };

                            let resp_meta = Meta {
                                service_name: req
                                    .meta
                                    .as_ref()
                                    .map(|m| m.service_name.clone())
                                    .unwrap_or_default(),
                                method_name: method.clone(),
                                message_type: 2, // Response
                                client_seq,
                                server_seq: 0,
                                error_code: 0,
                                error_message: String::new(),
                                ..Default::default()
                            };
                            let resp = GateMessage {
                                meta: Some(resp_meta),
                                body: body.into(),
                                token: String::new(),
                            };
                            let _ = ws.send(WsMessage::Binary(resp.encode_to_vec())).await;
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
