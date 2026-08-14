//! 请求队列与并发控制（1:1 翻译原 `core/src/services/rate-limiter.ts`）。
//!
//! 用于批量操作（帮 / 偷 / 巡访 / 商店 / 任务）的限流与并发控制，
//! 防止触发服务端限流。
//!
//! ## 核心组件
//!
//! - [`TokenBucket`]：令牌桶限流
//! - [`PriorityQueue`]：优先级队列
//! - [`RequestQueue`]：基于 token bucket + priority queue 的请求队列
//!
//! ## 服务配置
//!
//! - `PlantService`: max_concurrent=2, min_interval=200ms
//! - `FriendService`: max_concurrent=1, min_interval=500ms
//! - `VisitService`: max_concurrent=1, min_interval=500ms
//! - `TaskService`: max_concurrent=3, min_interval=100ms
//! - `MallService`: max_concurrent=2, min_interval=200ms
//! - default: max_concurrent=3, min_interval=100ms

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::sleep;

/// 默认配置
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    pub max_concurrent: usize,
    pub min_interval_ms: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub enable_burst: bool,
    pub burst_size: usize,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 3,
            min_interval_ms: 100,
            max_retries: 2,
            retry_delay_ms: 500,
            enable_burst: false,
            burst_size: 5,
        }
    }
}

// =====================================================================
// TokenBucket
// =====================================================================

/// 令牌桶
#[derive(Debug)]
pub struct TokenBucket {
    capacity: f64,
    tokens: f64,
    refill_rate_ms: f64,
    last_refill_ms: i64,
    max_wait_ms: i64,
}

impl TokenBucket {
    #[must_use]
    pub fn new(capacity: usize, refill_rate_ms: u64, max_wait_ms: i64) -> Self {
        Self {
            capacity: capacity as f64,
            tokens: capacity as f64,
            refill_rate_ms: refill_rate_ms.max(1) as f64,
            last_refill_ms: crate::utils::time::now_ms(),
            max_wait_ms,
        }
    }

    fn refill(&mut self) {
        let now = crate::utils::time::now_ms();
        let elapsed = (now - self.last_refill_ms).max(0) as f64;
        let tokens_to_add = (elapsed / self.refill_rate_ms) * self.capacity;
        self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
        self.last_refill_ms = now;
    }

    /// 阻塞等待获取 token
    pub async fn acquire(&mut self, tokens: f64) -> Result<(), &'static str> {
        let start = crate::utils::time::now_ms();
        while self.tokens < tokens {
            if (crate::utils::time::now_ms() - start) > self.max_wait_ms {
                return Err("请求等待超时");
            }
            self.refill();
            sleep(Duration::from_millis(50)).await;
        }
        self.tokens -= tokens;
        Ok(())
    }

    /// 释放 token
    pub fn release(&mut self, tokens: f64) {
        self.tokens = (self.tokens + tokens).min(self.capacity);
    }

    #[must_use]
    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    #[must_use]
    pub fn tokens(&self) -> f64 {
        self.tokens
    }
}

// =====================================================================
// PriorityQueue
// =====================================================================

type TaskFn =
    Box<dyn FnMut() -> futures::future::BoxFuture<'static, anyhow::Result<serde_json::Value>> + Send>;

/// 任务条目
pub struct TaskEntry {
    pub fn_run: TaskFn,
    pub resolve: tokio::sync::oneshot::Sender<serde_json::Value>,
    pub retries: u32,
    pub attempts: u32,
    pub label: String,
    pub priority: i32,
}

impl std::fmt::Debug for TaskEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskEntry")
            .field("label", &self.label)
            .field("retries", &self.retries)
            .field("attempts", &self.attempts)
            .field("priority", &self.priority)
            .finish()
    }
}

impl TaskEntry {
    pub fn new(
        f: impl FnMut() -> futures::future::BoxFuture<'static, anyhow::Result<serde_json::Value>> + Send + 'static,
        resolve: tokio::sync::oneshot::Sender<serde_json::Value>,
        label: String,
        priority: i32,
        retries: u32,
    ) -> Self {
        Self {
            fn_run: Box::new(f),
            resolve,
            retries,
            attempts: 0,
            label,
            priority,
        }
    }
}

struct QueueEntry {
    task: TaskEntry,
    priority: i32,
    added_at: i64,
}

/// 优先级队列（数字越大越优先）
pub struct PriorityQueue {
    queue: Vec<QueueEntry>,
}

impl Default for PriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PriorityQueue {
    #[must_use]
    pub fn new() -> Self {
        Self { queue: Vec::new() }
    }

    pub fn enqueue(&mut self, task: TaskEntry, priority: i32) {
        let entry = QueueEntry {
            task,
            priority,
            added_at: crate::utils::time::now_ms(),
        };
        let idx = self
            .queue
            .iter()
            .position(|e| e.priority < priority)
            .unwrap_or(self.queue.len());
        self.queue.insert(idx, entry);
    }

    pub fn dequeue(&mut self) -> Option<TaskEntry> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0).task)
        }
    }

    #[must_use]
    pub fn peek(&self) -> Option<&TaskEntry> {
        self.queue.first().map(|e| &e.task)
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.queue.len()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

// =====================================================================
// RequestQueue
// =====================================================================

/// 队列状态
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueStatus {
    pub queue_size: usize,
    pub available_tokens: usize,
    pub capacity: usize,
}

/// 请求队列
pub struct RequestQueue {
    bucket: Mutex<TokenBucket>,
    queue: Mutex<PriorityQueue>,
    processing: Mutex<bool>,
    config: RateLimiterConfig,
}

impl RequestQueue {
    #[must_use]
    pub fn new(config: RateLimiterConfig) -> Self {
        let bucket = Mutex::new(TokenBucket::new(
            config.max_concurrent,
            config.min_interval_ms,
            5000,
        ));
        Self {
            bucket,
            queue: Mutex::new(PriorityQueue::new()),
            processing: Mutex::new(false),
            config,
        }
    }

    /// 添加任务
    pub async fn add_request<F>(&self, f: F, label: &str, priority: i32) -> serde_json::Value
    where
        F: FnMut() -> futures::future::BoxFuture<'static, anyhow::Result<serde_json::Value>> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = TaskEntry::new(f, tx, label.to_string(), priority, self.config.max_retries);
        {
            let mut q = self.queue.lock().await;
            q.enqueue(task, priority);
        }
        self.process_queue().await;
        rx.await.unwrap_or(serde_json::Value::Null)
    }

    /// 处理队列
    async fn process_queue(&self) {
        let mut processing = self.processing.lock().await;
        if *processing {
            return;
        }
        *processing = true;
        drop(processing);

        loop {
            // 1. 取出任务
            let task = {
                let mut q = self.queue.lock().await;
                q.dequeue()
            };
            let Some(mut task) = task else {
                break;
            };

            // 2. 获取 token
            {
                let mut bucket = self.bucket.lock().await;
                if let Err(e) = bucket.acquire(1.0).await {
                    let _ = task.resolve.send(serde_json::json!({ "error": e }));
                    continue;
                }
            }

            // 3. 执行 fn_run（FnMut，可重试）
            let result = (task.fn_run)().await;
            {
                let mut bucket = self.bucket.lock().await;
                bucket.release(1.0);
            }

            // 4. 处理结果 / 重试
            match result {
                Ok(v) => {
                    let _ = task.resolve.send(v);
                }
                Err(e) => {
                    if task.attempts < task.retries {
                        task.attempts += 1;
                        tracing::info!(
                            "[{}] 请求失败，第 {} 次重试中... error={}",
                            task.label,
                            task.attempts,
                            e
                        );
                        let delay = self.config.retry_delay_ms * task.attempts as u64;
                        sleep(Duration::from_millis(delay)).await;
                        // 重试：把 task 重新入队（FnMut 可再次执行真正的任务）
                        let mut q = self.queue.lock().await;
                        let priority = task.priority;
                        q.enqueue(task, priority);
                    } else {
                        let _ = task.resolve.send(serde_json::json!({
                            "error": e.to_string()
                        }));
                    }
                }
            }
        }

        *self.processing.lock().await = false;
    }

    /// 设置并发度
    pub async fn set_concurrency(&self, concurrency: usize) {
        let mut bucket = self.bucket.lock().await;
        let new_cap = concurrency.clamp(1, 20) as f64;
        bucket.capacity = new_cap;
        bucket.tokens = bucket.tokens.min(new_cap);
    }

    /// 获取状态
    pub async fn get_status(&self) -> QueueStatus {
        let queue_size = self.queue.lock().await.size();
        let bucket = self.bucket.lock().await;
        QueueStatus {
            queue_size,
            available_tokens: bucket.tokens as usize,
            capacity: bucket.capacity as usize,
        }
    }

    /// 清空队列（不中断正在执行的任务）
    pub async fn clear(&self) {
        self.queue.lock().await.clear();
    }
}

// =====================================================================
// 服务队列单例
// =====================================================================

/// 服务配置
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub max_concurrent: usize,
    pub min_interval_ms: u64,
}

#[must_use]
pub fn get_service_config(service_name: &str) -> ServiceConfig {
    match service_name {
        "PlantService" => ServiceConfig { max_concurrent: 2, min_interval_ms: 200 },
        "FriendService" => ServiceConfig { max_concurrent: 1, min_interval_ms: 500 },
        "VisitService" => ServiceConfig { max_concurrent: 1, min_interval_ms: 500 },
        "TaskService" => ServiceConfig { max_concurrent: 3, min_interval_ms: 100 },
        "MallService" => ServiceConfig { max_concurrent: 2, min_interval_ms: 200 },
        _ => ServiceConfig { max_concurrent: 3, min_interval_ms: 100 },
    }
}

static SERVICE_QUEUES: once_cell::sync::Lazy<Mutex<HashMap<String, Arc<RequestQueue>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

/// 获取某服务的全局队列（首次调用时创建）
pub async fn get_service_queue(service_name: &str) -> Arc<RequestQueue> {
    let mut map = SERVICE_QUEUES.lock().await;
    if let Some(q) = map.get(service_name) {
        return Arc::clone(q);
    }
    let cfg = get_service_config(service_name);
    let q = Arc::new(RequestQueue::new(RateLimiterConfig {
        max_concurrent: cfg.max_concurrent,
        min_interval_ms: cfg.min_interval_ms,
        ..Default::default()
    }));
    map.insert(service_name.to_string(), Arc::clone(&q));
    q
}

/// 全局 farm 优化器
pub async fn get_farm_optimizer() -> Arc<RequestQueue> {
    get_service_queue("PlantService").await
}

/// 全局 friend 优化器
pub async fn get_friend_optimizer() -> Arc<RequestQueue> {
    get_service_queue("FriendService").await
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_bucket_initial_full() {
        let b = TokenBucket::new(3, 100, 5000);
        assert!((b.tokens() - 3.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn token_bucket_acquire_release() {
        let mut b = TokenBucket::new(3, 100, 5000);
        b.acquire(1.0).await.unwrap();
        assert!((b.tokens() - 2.0).abs() < 0.01);
        b.release(1.0);
        assert!((b.tokens() - 3.0).abs() < 0.01);
    }

    #[test]
    fn priority_queue_order() {
        let mut q = PriorityQueue::new();
        let (tx1, _) = tokio::sync::oneshot::channel();
        let (tx2, _) = tokio::sync::oneshot::channel();
        let (tx3, _) = tokio::sync::oneshot::channel();
        let t1 = TaskEntry::new(
            || Box::pin(async { Ok(serde_json::json!(1)) }),
            tx1,
            "t1".to_string(),
            0,
            0,
        );
        let t2 = TaskEntry::new(
            || Box::pin(async { Ok(serde_json::json!(2)) }),
            tx2,
            "t2".to_string(),
            5,
            0,
        );
        let t3 = TaskEntry::new(
            || Box::pin(async { Ok(serde_json::json!(3)) }),
            tx3,
            "t3".to_string(),
            2,
            0,
        );
        q.enqueue(t1, 0);
        q.enqueue(t2, 5);
        q.enqueue(t3, 2);
        // 优先级：t2(5) > t3(2) > t1(0)
        assert_eq!(q.dequeue().unwrap().label, "t2");
        assert_eq!(q.dequeue().unwrap().label, "t3");
        assert_eq!(q.dequeue().unwrap().label, "t1");
    }

    #[test]
    fn priority_queue_empty() {
        let mut q = PriorityQueue::new();
        assert!(q.dequeue().is_none());
        assert!(q.peek().is_none());
        assert_eq!(q.size(), 0);
    }

    #[test]
    fn priority_queue_clear() {
        let mut q = PriorityQueue::new();
        let (tx, _) = tokio::sync::oneshot::channel();
        q.enqueue(
            TaskEntry::new(|| Box::pin(async { Ok(serde_json::json!(1)) }), tx, "t".to_string(), 0, 0),
            0,
        );
        assert_eq!(q.size(), 1);
        q.clear();
        assert_eq!(q.size(), 0);
    }

    #[test]
    fn service_config_known_services() {
        assert_eq!(get_service_config("PlantService").max_concurrent, 2);
        assert_eq!(get_service_config("FriendService").max_concurrent, 1);
        assert_eq!(get_service_config("VisitService").max_concurrent, 1);
        assert_eq!(get_service_config("TaskService").max_concurrent, 3);
        assert_eq!(get_service_config("MallService").max_concurrent, 2);
    }

    #[test]
    fn service_config_default() {
        let c = get_service_config("Unknown");
        assert_eq!(c.max_concurrent, 3);
        assert_eq!(c.min_interval_ms, 100);
    }

    #[tokio::test]
    async fn service_queue_singleton() {
        let q1 = get_service_queue("PlantService").await;
        let q2 = get_service_queue("PlantService").await;
        assert!(Arc::ptr_eq(&q1, &q2));
    }

    #[tokio::test]
    async fn request_queue_executes_simple() {
        let q = RequestQueue::new(RateLimiterConfig {
            max_concurrent: 2,
            min_interval_ms: 50,
            max_retries: 0,
            ..Default::default()
        });
        let v = q
            .add_request(
                || Box::pin(async { Ok(serde_json::json!("hello")) }),
                "test",
                0,
            )
            .await;
        assert_eq!(v, serde_json::json!("hello"));
    }

    #[tokio::test]
    async fn request_queue_preserves_priority() {
        let q = RequestQueue::new(RateLimiterConfig {
            max_concurrent: 1,
            min_interval_ms: 10,
            max_retries: 0,
            ..Default::default()
        });
        // 单并发，先到先得；这里测试 priority 排序在 dequeue 层
        let mut pq = PriorityQueue::new();
        let (tx1, _) = tokio::sync::oneshot::channel();
        let (tx2, _) = tokio::sync::oneshot::channel();
        pq.enqueue(
            TaskEntry::new(|| Box::pin(async { Ok(serde_json::json!(1)) }), tx1, "low".to_string(), 0, 0),
            0,
        );
        pq.enqueue(
            TaskEntry::new(|| Box::pin(async { Ok(serde_json::json!(2)) }), tx2, "high".to_string(), 5, 0),
            5,
        );
        assert_eq!(pq.dequeue().unwrap().label, "high");
        assert_eq!(pq.dequeue().unwrap().label, "low");
    }

    #[tokio::test]
    async fn request_queue_retries_then_returns_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        // FnMut 现在可重试：max_retries=2 时 fn_run 会被调用 3 次（1 次 + 2 次重试）
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();
        let q = RequestQueue::new(RateLimiterConfig {
            max_concurrent: 1,
            min_interval_ms: 10,
            max_retries: 2,
            retry_delay_ms: 1,
            ..Default::default()
        });
        let v = q
            .add_request(
                move || {
                    let calls = calls_clone.clone();
                    Box::pin(async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Err::<serde_json::Value, _>(anyhow::anyhow!("boom"))
                    })
                },
                "error-test",
                0,
            )
            .await;
        // 重试耗尽后返回 { error }
        let obj = v.as_object().expect("object");
        assert!(obj.get("error").is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn request_queue_status_reflects() {
        let q = RequestQueue::new(RateLimiterConfig {
            max_concurrent: 3,
            min_interval_ms: 50,
            max_retries: 0,
            ..Default::default()
        });
        let s = q.get_status().await;
        assert_eq!(s.queue_size, 0);
        assert_eq!(s.capacity, 3);
    }
}
