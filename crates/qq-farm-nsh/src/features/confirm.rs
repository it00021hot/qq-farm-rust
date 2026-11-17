use nsh_base::alert_dialog;
use nsh_core::prelude::*;

pub(crate) fn confirm(
    title: &'static str,
    body: &'static str,
    action_id: &'static str,
    action_label: &'static str,
    then: impl FnOnce(bool) + 'static,
) {
    let answer = alert_dialog()
        .title(t!(title))
        .description(t!(body))
        .cancel("cancel", t!("accounts.cancel"))
        .destructive(action_id, t!(action_label))
        .close_on_escape(true)
        .show();
    spawn(async move {
        let accepted = answer.await.is_ok_and(|result| result.action.as_deref() == Some(action_id));
        then(accepted);
    });
}
