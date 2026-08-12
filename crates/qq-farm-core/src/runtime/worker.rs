//! 单账号 Worker。
//!
//! 每个账号对应一个 Worker，跑在独立 tokio task 里。
//! Worker 持有一个 [`Scheduler`] 和一个 [`Gateway`]，通过订阅 Notify 事件更新状态。
//!
//! 阶段 1B 范围：Worker 骨架（不实现具体业务）。
//! 业务模块（farm/friend/mall...）在阶段 1C-1E 注入到 worker 里。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::models::Account;
use crate::network::gateway::{Gateway, GatewayConfig};
use crate::network::encryptor::Encryptor;
use crate::proto::generated::gamepb::userpb::{DeviceInfo, ReportData};
use crate::runtime::events::WorkerEvent;
use crate::runtime::scheduler::Scheduler;
use crate::runtime::worker_handle::WorkerHandle;
use crate::runtime::worker_message::WorkerMessage;

/// Worker 启动配置
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// 网关配置
    pub gateway: GatewayConfig,
    /// 状态上报间隔
    pub status_interval: Duration,
    /// TSDK wasm 路径
    pub tsdk_wasm_path: std::path::PathBuf,
    /// 数据目录
    pub data_dir: std::path::PathBuf,
}

/// Worker —— 单账号的运行时
pub struct Worker {
    account: Account,
    config: WorkerConfig,
    scheduler: Scheduler,
    cancel: CancellationToken,
    msg_tx: mpsc::Sender<WorkerMessage>,
    msg_rx: Option<mpsc::Receiver<WorkerMessage>>,
    event_tx: tokio::sync::broadcast::Sender<WorkerEvent>,
}

impl Worker {
    /// 创建 Worker
    pub fn new(
        account: Account,
        config: WorkerConfig,
        event_tx: tokio::sync::broadcast::Sender<WorkerEvent>,
    ) -> Self {
        let namespace = format!("worker:{}", account.id);
        let (msg_tx, msg_rx) = mpsc::channel(32);
        Self {
            account,
            config,
            scheduler: Scheduler::new(namespace),
            cancel: CancellationToken::new(),
            msg_tx,
            msg_rx: Some(msg_rx),
            event_tx,
        }
    }

    /// 拿到控制句柄（用于从外面发消息/取消）
    pub fn handle(&self) -> WorkerHandle {
        WorkerHandle {
            account_id: self.account.id.clone(),
            msg_tx: self.msg_tx.clone(),
            cancel: self.cancel.clone(),
        }
    }

    /// 取出消息接收端（只能调一次，由 `spawn` 内部使用）
    fn take_msg_rx(&mut self) -> mpsc::Receiver<WorkerMessage> {
        self.msg_rx.take().expect("msg_rx already taken")
    }

    /// 启动 worker（spawn 到当前 tokio runtime）。
    ///
    /// 如果传了 `engine`，会在 spawn 完成后构造 WorkerLoop 并注册到 engine，
    /// controller 就能通过 `engine.worker_loop(account_id)` 拿到实例。
    /// 退出时自动 `unregister_worker_loop`。
    pub fn spawn_with_engine(
        mut self,
        engine: Option<Arc<crate::runtime::engine::RuntimeEngine>>,
    ) -> WorkerHandle {
        let handle = self.handle();
        let event_tx = self.event_tx.clone();
        let account_id = self.account.id.clone();
        let account_name = self.account.display_name.clone();
        let cancel = self.cancel.clone();
        let config = self.config.clone();
        let scheduler = self.scheduler.clone();
        let msg_rx = self.take_msg_rx();
        let account = self.account;

        tokio::spawn(async move {
            // 启动事件
            let _ = event_tx.send(WorkerEvent::Started {
                account_id: account_id.clone(),
                account_name: account_name.clone(),
            });

            // 加载 TSDK（每个 worker 独立 runtime）
            let tsdk_data_dir = config.data_dir.join(account_id.as_str());
            let tsdk = match crate::crypto::tsdk::TsdkRuntime::load(
                &config.tsdk_wasm_path,
                tsdk_data_dir.to_string_lossy().to_string(),
            ) {
                Ok(rt) => Arc::new(rt),
                Err(e) => {
                    tracing::error!(account_id = %account_id, "TSDK 加载失败: {e}");
                    let _ = event_tx.send(WorkerEvent::Error {
                        account_id: account_id.clone(),
                        message: format!("TSDK 加载失败: {e}"),
                    });
                    let _ = event_tx.send(WorkerEvent::Stopped {
                        account_id: account_id.clone(),
                        reason: format!("TSDK 加载失败: {e}"),
                    });
                    return;
                }
            };
            let encryptor: Arc<dyn Encryptor> = Arc::new(crate::network::encryptor::TsdkEncryptor::new(tsdk.clone()));

            // 构造 Gateway
            let gateway = Arc::new(Gateway::new(config.gateway.clone(), encryptor));

            // 构造所有 service + WorkerLoop，注册到 engine
            if let Some(eng) = &engine {
                let farm = Arc::new(crate::services::farm::scheduler::FarmService::new(gateway.clone()));
                let friend = Arc::new(crate::services::friend::scheduler::FriendService::new(
                    gateway.clone(),
                    5,
                ));
                let email = Arc::new(crate::services::email::EmailService::new(gateway.clone()));
                let share = Arc::new(crate::services::share::ShareService::new(gateway.clone()));
                let monthcard = Arc::new(crate::services::monthcard::MonthCardService::new(gateway.clone()));
                let qqvip = Arc::new(crate::services::qqvip::QQVipService::new(gateway.clone()));
                let mall = Arc::new(crate::services::mall::MallService::new(gateway.clone()));
                let task = Arc::new(crate::services::task::TaskService::new(gateway.clone()));
                let warehouse = Arc::new(crate::services::warehouse::WarehouseService::new(gateway.clone()));
                let mystery_shop = Arc::new(
                    crate::services::mystery_shop::MysteryShopService::new(gateway.clone()),
                );
                let activity_center = Arc::new(
                    crate::services::activity_center::ActivityCenterService::new(gateway.clone()),
                );

                let worker_loop = Arc::new(crate::runtime::worker_loop::WorkerLoop::new(
                    account.clone(),
                    crate::runtime::worker_loop::WorkerLoopConfig::default(),
                    gateway.clone(),
                    event_tx.clone(),
                    farm,
                    friend,
                    email,
                    share,
                    monthcard,
                    qqvip,
                    mall,
                    task,
                    warehouse,
                    mystery_shop,
                    activity_center,
                ));
                eng.register_worker_loop(&account_id, worker_loop.clone());
                tracing::info!(account_id = %account_id, "WorkerLoop 已注册到 engine");

                // 注册心跳超时回调（重连）
                let eng_for_cb = eng.clone();
                let acc_id_for_cb = account_id.clone();
                worker_loop.on_heartbeat_timeout(move |_acc_id| {
                    tracing::warn!(account_id = %acc_id_for_cb, "心跳超时，触发 worker 重启");
                    // 通过 engine.stop_worker 触发重连
                    eng_for_cb.stop_worker(&acc_id_for_cb);
                });

                // === 1. WS 连接 + 登录 ===
                if let Err(e) = gateway.connect().await {
                    tracing::error!(account_id = %account_id, "WS 连接失败: {e}");
                    let _ = event_tx.send(WorkerEvent::Error {
                        account_id: account_id.clone(),
                        message: format!("WS 连接失败: {e}"),
                    });
                } else {
                    tracing::info!(account_id = %account_id, "WS 已连接，开始登录");

                    // 构造 LoginRequest 字段
                    let device_info = DeviceInfo {
                        client_version: config.gateway.client_version.clone(),
                        sys_software: std::env::consts::OS.to_string(),
                        ..Default::default()
                    };
                    let report_data = ReportData {
                        minigame_channel: "other-qq".to_string(),
                        minigame_platid: 2,
                        ..Default::default()
                    };

                    match gateway.login(&device_info, &report_data, &tsdk).await {
                        Ok(reply) => {
                            if let Some(basic) = &reply.basic {
                                worker_loop.set_gid(basic.gid);
                            }
                            let _ = event_tx.send(WorkerEvent::Started {
                                account_id: account_id.clone(),
                                account_name: account_name.clone(),
                            });
                        }
                        Err(e) => {
                            tracing::error!(account_id = %account_id, "登录失败: {e}");
                            let _ = event_tx.send(WorkerEvent::Error {
                                account_id: account_id.clone(),
                                message: format!("登录失败: {e}"),
                            });
                        }
                    }

                    // === 2. 启动 ACE runtime（5 个定时任务） ===
                    let ace = Arc::new(crate::services::ace::AceShared::new());
                    let sender = Arc::new(crate::services::ace::GatewayAceSender {
                        gateway: gateway.clone(),
                    });
                    ace.start(sender, tsdk.clone());
                }
            }

            // 跑 worker 主循环
            let exit = run_worker_loop(
                account,
                config,
                scheduler,
                msg_rx,
                event_tx.clone(),
                gateway,
                cancel,
            )
            .await;

            // 注销 WorkerLoop
            if let Some(eng) = &engine {
                eng.unregister_worker_loop(&account_id);
            }

            // 退出事件
            let _ = event_tx.send(WorkerEvent::Stopped {
                account_id,
                reason: exit.reason,
            });
        });

        handle
    }

    /// 启动 worker（不注册到 engine）
    pub fn spawn(self) -> WorkerHandle {
        self.spawn_with_engine(None)
    }
}

/// Worker 退出原因
struct WorkerExit {
    reason: String,
}

/// Worker 主循环
async fn run_worker_loop(
    account: Account,
    config: WorkerConfig,
    scheduler: Scheduler,
    mut msg_rx: mpsc::Receiver<WorkerMessage>,
    event_tx: tokio::sync::broadcast::Sender<WorkerEvent>,
    gateway: Arc<Gateway>,
    cancel: CancellationToken,
) -> WorkerExit {
    // 注册状态上报任务
    let account_id = account.id.clone();
    let account_name = account.display_name.clone();
    let event_tx_status = event_tx.clone();
    scheduler.set_interval_task(
        "status_report",
        config.status_interval,
        Arc::new(move || {
            let acc_id = account_id.clone();
            let acc_name = account_name.clone();
            let tx = event_tx_status.clone();
            Box::pin(async move {
                let _ = tx.send(WorkerEvent::Status {
                    account_id: acc_id,
                    account_name: acc_name,
                    status: serde_json::json!({
                        "phase": "online",
                        "note": "阶段 1B 占位",
                    }),
                });
            })
        }),
    );

    let mut gateway = Some(gateway);

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                scheduler.shutdown();
                if let Some(g) = gateway.take() {
                    // 这里如果有活跃连接可以 close
                    let _ = g;
                }
                return WorkerExit { reason: "主动取消".to_string() };
            }
            msg = msg_rx.recv() => {
                match msg {
                    Some(WorkerMessage::Connect) => {
                        if let Some(g) = &gateway {
                            match g.connect().await {
                                Ok(_ws) => {
                                    g.mark_online();
                                    tracing::info!(account_id = %account.id, "worker connected");
                                }
                                Err(e) => {
                                    let _ = event_tx.send(WorkerEvent::Error {
                                        account_id: account.id.clone(),
                                        message: format!("connect failed: {e}"),
                                    });
                                }
                            }
                        }
                    }
                    Some(WorkerMessage::Disconnect) => {
                        // 简化：阶段 1B 不实现完整 disconnect 协议
                        tracing::info!(account_id = %account.id, "worker disconnect requested");
                    }
                    Some(WorkerMessage::ReloadConfig) => {
                        tracing::info!(account_id = %account.id, "config reload requested");
                    }
                    Some(WorkerMessage::Custom { tag, payload }) => {
                        tracing::debug!(account_id = %account.id, tag = %tag, ?payload, "custom msg");
                    }
                    None => {
                        // 发送端 drop
                        scheduler.shutdown();
                        return WorkerExit { reason: "消息通道关闭".to_string() };
                    }
                }
            }
        }
    }
}

// ===== 兼容 trait: Gateway 需要 Arc<Gateway> =====
// 上面 `let gateway = Arc::new(Gateway::new(...))` 但 Worker 用了 `let gateway = Gateway::new(...)` 直接值
// 为了 run_worker_loop 接受 Arc<Gateway>，这里再加一个 wrapper
// —— 实际上 Arc<dyn Gateway> 用 trait object 更干净；阶段 1B 简化用 Arc<Gateway>
