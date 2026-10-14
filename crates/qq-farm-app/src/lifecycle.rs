//! 桌面宿主共用的引擎退出门面。

use std::time::Duration;

use crate::AppContext;

/// 拒绝新会话并等待现有游戏任务停止。宿主随后停止 runtime，再释放 EngineLease。
pub async fn shutdown(app: &AppContext, timeout: Duration) {
    app.engine.shutdown_and_wait(timeout).await;
}
