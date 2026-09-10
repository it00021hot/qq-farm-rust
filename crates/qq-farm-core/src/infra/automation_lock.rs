//! 自动化任务全局互斥（对齐 bot `services/automation-lock.ts`）。
//!
//! bot 每个账号一个 worker 进程，用 AsyncLocalStorage + promise 链实现
//! 「已在互斥上下文内直接执行（可嵌套），否则排队串行」的全局队列。
//! rust 桌面端单进程托管多账号，因此按 `account_id` 维度各建一条队列，
//! 语义与 bot 的账号内串行一致：
//!
//! - 已在互斥上下文内（嵌套调用）→ 直接执行，不重新排队；
//! - 否则进入该账号的 FIFO 队列，轮到后才执行；
//! - `is_automation_task_running_for` 查询该账号是否有互斥任务在执行。

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

tokio::task_local! {
    static EXCLUSIVE: bool;
}

/// `account_id` → 该账号的互斥槽（FIFO 公平）
static LOCKS: RwLock<Option<HashMap<String, Arc<Semaphore>>>> = RwLock::new(None);

/// 正在执行互斥任务的账号集合（Vec 便于 const 初始化）
static RUNNING: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn locks() -> parking_lot::RwLockReadGuard<'static, Option<HashMap<String, Arc<Semaphore>>>> {
    LOCKS.read()
}

fn locks_mut() -> parking_lot::RwLockWriteGuard<'static, Option<HashMap<String, Arc<Semaphore>>>> {
    LOCKS.write()
}

fn lock_for(account_id: &str) -> Arc<Semaphore> {
    let read = locks();
    if let Some(slot) = read.as_ref().and_then(|all| all.get(account_id)) {
        return Arc::clone(slot);
    }
    drop(read);
    let mut guard = locks_mut();
    let all = guard.get_or_insert_with(HashMap::new);
    Arc::clone(
        all.entry(account_id.to_string()).or_insert_with(|| Arc::new(Semaphore::const_new(1))),
    )
}

/// 指定账号是否有互斥自动化任务正在执行（对齐 bot `isAutomationTaskRunning`）
#[must_use]
pub fn is_automation_task_running_for(account_id: &str) -> bool {
    RUNNING.lock().iter().any(|id| id == account_id)
}

/// 在该账号的互斥上下文里执行自动化任务（对齐 bot `runExclusiveAutomationTask`）。
///
/// - 已在互斥上下文内（嵌套调用）→ 直接执行，不重新排队；
/// - 否则排队等待，直到轮到本任务再执行。
pub async fn run_exclusive_automation_task<F, T>(account_id: &str, _name: &str, task: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let already_exclusive = EXCLUSIVE.try_with(|_| true).unwrap_or(false);
    if already_exclusive {
        return task.await;
    }

    let slot = lock_for(account_id);
    let permit: OwnedSemaphorePermit =
        Arc::clone(&slot).acquire_owned().await.expect("automation lock semaphore closed");
    RUNNING.lock().push(account_id.to_string());
    let result = EXCLUSIVE.scope(true, task).await;
    RUNNING.lock().retain(|id| id != account_id);
    drop(permit);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ACC: &str = "test-automation-lock-account";

    #[tokio::test]
    async fn nested_call_runs_inline() {
        // 已在互斥上下文内的嵌套调用直接执行，不排队
        let result = run_exclusive_automation_task(ACC, "outer", async {
            run_exclusive_automation_task(ACC, "inner", async { 42_i32 }).await
        })
        .await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn serializes_tasks_per_account() {
        let order = Arc::new(Mutex::new(Vec::new()));

        let order_a = Arc::clone(&order);
        let first = run_exclusive_automation_task(ACC, "a", async move {
            order_a.lock().push("a-start");
            assert!(is_automation_task_running_for(ACC));
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            order_a.lock().push("a-end");
        });

        let order_b = Arc::clone(&order);
        let second = run_exclusive_automation_task(ACC, "b", async move {
            order_b.lock().push("b-start");
            order_b.lock().push("b-end");
        });

        tokio::join!(first, second);
        assert!(!is_automation_task_running_for(ACC));
        assert_eq!(*order.lock(), vec!["a-start", "a-end", "b-start", "b-end"]);
    }

    #[tokio::test]
    async fn different_accounts_run_in_parallel() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c1 = Arc::clone(&counter);
        let c2 = Arc::clone(&counter);
        let (r1, r2) = tokio::join!(
            run_exclusive_automation_task("acc-x", "x", async move {
                c1.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }),
            run_exclusive_automation_task("acc-y", "y", async move {
                c2.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }),
        );
        let _ = (r1, r2);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
