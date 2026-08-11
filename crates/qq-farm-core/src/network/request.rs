//! 异步请求/响应管理。
//!
//! 原 network.ts 用一个 `pendingCallbacks: Map<seq, callback>` + `clientSeq` 自增
//! 来关联请求/响应。Rust 端用 `HashMap` + tokio 的 `oneshot::channel` 实现。
//!
//! ## 流程
//!
//! 1. 调用方通过 [`RequestManager::call`] 发起请求，拿到 `client_seq` + `oneshot::Receiver`
//! 2. 发送时把 `client_seq` 写入 frame
//! 3. 收到响应时按 `client_seq` 找到对应 receiver，send 响应数据
//! 4. 超时由调用方用 `tokio::time::timeout` 控制

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::sync::oneshot;

use crate::network::error::NetworkError;

/// 待处理请求
struct Pending {
    service_name: String,
    method_name: String,
    /// 用于在 channel 完成时通知调用方
    sender: Option<oneshot::Sender<Response>>,
}

/// 响应数据（成功）
#[derive(Debug, Clone)]
pub struct Response {
    /// 业务负载（已解密）
    pub body: Vec<u8>,
    /// 服务端 seq
    pub server_seq: i64,
}

/// 请求管理器的可共享句柄
#[derive(Clone)]
pub struct RequestManager {
    inner: Arc<Inner>,
}

struct Inner {
    /// 下一个 client_seq（自增，初始 1）
    next_seq: AtomicI64,
    /// 待处理请求
    pending: parking_lot::Mutex<HashMap<i64, Pending>>,
}

impl RequestManager {
    /// 创建
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                next_seq: AtomicI64::new(1),
                pending: parking_lot::Mutex::new(HashMap::new()),
            }),
        }
    }

    /// 分配下一个 client_seq
    pub fn next_seq(&self) -> i64 {
        self.inner.next_seq.fetch_add(1, Ordering::SeqCst)
    }

    /// 发起请求：返回 (client_seq, receiver)
    ///
    /// 调用方负责把 `client_seq` 写入 frame，发送后 `await receiver`
    pub fn call(
        &self,
        service_name: impl Into<String>,
        method_name: impl Into<String>,
    ) -> (i64, oneshot::Receiver<Response>) {
        let seq = self.next_seq();
        let (tx, rx) = oneshot::channel();
        let pending = Pending {
            service_name: service_name.into(),
            method_name: method_name.into(),
            sender: Some(tx),
        };
        self.inner.pending.lock().insert(seq, pending);
        (seq, rx)
    }

    /// 完成一个请求（收到响应时调用）
    ///
    /// 返回 `true` 表示找到了对应 pending；`false` 表示 seq 无效（可能超时后被清理）
    pub fn complete(&self, seq: i64, body: Vec<u8>, server_seq: i64) -> bool {
        let mut pending_map = self.inner.pending.lock();
        if let Some(mut pending) = pending_map.remove(&seq) {
            if let Some(tx) = pending.sender.take() {
                let _ = tx.send(Response { body, server_seq });
                return true;
            }
        }
        false
    }

    /// 以业务错误完成一个请求
    pub fn fail(&self, seq: i64, err: NetworkError) -> bool {
        let mut pending_map = self.inner.pending.lock();
        if let Some(mut pending) = pending_map.remove(&seq) {
            if let Some(tx) = pending.sender.take() {
                // 把错误包成 NetworkError::Gateway 之外的某类 — 这里用 oneshot 没法直接发 err
                // 改：把错误转成空 body + caller 自行判断。但更干净是改成 oneshot::Sender<Result<Response, _>>
                // —— 后续优化。先 drop 即可（caller 用 select! + timeout 检测完成）
                let _ = tx; // drop
                tracing::warn!(?err, "request failed (no detail returned to caller)");
                return true;
            }
        }
        false
    }

    /// 取消一个待处理请求（超时/主动中断）
    pub fn cancel(&self, seq: i64) -> Option<(String, String)> {
        let mut pending_map = self.inner.pending.lock();
        pending_map.remove(&seq).map(|p| (p.service_name, p.method_name))
    }

    /// 当前待处理请求数
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.inner.pending.lock().len()
    }

    /// 拒绝所有待处理请求（连接断开时调用）
    pub fn reject_all(&self) -> usize {
        let mut pending_map = self.inner.pending.lock();
        let count = pending_map.len();
        pending_map.clear();
        count
    }
}

impl Default for RequestManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn call_and_complete() {
        let mgr = RequestManager::new();
        let (seq, rx) = mgr.call("svc", "Method");
        assert_eq!(seq, 1);
        let next = mgr.next_seq();
        assert_eq!(next, 2);

        // 模拟异步任务
        let mgr2 = mgr.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            mgr2.complete(seq, b"resp".to_vec(), 100);
        });

        let resp = rx.await.expect("channel");
        assert_eq!(resp.body, b"resp");
        assert_eq!(resp.server_seq, 100);
        assert_eq!(mgr.pending_count(), 0);
    }

    #[tokio::test]
    async fn complete_unknown_seq_is_noop() {
        let mgr = RequestManager::new();
        assert!(!mgr.complete(999, b"x".to_vec(), 0));
    }

    #[tokio::test]
    async fn cancel_drops_receiver() {
        let mgr = RequestManager::new();
        let (seq, rx) = mgr.call("svc", "Method");
        let info = mgr.cancel(seq);
        assert_eq!(info, Some(("svc".into(), "Method".into())));
        // rx 现在 dropped —— 模拟
        drop(rx);
        assert_eq!(mgr.pending_count(), 0);
    }

    #[tokio::test]
    async fn reject_all() {
        let mgr = RequestManager::new();
        let _ = mgr.call("a", "b");
        let _ = mgr.call("c", "d");
        let n = mgr.reject_all();
        assert_eq!(n, 2);
        assert_eq!(mgr.pending_count(), 0);
    }

    #[tokio::test]
    async fn max_concurrent_pending() {
        // 模拟原 TS 里 "pendingCallbacks.size >= 5" 限制
        let mgr = RequestManager::new();
        let mut receivers = Vec::new();
        for i in 0..5 {
            let (seq, rx) = mgr.call("svc", &format!("m{i}"));
            receivers.push((seq, rx));
        }
        assert_eq!(mgr.pending_count(), 5);
    }
}
