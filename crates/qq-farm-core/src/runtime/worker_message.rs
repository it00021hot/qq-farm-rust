//! 外部发给 Worker 的消息。

use serde::{Deserialize, Serialize};

/// Worker 控制消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerMessage {
    /// 连接
    Connect,
    /// 断开
    Disconnect,
    /// 重载配置
    ReloadConfig,
    /// 自定义消息（业务扩展用）
    Custom {
        tag: String,
        payload: serde_json::Value,
    },
}
