//! 网关心跳任务注册（判死策略对齐 node `keepalive-policy.ts`）。

use super::*;

impl WorkerLoop {
    /// 注册心跳 interval 任务
    pub(super) fn start_heartbeat_task(self: &Arc<Self>, scheduler: &Scheduler) {

    }
}
