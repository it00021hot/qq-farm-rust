//! Worker 生命周期事件。
//!
//! 通过 tokio 的 `broadcast::channel` 分发给所有订阅者（admin server / Socket.io 等）。

use crate::models::AccountSession;
use crate::runtime::scheduler::SchedulerSnapshot;
use serde::Serialize;

/// Worker 事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerEvent {
    /// Worker 已启动
    Started {
        account_id: String,
        account_name: String,
    },
    /// Worker 已停止
    Stopped {
        account_id: String,
        reason: String,
    },
    /// Worker 状态更新（轮询周期上报）
    Status {
        account_id: String,
        account_name: String,
        /// 序列化后的状态（用户金币/等级/土地/好友数等）
        status: serde_json::Value,
    },
    /// Worker 出错
    Error {
        account_id: String,
        message: String,
    },
    /// Worker 日志
    Log {
        account_id: String,
        account_name: String,
        level: String,
        module: String,
        message: String,
    },
    /// 调度器快照（用于 UI 展示）
    Schedulers {
        schedulers: Vec<SchedulerSnapshot>,
    },
}

impl WorkerEvent {
    /// 关联的 account_id
    #[must_use]
    pub fn account_id(&self) -> Option<&str> {
        match self {
            Self::Started { account_id, .. }
            | Self::Stopped { account_id, .. }
            | Self::Status { account_id, .. }
            | Self::Error { account_id, .. }
            | Self::Log { account_id, .. } => Some(account_id),
            Self::Schedulers { .. } => None,
        }
    }
}

impl AccountSession {
    /// 便捷构造（如果还没有在 models 里）
    pub fn _dummy() -> Self {
        Self::new("_dummy", "_dummy", "_dummy")
    }
}
