use std::sync::Arc;

use nsh_base::alert_dialog;
use nsh_core::prelude::*;
use qq_farm_app::accounts::AclPolicy;

use crate::session;

pub(crate) struct AccountToggle {
    pub id: String,
    pub running: bool,
    pub pending: Signal<Option<String>>,
    pub failure: Signal<Option<String>>,
}

pub(crate) fn toggle_account(toggle: AccountToggle, done: impl Fn() + 'static) {
    if toggle.pending.get_untracked().is_some() {
        return;
    }
    let AccountToggle { id, running, pending, failure } = toggle;
    pending.set(Some(id.clone()));
    failure.set(None);
    if running {
        confirm_stop(id, pending, failure, done);
        return;
    }
    spawn(async move {
        finish(id, true, pending, failure).await;
        done();
    });
}

fn confirm_stop(
    id: String,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    done: impl Fn() + 'static,
) {
    let answer = alert_dialog()
        .title(t!("accounts.confirmStop.title"))
        .description(t!("accounts.confirmStop.body"))
        .cancel("cancel", t!("accounts.cancel"))
        .destructive("stop", t!("accounts.stop"))
        .close_on_escape(true)
        .show();
    spawn(async move {
        let confirmed = answer.await.is_ok_and(|result| result.action.as_deref() == Some("stop"));
        if confirmed {
            finish(id, false, pending, failure).await;
            done();
            return;
        }
        pending.set(None);
    });
}

async fn finish(
    id: String,
    start: bool,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
) {
    let message = command(id, start).await.err();
    failure.set(message);
    pending.set(None);
}

async fn command(id: String, start: bool) -> Result<(), String> {
    let joined = session::session()
        .runtime
        .spawn_blocking(move || apply_command(id, start))
        .await
        .map_err(|error| error.to_string())?;
    joined
}

fn apply_command(id: String, start: bool) -> Result<(), String> {
    let context = Arc::clone(&session::session().context);
    let policy = AclPolicy::LocalOwner;
    if start {
        qq_farm_app::accounts::start_account(&context, &policy, &id).map(|_| ())
    } else {
        qq_farm_app::accounts::stop_account(&context, &policy, &id)
    }
    .map_err(|error| error.to_string())
}
