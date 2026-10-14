use super::super::*;
use std::collections::HashSet;

#[derive(Default)]
pub(crate) struct AccountLifecycleState {
    pending_ids: HashSet<String>,
}

impl AccountLifecycleState {
    pub(crate) fn begin(&mut self, account_id: &str) -> bool {
        self.pending_ids.insert(account_id.to_string())
    }

    pub(crate) fn finish(&mut self, account_id: &str) {
        self.pending_ids.remove(account_id);
    }

    pub(crate) fn is_pending(&self, account_id: &str) -> bool {
        self.pending_ids.contains(account_id)
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn run_account_lifecycle(
        &mut self,
        id: String,
        start: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.accounts.lifecycle.begin(&id) {
            return;
        }
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let worker_id = id.clone();
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    let policy = qq_farm_app::accounts::AclPolicy::LocalOwner;
                    if start {
                        qq_farm_app::accounts::start_account(&app, &policy, &worker_id).map(|_| ())
                    } else {
                        qq_farm_app::accounts::stop_account(&app, &policy, &worker_id)
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.accounts.lifecycle.finish(&id);
                match result {
                    Ok(Ok(())) => {
                        this.show_account_success("更新成功", "Update Success", cx);
                        this.refresh(cx);
                    }
                    Ok(Err(error)) => {
                        this.error = Some(error.to_string());
                        cx.notify();
                    }
                    Err(error) => {
                        this.error = Some(crate::i18n::background_task_error(
                            &this.preferences.language,
                            error,
                        ));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::AccountLifecycleState;

    #[test]
    fn lifecycle_pending_is_per_account_and_rejects_duplicate_actions() {
        let mut state = AccountLifecycleState::default();

        assert!(state.begin("account-a"));
        assert!(state.begin("account-b"));
        assert!(!state.begin("account-a"));
        assert!(state.is_pending("account-a"));
        assert!(state.is_pending("account-b"));

        state.finish("account-a");
        assert!(!state.is_pending("account-a"));
        assert!(state.is_pending("account-b"));
    }
}
