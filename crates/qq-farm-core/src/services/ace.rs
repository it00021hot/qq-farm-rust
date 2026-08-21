//! ACE 反作弊 runtime。
//!
//! 1:1 对应原 `core/src/services/ace.ts`（66 行）。

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;

use futures::FutureExt;
use parking_lot::Mutex;
use prost::Message as _;
use tokio::sync::broadcast;

use crate::crypto::tsdk::TsdkRuntime;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::acepb::{AntiDataReply, AntiDataRequest};
use crate::runtime::events::WorkerEvent;
use crate::runtime::scheduler::{Scheduler, TaskFn};

/// ACE sender 抽象（gateway 提供）
#[async_trait::async_trait]
pub trait AceSender: Send + Sync + 'static {
    /// 发请求给服务器；返回 body 字节（等到回包或断线，不走业务锁）
    async fn send(&self, service: &str, method: &str, body: &[u8])
        -> crate::error::Result<Vec<u8>>;
}

/// ACE runtime 共享状态
pub struct AceShared {
    /// sender 函数（由 gateway 提供）
    sender: Mutex<Option<Arc<dyn AceSender>>>,
    /// TSDK runtime（外部注入）
    tsdk: Mutex<Option<Arc<TsdkRuntime>>>,
    /// 上次 speed_check 时间戳
    last_speed_check_at: AtomicI64,
    /// readyLogged 标志
    ready_logged: AtomicBool,
    /// 防止 AntiData 重入
    request_running: AtomicBool,
    /// scheduler
    scheduler: Scheduler,
    /// worker 事件总线（用于发 WasmReset）
    event_tx: Mutex<Option<broadcast::Sender<WorkerEvent>>>,
    /// 账号名（用于事件）
    account_name: Mutex<String>,
}

impl AceShared {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sender: Mutex::new(None),
            tsdk: Mutex::new(None),
            last_speed_check_at: AtomicI64::new(0),
            ready_logged: AtomicBool::new(false),
            request_running: AtomicBool::new(false),
            scheduler: Scheduler::new("ace"),
            event_tx: Mutex::new(None),
            account_name: Mutex::new(String::new()),
        }
    }

    /// 注入 worker 事件总线 + 账号名（用于 WasmReset 事件）
    pub fn set_event_sink(&self, event_tx: broadcast::Sender<WorkerEvent>, account_name: String) {
        *self.event_tx.lock() = Some(event_tx);
        *self.account_name.lock() = account_name;
    }

    /// 启动 ACE runtime（注册 5 个定时任务）
    pub fn start(self: &Arc<Self>, sender: Arc<dyn AceSender>, tsdk: Arc<TsdkRuntime>) {
        // 清理旧状态
        self.stop(false);

        *self.sender.lock() = Some(sender);
        *self.tsdk.lock() = Some(tsdk);
        self.ready_logged.store(false, Ordering::SeqCst);
        self.last_speed_check_at.store(crate::utils::time::now_ms(), Ordering::SeqCst);

        // 1. anti_data 5s
        self.scheduler.set_interval_task(
            "anti_data",
            std::time::Duration::from_secs(5),
            ace_anti_data_task(self.clone()),
        );

        // 2. process_received_data 5s
        self.scheduler.set_interval_task(
            "process_received_data",
            std::time::Duration::from_secs(5),
            ace_simple_task(self.clone(), |s| {
                if let Some(tsdk) = s.tsdk.lock().as_ref() {
                    let _ = tsdk.process_received_data();
                }
            }),
        );

        // 3. heartbeat_tick 25s
        self.scheduler.set_interval_task(
            "heartbeat_tick",
            std::time::Duration::from_secs(25),
            ace_simple_task(self.clone(), |s| {
                if let Some(tsdk) = s.tsdk.lock().as_ref() {
                    let _ = tsdk.heartbeat_tick();
                }
            }),
        );

        // 4. speed_check 30s
        self.scheduler.set_interval_task(
            "speed_check",
            std::time::Duration::from_secs(30),
            ace_simple_task(self.clone(), |s| {
                let now = crate::utils::time::now_ms();
                let last = s.last_speed_check_at.swap(now, Ordering::SeqCst);
                let elapsed = if last == 0 { 30_000 } else { (now - last).max(0) as u64 };
                if let Some(tsdk) = s.tsdk.lock().as_ref() {
                    let _ = tsdk.detect_speed_hack(elapsed);
                }
            }),
        );

        // 5. status_report 150s
        self.scheduler.set_interval_task(
            "status_report",
            std::time::Duration::from_secs(150),
            ace_simple_task(self.clone(), |s| {
                if let Some(tsdk) = s.tsdk.lock().as_ref() {
                    let _ = tsdk.send_status();
                }
            }),
        );
    }

    /// 停止 ACE runtime
    pub fn stop(&self, destroy_wasm: bool) {
        self.scheduler.clear_all();
        self.request_running.store(false, Ordering::SeqCst);
        self.ready_logged.store(false, Ordering::SeqCst);
        *self.sender.lock() = None;
        if destroy_wasm {
            if let Some(tsdk) = self.tsdk.lock().take() {
                tsdk.destroy();
            }
        } else {
            *self.tsdk.lock() = None;
        }
        self.last_speed_check_at.store(0, Ordering::SeqCst);
    }

    /// 上报 AntiData
    pub async fn send_anti_data(self: &Arc<Self>) {
        if self
            .request_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        // RAII 复位：保证 panic 路径也会释放 request_running，避免调度永久卡死
        let _reset_guard = RequestRunningGuard {
            flag: &self.request_running,
        };
        let inner_result = AssertUnwindSafe(self.send_anti_data_inner())
            .catch_unwind()
            .await;
        match inner_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "ACE AntiData 上报失败");
            }
            Err(panic_payload) => {
                let msg = crate::runtime::safe_spawn::format_panic_payload(panic_payload);
                tracing::error!(panic = %msg, "ACE AntiData 上报任务 panic");
            }
        }
    }

    async fn send_anti_data_inner(self: &Arc<Self>) -> crate::error::Result<()> {
        let tsdk_clone = {
            let guard = self.tsdk.lock();
            guard.as_ref().cloned()
        };
        let Some(tsdk) = tsdk_clone else {
            return Ok(());
        };
        // 若 wasm 已请求重置（pending_reset）且 worker 还没来重建，停止 anti_data
        // 避免在坏状态 wasm 上继续消费任务；心跳也会走同样的短路。
        if tsdk.is_reset_pending() {
            return Ok(());
        }
        let data = match tsdk.get_data_to_server() {
            Ok(d) => d,
            Err(e) => {
                // 任何 wasm 错误都已经在内部累计到 consecutive_fail_count；
                // 若已达阈值，is_reset_pending() 会返回 true，这里再发一次事件兜底
                // （防止 ace 任务自己先踩到边缘）。
                if tsdk.is_reset_pending() {
                    self.emit_wasm_reset(tsdk.consecutive_fail_count(), format!("get_data_to_server failed: {e}"));
                }
                return Err(e);
            }
        };
        if data.is_empty() {
            return Ok(());
        }

        let sender = {
            let guard = self.sender.lock();
            guard.as_ref().cloned()
        };
        let Some(sender) = sender else {
            return Ok(());
        };

        let req = AntiDataRequest { data: prost::bytes::Bytes::from(data.clone()) };
        let body = req.encode_to_vec();

        let reply_body = sender.send("gamepb.acepb.AceService", "AntiData", &body).await?;

        let reply = AntiDataReply::decode(reply_body.as_slice())?;
        if !reply.result.is_empty() {
            tsdk.send_data_from_server(&reply.result)?;
            if !self.ready_logged.swap(true, Ordering::SeqCst) {
                tracing::info!(
                    "ACE 链路正常: 上报 {} 字节，回灌 {} 字节",
                    data.len(),
                    reply.result.len()
                );
            }
        }
        Ok(())
    }
}

impl Default for AceShared {
    fn default() -> Self {
        Self::new()
    }
}

impl AceShared {
    /// 发 WasmReset 事件给 worker。worker 收到后会重建 TSDK。
    fn emit_wasm_reset(&self, consecutive_fail_count: u32, reason: String) {
        let tx = self.event_tx.lock().clone();
        if let Some(tx) = tx {
            // 取出 account_id 路径：worker 通过 set_event_sink 传进来时
            // 没法直接拿到 account_id；这里用一个临时约定：调用方传入。
            // 实际场景：worker 侧会订阅所有事件并按 WorkerEvent::WasmReset 的
            // 字段处理（account_id 直接来自事件）。但当前接口没让 worker 传
            // account_id，因此这里先从 sender 反查不到——本节代码仅在内部被
            // 触发，account_id 会在更上层 worker 装配时通过其它方式通知。
            // **暂时** 我们把 account_id 留空字符串，worker 端需要从外部
            // 找到对应的 account。
            let account_id = String::new();
            let account_name = self.account_name.lock().clone();
            let _ = tx.send(WorkerEvent::WasmReset {
                account_id,
                account_name,
                consecutive_fail_count,
                reason,
            });
        }
    }
}

// ===== 任务构造 helper =====

fn ace_anti_data_task(shared: Arc<AceShared>) -> TaskFn {
    Arc::new(move || {
        let shared = shared.clone();
        Box::pin(async move {
            shared.send_anti_data().await;
        })
    })
}

fn ace_simple_task<F>(shared: Arc<AceShared>, f: F) -> TaskFn
where
    F: Fn(&AceShared) + Send + Sync + 'static,
{
    let f = Arc::new(f);
    Arc::new(move || {
        let shared = shared.clone();
        let f = f.clone();
        Box::pin(async move {
            f(&shared);
        })
    })
}

// ===== Gateway 适配器 =====

/// 把 `Arc<Gateway>` 适配成 `AceSender`
pub struct GatewayAceSender {
    pub gateway: Arc<Gateway>,
}

#[async_trait::async_trait]
impl AceSender for GatewayAceSender {
    async fn send(
        &self,
        service: &str,
        method: &str,
        body: &[u8],
    ) -> crate::error::Result<Vec<u8>> {
        self.gateway
            .request_unlocked(service, method, body)
            .await
            .map_err(crate::error::Error::Network)
    }
}

// ===== RAII 工具 =====

/// RAII guard for `request_running`：Drop 时复位为 false，保证 panic 路径也能解锁。
struct RequestRunningGuard<'a> {
    flag: &'a AtomicBool,
}

impl Drop for RequestRunningGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ace_shared_new() {
        let s = AceShared::new();
        assert_eq!(s.scheduler.task_count(), 0);
    }

    #[test]
    fn stop_clears_state() {
        let s = AceShared::new();
        s.stop(false);
        assert!(!s.ready_logged.load(Ordering::SeqCst));
        assert_eq!(s.scheduler.task_count(), 0);
    }
}
