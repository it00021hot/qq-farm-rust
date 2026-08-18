//! Worker 控制句柄。
//!
//! 从 [`Worker`] 或 [`RuntimeEngine`] 获取，用来：
//! - 发控制消息（connect / disconnect / reload）
//! - 取消 worker
//! - 跟 worker 子任务通信

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::runtime::worker_message::WorkerMessage;

/// Worker 句柄（Clone）
#[derive(Clone)]
pub struct WorkerHandle {
    /// 关联账号 ID
    pub account_id: String,
    /// 消息发送端
    pub(crate) msg_tx: mpsc::Sender<WorkerMessage>,
    /// 取消 token（clone 出来多次取消都生效）
    pub(crate) cancel: CancellationToken,
}

impl WorkerHandle {
    /// 异步发消息（不关心结果）
    pub async fn send(
        &self,
        msg: WorkerMessage,
    ) -> Result<(), mpsc::error::SendError<WorkerMessage>> {
        self.msg_tx.send(msg).await
    }

    /// 同步尝试发（不阻塞）
    pub fn try_send(
        &self,
        msg: WorkerMessage,
    ) -> Result<(), mpsc::error::TrySendError<WorkerMessage>> {
        self.msg_tx.try_send(msg)
    }

    /// 取消 worker
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// 检查是否已取消
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}
