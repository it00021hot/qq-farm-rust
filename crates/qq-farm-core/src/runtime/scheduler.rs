//! 任务调度器。
//!
//! 与原 `core/src/services/scheduler.ts` 对齐：
//! - `set_interval_task(name, interval_ms, fn)`：按间隔循环执行
//! - `set_timeout_task(name, delay_ms, fn)`：延迟一次性执行
//! - `clear(name)`：取消任务
//! - `clear_all()`：取消所有任务
//!
//! ## 命名空间
//!
//! 每个 Scheduler 实例有独立 namespace（多个账号 worker 各持一个）。
//! `Scheduler::registry()` 提供全局注册表快照（用于 UI 展示）。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::Notify;
use tokio::task::AbortHandle;
use tokio_util::sync::CancellationToken;

/// 任务状态（用于快照）
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskState {
    pub name: String,
    pub kind: String, // "interval" | "timeout"
    pub interval_ms: Option<u64>,
    pub delay_ms: Option<u64>,
    pub running: bool,
}

/// 调度器快照
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchedulerSnapshot {
    pub namespace: String,
    pub created_at_ms: i64,
    pub task_count: usize,
    pub tasks: Vec<TaskState>,
}

/// 任务函数类型
pub type TaskFn = Arc<dyn Fn() -> futures::future::BoxFuture<'static, ()> + Send + Sync + 'static>;

/// 调度器
#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<SchedulerInner>,
}

struct SchedulerInner {
    namespace: String,
    created_at_ms: i64,
    /// 已注册任务（按 name）
    tasks: Mutex<HashMap<String, RegisteredTask>>,
    /// 取消信号
    cancel: CancellationToken,
    /// 全局注册表
    registry: Arc<SchedulerRegistry>,
}

struct RegisteredTask {
    state: TaskState,
    abort: AbortHandle,
}

impl Scheduler {
    /// 创建调度器（自动加入全局注册表）
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        let namespace = namespace.into();
        let registry = SchedulerRegistry::global();
        registry.insert(&namespace);
        Self {
            inner: Arc::new(SchedulerInner {
                namespace,
                created_at_ms: chrono::Utc::now().timestamp_millis(),
                tasks: Mutex::new(HashMap::new()),
                cancel: CancellationToken::new(),
                registry,
            }),
        }
    }

    /// 命名空间名
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.inner.namespace
    }

    /// 注册一个 interval 任务
    ///
    /// 如果同名任务已存在，会被替换
    pub fn set_interval_task(
        &self,
        name: &str,
        interval: Duration,
        task: TaskFn,
    ) {
        self.clear(name);
        let cancel = self.inner.cancel.clone();
        let interval_ms = interval.as_millis() as u64;
        let interval_tokio = interval;
        let task_name = name.to_string();
        let task_for_log = task.clone();

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval_tokio);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // 第一次立即触发
            ticker.tick().await;
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {
                        let task = task_for_log.clone();
                        task().await;
                    }
                }
            }
            tracing::trace!(name = %task_name, "interval task cancelled");
        });
        let abort = handle.abort_handle();

        let state = TaskState {
            name: name.to_string(),
            kind: "interval".to_string(),
            interval_ms: Some(interval_ms),
            delay_ms: None,
            running: true,
        };
        self.inner.tasks.lock().insert(name.to_string(), RegisteredTask { state, abort });
    }

    /// 注册一个 timeout 任务（一次性延迟执行）
    pub fn set_timeout_task(&self, name: &str, delay: Duration, task: TaskFn) {
        self.clear(name);
        let cancel = self.inner.cancel.clone();
        let delay_ms = delay.as_millis() as u64;
        let task_name = name.to_string();
        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(delay) => {
                    task().await;
                }
            }
            tracing::trace!(name = %task_name, "timeout task fired");
        });
        let abort = handle.abort_handle();

        let state = TaskState {
            name: name.to_string(),
            kind: "timeout".to_string(),
            interval_ms: None,
            delay_ms: Some(delay_ms),
            running: true,
        };
        self.inner.tasks.lock().insert(name.to_string(), RegisteredTask { state, abort });
    }

    /// 取消一个任务
    pub fn clear(&self, name: &str) {
        if let Some(t) = self.inner.tasks.lock().remove(name) {
            t.abort.abort();
        }
    }

    /// 取消所有任务
    pub fn clear_all(&self) {
        let mut tasks = self.inner.tasks.lock();
        for (_, t) in tasks.drain() {
            t.abort.abort();
        }
    }

    /// 取消所有任务 + 触发 cancellation token（用于 shutdown）
    pub fn shutdown(&self) {
        self.clear_all();
        self.inner.cancel.cancel();
    }

    /// 任务数
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.inner.tasks.lock().len()
    }

    /// 快照
    #[must_use]
    pub fn snapshot(&self) -> SchedulerSnapshot {
        let tasks = self.inner.tasks.lock();
        SchedulerSnapshot {
            namespace: self.inner.namespace.clone(),
            created_at_ms: self.inner.created_at_ms,
            task_count: tasks.len(),
            tasks: tasks.values().map(|t| t.state.clone()).collect(),
        }
    }
}

/// 把 `async fn` 转成 `TaskFn` 的便捷宏
#[macro_export]
macro_rules! task_fn {
    ($f:expr) => {
        $crate::runtime::scheduler::TaskFn::new(|| Box::pin($f()))
    };
}

// ===== 全局注册表 =====

/// 全局 Scheduler 注册表（所有 namespace 的 Scheduler 都在这里登记）
pub struct SchedulerRegistry {
    namespaces: Mutex<HashMap<String, SchedulerEntry>>,
    notify: Notify,
}

struct SchedulerEntry {
    created_at_ms: i64,
}

impl SchedulerRegistry {
    fn global() -> Arc<Self> {
        use std::sync::OnceLock;
        static REG: OnceLock<Arc<SchedulerRegistry>> = OnceLock::new();
        REG.get_or_init(|| {
            Arc::new(Self {
                namespaces: Mutex::new(HashMap::new()),
                notify: Notify::new(),
            })
        })
        .clone()
    }

    fn insert(&self, namespace: &str) {
        let mut map = self.namespaces.lock();
        map.insert(
            namespace.to_string(),
            SchedulerEntry {
                created_at_ms: chrono::Utc::now().timestamp_millis(),
            },
        );
        self.notify.notify_waiters();
    }

    /// 列出所有 namespace
    #[must_use]
    pub fn list_namespaces(&self) -> Vec<String> {
        let map = self.namespaces.lock();
        let mut keys: Vec<String> = map.keys().cloned().collect();
        keys.sort();
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn interval_task_runs() {
        let scheduler = Scheduler::new("test");
        let counter = Arc::new(AtomicUsize::new(0));
        let counter2 = counter.clone();
        scheduler.set_interval_task("tick", Duration::from_millis(20), Arc::new(move || {
            let c = counter2.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
            })
        }));
        tokio::time::sleep(Duration::from_millis(110)).await;
        let n = counter.load(Ordering::SeqCst);
        scheduler.shutdown();
        assert!(n >= 3, "expected >=3 ticks, got {n}");
    }

    #[tokio::test]
    async fn timeout_task_fires_once() {
        let scheduler = Scheduler::new("test-2");
        let counter = Arc::new(AtomicUsize::new(0));
        let counter2 = counter.clone();
        scheduler.set_timeout_task("once", Duration::from_millis(20), Arc::new(move || {
            let c = counter2.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
            })
        }));
        tokio::time::sleep(Duration::from_millis(80)).await;
        let n = counter.load(Ordering::SeqCst);
        scheduler.shutdown();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn clear_removes_task() {
        let scheduler = Scheduler::new("test-3");
        scheduler.set_interval_task("a", Duration::from_millis(10), Arc::new(|| {
            Box::pin(async {})
        }));
        assert_eq!(scheduler.task_count(), 1);
        scheduler.clear("a");
        assert_eq!(scheduler.task_count(), 0);
    }

    #[tokio::test]
    async fn snapshot_includes_tasks() {
        let scheduler = Scheduler::new("test-4");
        scheduler.set_interval_task("a", Duration::from_secs(60), Arc::new(|| Box::pin(async {})));
        scheduler.set_timeout_task("b", Duration::from_millis(100), Arc::new(|| Box::pin(async {})));
        let snap = scheduler.snapshot();
        assert_eq!(snap.namespace, "test-4");
        assert_eq!(snap.task_count, 2);
        scheduler.shutdown();
    }
}

#[allow(dead_code)]
fn _ensure_notify_used(_: &Notify) {}
