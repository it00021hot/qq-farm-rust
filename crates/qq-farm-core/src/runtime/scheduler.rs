//! 任务调度器。
//!
//! 与原 `core/src/services/scheduler.ts` 对齐：
//! - `set_interval_task(name, interval, fn)`：按间隔循环（默认 preventOverlap=true，不阻塞 ticker）
//! - `set_timeout_task(name, delay, fn)`：延迟一次性执行
//! - `clear(name)` / `clear_all()` / `shutdown()`
//!
//! ## 命名空间
//!
//! 每个 Scheduler 实例有独立 namespace（多个账号 worker 各持一个）。
//! `Scheduler::registry()` 提供全局注册表快照（用于 UI 展示）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// interval 选项（对齐 TS `setIntervalTask(..., options)`）
#[derive(Debug, Clone, Copy)]
pub struct IntervalOptions {
    /// 默认 true：上一次未完成则跳过本拍（对齐 bot）
    pub prevent_overlap: bool,
    /// 默认 false：首拍在 interval 之后（对齐 bot；true 则立刻跑一次）
    pub run_immediately: bool,
}

impl Default for IntervalOptions {
    fn default() -> Self {
        Self { prevent_overlap: true, run_immediately: false }
    }
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

    /// 注册 interval（默认 preventOverlap=true，不阻塞 ticker）
    pub fn set_interval_task(&self, name: &str, interval: Duration, task: TaskFn) {
        self.set_interval_task_with_options(name, interval, task, IntervalOptions::default());
    }

    /// 带选项的 interval（对齐 TS options）
    pub fn set_interval_task_with_options(
        &self,
        name: &str,
        interval: Duration,
        task: TaskFn,
        options: IntervalOptions,
    ) {
        self.clear(name);
        let cancel = self.inner.cancel.clone();
        let interval_ms = interval.as_millis() as u64;
        let interval_tokio = interval;
        let task_name = name.to_string();
        let task_for_run = task.clone();
        let prevent_overlap = options.prevent_overlap;
        let run_immediately = options.run_immediately;

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval_tokio);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // bot：默认首拍在 delay 之后；runImmediately 才立刻跑
            if !run_immediately {
                ticker.tick().await;
            }
            let running = Arc::new(AtomicBool::new(false));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {
                        // 对齐 bot：在启动回调前同步置 running，避免叠跑
                        if prevent_overlap && running.swap(true, Ordering::AcqRel) {
                            continue;
                        }
                        if !prevent_overlap {
                            running.store(true, Ordering::Release);
                        }
                        let task = task_for_run.clone();
                        let running = running.clone();
                        tokio::spawn(async move {
                            task().await;
                            running.store(false, Ordering::Release);
                        });
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
                    // 到期后另起 task 跑回调，clear() 只取消尚未开火的 timer。
                    tokio::spawn(async move {
                        task().await;
                    });
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
        std::sync::Arc::new(|| Box::pin($f()) as futures::future::BoxFuture<'static, ()>)
            as $crate::runtime::scheduler::TaskFn
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
            Arc::new(Self { namespaces: Mutex::new(HashMap::new()), notify: Notify::new() })
        })
        .clone()
    }

    fn insert(&self, namespace: &str) {
        let mut map = self.namespaces.lock();
        map.insert(
            namespace.to_string(),
            SchedulerEntry { created_at_ms: chrono::Utc::now().timestamp_millis() },
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
        scheduler.set_interval_task(
            "tick",
            Duration::from_millis(20),
            Arc::new(move || {
                let c = counter2.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                })
            }),
        );
        tokio::time::sleep(Duration::from_millis(110)).await;
        let n = counter.load(Ordering::SeqCst);
        scheduler.shutdown();
        assert!(n >= 3, "expected >=3 ticks, got {n}");
    }

    #[tokio::test]
    async fn interval_prevent_overlap_skips_while_running() {
        let scheduler = Scheduler::new("overlap");
        let entered = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicUsize::new(0));
        let e2 = entered.clone();
        let f2 = finished.clone();
        scheduler.set_interval_task_with_options(
            "slow",
            Duration::from_millis(20),
            Arc::new(move || {
                let e = e2.clone();
                let f = f2.clone();
                Box::pin(async move {
                    e.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(80)).await;
                    f.fetch_add(1, Ordering::SeqCst);
                })
            }),
            IntervalOptions { prevent_overlap: true, run_immediately: false },
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        let e = entered.load(Ordering::SeqCst);
        let f = finished.load(Ordering::SeqCst);
        scheduler.shutdown();
        // 允许末尾一拍仍在跑：finished 可比 entered 少 1
        assert!(f >= 2, "expected several completed runs, finished={f}");
        assert!(
            e <= f + 1 && e <= 5,
            "preventOverlap should keep run count low, entered={e} finished={f}"
        );
    }

    #[tokio::test]
    async fn timeout_task_fires_once() {
        let scheduler = Scheduler::new("test-timeout");
        let counter = Arc::new(AtomicUsize::new(0));
        let c2 = counter.clone();
        scheduler.set_timeout_task(
            "once",
            Duration::from_millis(30),
            Arc::new(move || {
                let c = c2.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                })
            }),
        );
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        scheduler.shutdown();
    }

    #[tokio::test]
    async fn clear_timeout_does_not_abort_running_callback() {
        let scheduler = Scheduler::new("test-clear-running");
        let started = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicUsize::new(0));
        let s2 = started.clone();
        let f2 = finished.clone();
        scheduler.set_timeout_task(
            "slow",
            Duration::from_millis(20),
            Arc::new(move || {
                let s = s2.clone();
                let f = f2.clone();
                Box::pin(async move {
                    s.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(80)).await;
                    f.fetch_add(1, Ordering::SeqCst);
                })
            }),
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(started.load(Ordering::SeqCst), 1);
        scheduler.clear("slow");
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(finished.load(Ordering::SeqCst), 1);
        scheduler.shutdown();
    }

    #[tokio::test]
    async fn clear_stops_interval() {
        let scheduler = Scheduler::new("test-clear");
        let counter = Arc::new(AtomicUsize::new(0));
        let c2 = counter.clone();
        scheduler.set_interval_task(
            "a",
            Duration::from_millis(10),
            Arc::new(move || {
                let c = c2.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                })
            }),
        );
        tokio::time::sleep(Duration::from_millis(35)).await;
        scheduler.clear("a");
        let n = counter.load(Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(counter.load(Ordering::SeqCst), n);
        scheduler.shutdown();
    }
}
