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

/// 当前持有者：account_id -> (任务名, 获得时刻 ms)。用于卡死诊断与告警点名。
struct HolderInfo {
    name: String,
    since_ms: i64,
}

static HOLDERS: Mutex<Option<HashMap<String, HolderInfo>>> = Mutex::new(None);

/// 等待互斥槽超过该时长即告警（正常自动化任务最长几十秒）
const WAIT_WARN_MS: u64 = 120_000;

/// 单个互斥任务的执行上限：超时即取消 future（连带释放 permit 与其持有的
/// 所有 tokio 锁守卫），排队任务立即恢复。巡查间隔仅 20~25s、任务秒级完成；
/// 2026-09-11 小号事故：挂死任务永久持有 permit，账号"在线但自动化零动作"
/// 持续数小时，重连也无法恢复（挂死的是独立 tick task，不被 worker 停止中止）。
pub const MAX_EXEC_MS: u64 = 10 * 60 * 1000;

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

/// 查询某账号当前互斥持有者（诊断用）
#[must_use]
pub fn current_holder(account_id: &str) -> Option<(String, i64)> {
    HOLDERS.lock().as_ref().and_then(|m| m.get(account_id)).map(|h| (h.name.clone(), h.since_ms))
}

/// 在该账号的互斥上下文里执行自动化任务（对齐 bot `runExclusiveAutomationTask`）。
///
/// - 已在互斥上下文内（嵌套调用）→ 直接执行，不重新排队；
/// - 否则排队等待，直到轮到本任务再执行。
/// - 等待超过 2 分钟点名告警持有者；执行超过 [`MAX_EXEC_MS`] 取消任务自愈
///   （释放 permit 与任务持有的锁守卫），账号自动化从此不可能被单个挂死
///   任务永久瘫痪。
///
/// 所有调用方均不消费返回值，故统一不返回。
pub async fn run_exclusive_automation_task<F, T>(account_id: &str, name: &str, task: F)
where
    F: std::future::Future<Output = T>,
{
    run_exclusive_with_limit(account_id, name, task, MAX_EXEC_MS).await;
}

/// 带执行上限的互斥执行（测试用短时限走这里）。
async fn run_exclusive_with_limit<F, T>(account_id: &str, name: &str, task: F, max_exec_ms: u64)
where
    F: std::future::Future<Output = T>,
{
    let already_exclusive = EXCLUSIVE.try_with(|_| true).unwrap_or(false);
    if already_exclusive {
        task.await;
        return;
    }

    let slot = lock_for(account_id);
    let mut warned = false;
    // acquire future 只创建一次：select 每轮复用，保住 FIFO 排队位置不被插队
    let mut acquire = std::pin::pin!(Arc::clone(&slot).acquire_owned());
    let permit: OwnedSemaphorePermit = loop {
        tokio::select! {
            permit = &mut acquire => {
                break permit.expect("automation lock semaphore closed");
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(WAIT_WARN_MS)) => {
                if !warned {
                    warned = true;
                    let holder = current_holder(account_id);
                    match holder {
                        Some((holder_name, since)) => {
                            let held_s = (crate::utils::time::now_ms() - since) / 1000;
                            tracing::error!(
                                account_id,
                                waiting = name,
                                holder = %holder_name,
                                holder_held_s = held_s,
                                "自动化互斥锁等待超时：持有者疑似卡死"
                            );
                        }
                        None => {
                            tracing::error!(
                                account_id,
                                waiting = name,
                                "自动化互斥锁等待超时（无持有者记录）"
                            );
                        }
                    }
                }
                // 继续等待（select 循环），不放弃任务
            }
        }
    };
    {
        let mut guard = HOLDERS.lock();
        guard.get_or_insert_with(HashMap::new).insert(
            account_id.to_string(),
            HolderInfo { name: name.to_string(), since_ms: crate::utils::time::now_ms() },
        );
    }
    RUNNING.lock().push(account_id.to_string());
    let started = crate::utils::time::now_ms();
    // 执行上限兜底：超时取消 future（drop 守卫、释放 permit），
    // 把"账号级自动化永久瘫痪"降级为"单轮任务作废，下轮 tick 重来"
    let outcome = tokio::time::timeout(
        std::time::Duration::from_millis(max_exec_ms),
        EXCLUSIVE.scope(true, task),
    )
    .await;
    RUNNING.lock().retain(|id| id != account_id);
    HOLDERS.lock().as_mut().map(|m| m.remove(account_id));
    drop(permit);
    match outcome {
        Ok(_) => {}
        Err(_) => {
            let held_s = (crate::utils::time::now_ms() - started) / 1000;
            tracing::error!(
                account_id,
                task = name,
                held_s,
                "自动化任务执行超时，已取消并释放互斥锁（排队任务将自动恢复）"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ACC: &str = "test-automation-lock-account";

    #[tokio::test]
    async fn nested_call_runs_inline() {
        // 已在互斥上下文内的嵌套调用直接执行，不排队
        let hit = Arc::new(AtomicUsize::new(0));
        let h = Arc::clone(&hit);
        run_exclusive_automation_task(ACC, "outer", async move {
            run_exclusive_automation_task(ACC, "inner", async move {
                h.fetch_add(1, Ordering::SeqCst);
            })
            .await;
        })
        .await;
        assert_eq!(hit.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn hung_task_times_out_and_releases_lock() {
        // 挂死任务（永不完成的 await）到达执行上限后被取消，锁释放，
        // 排队的下一个任务得以执行——2026-09-11 小号"在线但零动作"事故的自愈路径
        let entered = Arc::new(AtomicUsize::new(0));
        let e1 = Arc::clone(&entered);
        let hung = run_exclusive_with_limit(
            ACC,
            "hung",
            async move {
                e1.fetch_add(1, Ordering::SeqCst);
                std::future::pending::<()>().await;
            },
            80,
        );
        let next_ran = Arc::new(AtomicUsize::new(0));
        let n1 = Arc::clone(&next_ran);
        let next = run_exclusive_with_limit(
            ACC,
            "next",
            async move {
                n1.fetch_add(1, Ordering::SeqCst);
            },
            80,
        );
        // 挂死任务占住锁；等过执行上限后应被取消并放行 next
        let done = tokio::time::timeout(std::time::Duration::from_millis(2_000), async {
            let _ = hung.await;
            next.await;
        })
        .await;
        assert!(done.is_ok(), "hung task should be cancelled and next should run");
        assert_eq!(entered.load(Ordering::SeqCst), 1);
        assert_eq!(next_ran.load(Ordering::SeqCst), 1);
        assert!(!is_automation_task_running_for(ACC));
        assert!(current_holder(ACC).is_none());
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
