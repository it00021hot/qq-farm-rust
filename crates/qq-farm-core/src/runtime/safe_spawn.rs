//! 后台任务 panic 隔离：记录后继续，不拖垮进程（依赖 `panic = "unwind"`）。
//!
//! 同一 label 连续 N 次 panic 会触发 [`PanicCircuitBreaker`] 提供的回调，worker
//! 可以借此熔断（如请求重建 TSDK），避免在坏状态 wasm 上无限重试。

use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use tokio::task::JoinHandle;

/// panic 连续 N 次（默认 [`DEFAULT_PANIC_THRESHOLD`]）即触发 `on_threshold` 回调
pub const DEFAULT_PANIC_THRESHOLD: u32 = 5;

/// 同一 label 的 panic 熔断器：连续 ≥ threshold 次 panic 后调用一次 on_threshold，
/// 之后清零（业务侧负责完成"熔断"动作后重置）。
#[derive(Clone)]
pub struct PanicCircuitBreaker {
    inner: Arc<PanicCircuitBreakerInner>,
}

struct PanicCircuitBreakerInner {
    counts: Mutex<HashMap<String, u32>>,
    threshold: u32,
    on_threshold: Box<dyn Fn(&str, u32) + Send + Sync>,
}

impl PanicCircuitBreaker {
    /// 创建熔断器。`on_threshold(label, count)` 在连续 panic 达到阈值时触发一次。
    pub fn new<F>(threshold: u32, on_threshold: F) -> Self
    where
        F: Fn(&str, u32) + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(PanicCircuitBreakerInner {
                counts: Mutex::new(HashMap::new()),
                threshold,
                on_threshold: Box::new(on_threshold),
            }),
        }
    }

    /// 记录一次成功（清零该 label 计数）。
    pub fn record_success(&self, label: &str) {
        if let Ok(mut m) = self.inner.counts.lock() {
            m.remove(label);
        }
    }

    /// 记录一次 panic；若累计达到阈值则触发 `on_threshold` 并清零。
    /// 返回 `true` 表示已触发熔断。
    pub fn record_panic(&self, label: &str) -> bool {
        let n = {
            let Ok(mut m) = self.inner.counts.lock() else { return false };
            let count = m.entry(label.to_string()).or_insert(0);
            *count += 1;
            *count
        };
        if n >= self.inner.threshold {
            (self.inner.on_threshold)(label, n);
            if let Ok(mut m) = self.inner.counts.lock() {
                m.remove(label);
            }
            true
        } else {
            false
        }
    }
}

/// 全局 panic 熔断器（lazy init）。worker 可在 `spawn_logged_with_account` 内
/// 通过 `set_global_panic_breaker` 注入。
static GLOBAL_BREAKER: Mutex<Option<PanicCircuitBreaker>> = Mutex::new(None);

/// 设置全局 panic 熔断器。worker 启动时调用一次。
pub fn set_global_panic_breaker(breaker: PanicCircuitBreaker) {
    if let Ok(mut g) = GLOBAL_BREAKER.lock() {
        *g = Some(breaker);
    }
}

fn global_breaker() -> Option<PanicCircuitBreaker> {
    GLOBAL_BREAKER.lock().ok().and_then(|g| g.clone())
}

/// 将 panic payload 格式化为可读字符串。
#[must_use]
pub fn format_panic_payload(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Box<dyn Any>".to_string()
    }
}

/// `tokio::spawn` 包装：捕获 future 内 panic，写日志后返回 `Ok(())` 语义的完成。
pub fn spawn_logged<F>(label: &'static str, fut: F) -> JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let breaker = global_breaker();
    tokio::spawn(async move {
        match AssertUnwindSafe(fut).catch_unwind().await {
            Ok(()) => {
                if let Some(b) = &breaker {
                    b.record_success(label);
                }
            }
            Err(payload) => {
                let msg = format_panic_payload(payload);
                tracing::error!(label, panic = %msg, "background task panicked");
                crate::utils::logger::record_panic(label, None, &msg);
                if let Some(b) = &breaker {
                    b.record_panic(label);
                }
            }
        }
    })
}

/// 带账号上下文的 spawn：panic 时回调清理（例如释放 worker）。
pub fn spawn_logged_with_account<F, C>(
    label: &'static str,
    account_id: String,
    fut: F,
    on_panic: C,
) -> JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
    C: FnOnce(&str, &str) + Send + 'static,
{
    let breaker = global_breaker();
    tokio::spawn(async move {
        match AssertUnwindSafe(fut).catch_unwind().await {
            Ok(()) => {
                if let Some(b) = &breaker {
                    b.record_success(label);
                }
            }
            Err(payload) => {
                let msg = format_panic_payload(payload);
                tracing::error!(
                    label,
                    account_id = %account_id,
                    panic = %msg,
                    "account task panicked"
                );
                crate::utils::logger::record_panic(label, Some(&account_id), &msg);
                on_panic(&account_id, &msg);
                if let Some(b) = &breaker {
                    b.record_panic(label);
                }
            }
        }
    })
}

/// 全局 panic 总数（用于诊断，与 breaker 无关）
static TOTAL_PANICS: AtomicU32 = AtomicU32::new(0);

/// 当前进程自启动以来的 panic 总数。
#[must_use]
pub fn total_panics() -> u32 {
    TOTAL_PANICS.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn spawn_logged_catches_panic_without_aborting() {
        let flagged = Arc::new(AtomicBool::new(false));
        let f2 = flagged.clone();
        let handle = spawn_logged("test_panic", async move {
            panic!("probe-safe-spawn");
        });
        handle.await.expect("join should succeed after catch_unwind");
        let h2 = spawn_logged("test_ok", async move {
            f2.store(true, Ordering::SeqCst);
        });
        h2.await.expect("follow-up task");
        assert!(flagged.load(Ordering::SeqCst));
    }

    #[test]
    fn circuit_breaker_triggers_at_threshold() {
        let called = Arc::new(AtomicU32::new(0));
        let c2 = called.clone();
        let b = PanicCircuitBreaker::new(3, move |_label, _count| {
            c2.fetch_add(1, Ordering::SeqCst);
        });
        assert!(!b.record_panic("x"));
        assert!(!b.record_panic("x"));
        assert!(b.record_panic("x"));
        assert_eq!(called.load(Ordering::SeqCst), 1);
        // 清零后再次累计
        assert!(!b.record_panic("x"));
        assert!(!b.record_panic("x"));
        assert!(b.record_panic("x"));
        assert_eq!(called.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn circuit_breaker_success_resets() {
        let b = PanicCircuitBreaker::new(2, |_, _| {});
        assert!(!b.record_panic("x"));
        b.record_success("x");
        assert!(!b.record_panic("x"));
        // 第 2 次 panic 达到阈值
        assert!(b.record_panic("x"));
    }
}
