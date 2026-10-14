use super::super::*;
use std::time::Duration;

impl FarmDesktop {
    pub(crate) fn request_quit(&mut self, cx: &mut Context<Self>) {
        if self.quitting {
            return;
        }
        self.quitting = true;
        self.loading = false;
        if let Some(subscription) = self.realtime_subscription.take() {
            subscription.abort();
        }
        self.request_revision = self.request_revision.wrapping_add(1);
        self.close_account_form();
        self.persist_ui_preferences();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::lifecycle::shutdown(&app, Duration::from_secs(5)).await;
                })
                .await;
            if let Err(error) = result {
                tracing::error!(%error, "等待引擎退出的任务失败");
            }
            let _ = this.update(cx, |_, cx| cx.quit());
        })
        .detach();
    }
}
