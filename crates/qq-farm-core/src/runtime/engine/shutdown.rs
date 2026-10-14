use super::RuntimeEngine;
use std::time::Duration;

impl RuntimeEngine {
    /// 关闭启动入口并取消会话和定时重连；可重复调用。
    pub fn shutdown(&self) {
        let _admission = self.worker_admission.write();
        self.shutdown_token.cancel();
        let handles: Vec<_> = self.workers.write().drain().collect();
        for (id, handle) in handles {
            handle.cancel();
            if let Some(worker) = self.runtime_state.workers.lock().get_mut(&id) {
                worker.stopping = true;
            }
        }
        self.worker_loops.write().clear();
        let mut reconnect = self.wx_reconnect.write();
        for (_, task) in reconnect.tasks.drain() {
            task.abort();
        }
        reconnect.inflight.clear();
        reconnect.attempts.clear();
    }

    /// 等待游戏任务结束；超过全局等待时长则中止剩余任务并收回句柄。
    /// 调用方在此方法返回且宿主 runtime 停止前应继续持有数据目录锁。
    pub async fn shutdown_and_wait(&self, timeout: Duration) {
        self.shutdown();
        let _wait = self.shutdown_wait.lock().await;
        let mut tasks: Vec<_> =
            self.worker_tasks.write().drain().map(|(_, (_, task))| task).collect();
        let qq_bot = self.qq_bot();
        let finish = async {
            qq_bot.stop_all().await;
            futures::future::join_all(tasks.iter_mut()).await;
        };
        if tokio::time::timeout(timeout, finish).await.is_err() {
            tracing::warn!("引擎退出等待超时，中止剩余游戏任务");
            for task in &tasks {
                task.abort();
            }
            futures::future::join_all(tasks).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::engine::EngineConfig;
    use std::sync::Arc;

    #[tokio::test]
    async fn shutdown_rejects_new_workers_and_drains_cancelled_tasks() {
        let engine = Arc::new(RuntimeEngine::assemble(EngineConfig::default()));
        let task = tokio::spawn(std::future::pending::<()>());
        let aborted = task.abort_handle();
        engine.worker_tasks.write().insert("pending".into(), (1, task));
        engine.shutdown_and_wait(Duration::from_millis(10)).await;
        assert!(aborted.is_finished());
        assert!(engine.worker_tasks.read().is_empty());
        assert!(engine
            .start_worker(crate::models::AccountSession::new("late", "code", "late"))
            .is_err());
        engine.shutdown_and_wait(Duration::from_millis(10)).await;
    }

    #[tokio::test]
    async fn shutdown_waits_for_worker_cleanup_and_closes_background_loops() {
        let engine = Arc::new(RuntimeEngine::assemble(EngineConfig::default()));
        engine.spawn_event_bridge();
        engine.spawn_wx_keepalive();
        let token = engine.shutdown_token.clone();
        let (finished, mut completion) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            token.cancelled().await;
            let _ = finished.send(());
        });
        engine.worker_tasks.write().insert("graceful".into(), (1, task));
        engine.shutdown_and_wait(Duration::from_secs(1)).await;
        assert_eq!(completion.try_recv(), Ok(()));
        tokio::time::timeout(Duration::from_secs(1), async {
            while Arc::strong_count(&engine) > 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("background loops release their engine subscriptions");
    }
}
