//! 后台任务 panic 隔离：记录后继续，不拖垮进程（依赖 `panic = "unwind"`）。

use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;

use futures::FutureExt;
use tokio::task::JoinHandle;

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
    tokio::spawn(async move {
        match AssertUnwindSafe(fut).catch_unwind().await {
            Ok(()) => {}
            Err(payload) => {
                let msg = format_panic_payload(payload);
                tracing::error!(label, panic = %msg, "background task panicked");
                crate::utils::logger::record_panic(label, None, &msg);
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
    tokio::spawn(async move {
        match AssertUnwindSafe(fut).catch_unwind().await {
            Ok(()) => {}
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
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
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
}
