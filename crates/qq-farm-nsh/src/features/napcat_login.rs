use nsh_core::prelude::*;

use qq_farm_core::services::napcat_login::{flow_still_current, QqScanAction};

use super::accounts::AccountRow;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, field, ButtonVariant};

pub(crate) fn section(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let enabled = signal(false);
    let endpoint = signal(String::new());
    let signature = signal(String::new());
    let generation = signal(0_u64);
    let task_id = signal(String::new());
    let image_src = signal(String::new());
    let status = signal(String::new());
    spawn(async move {
        let loaded = session::call(async move { Ok(qq_farm_app::qq_login::settings()) }).await;
        if let Ok(settings) = loaded {
            enabled.set(settings.qq_qr_login);
            endpoint.set(settings.nap_cat_endpoint);
            signature.set(settings.nap_cat_signature);
        }
    });
    view()
        .gap_2()
        .child(text(move || text_key("accounts.qqEnable")).text_sm())
        .child(
            button(move || text_key("accounts.qqEnable"), ButtonVariant::Quiet)
                .selected(move || enabled.get())
                .on_press(move || enabled.set(!enabled.get_untracked())),
        )
        .child(field(
            endpoint,
            move || text_key("accounts.qqEndpoint").to_string(),
            move |text| endpoint.set(text.to_owned()),
        ))
        .child(field(
            signature,
            move || text_key("accounts.qqSignature").to_string(),
            move |text| signature.set(text.to_owned()),
        ))
        .child(
            button(move || text_key("accounts.qqSave"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || save_settings(enabled, endpoint, signature, pending, failure)),
        )
        .child(when(
            move || enabled.get(),
            move || {
                scan_controls(label, pending, failure, rows, generation, task_id, image_src, status)
            },
        ))
}

fn scan_controls(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    generation: Signal<u64>,
    task_id: Signal<String>,
    image_src: Signal<String>,
    status: Signal<String>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(
            button(move || text_key("accounts.qqScan"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    start_scan(
                        label, pending, failure, rows, generation, task_id, image_src, status,
                    )
                }),
        )
        .child(when(
            move || !task_id.get().is_empty(),
            move || {
                button(move || text_key("accounts.qqCancel"), ButtonVariant::Quiet)
                    .on_press(move || cancel_scan(generation, task_id, image_src, status))
            },
        ))
        .child(when(
            move || !status.get().is_empty(),
            move || text(move || text_key(status_key(&status.get()))).text_sm(),
        ))
        .child(when(
            move || !image_src.get().is_empty(),
            move || image(move || image_src.get()).w(px(180.0)).h(px(180.0)),
        ))
}

fn save_settings(
    enabled: Signal<bool>,
    endpoint: Signal<String>,
    signature: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    pending.set(Some("qq-settings".to_owned()));
    failure.set(None);
    let settings = qq_farm_core::models::store::global_config::QqLoginSettings {
        qq_qr_login: enabled.get_untracked(),
        nap_cat_endpoint: endpoint.get_untracked(),
        nap_cat_signature: signature.get_untracked(),
    };
    spawn(async move {
        let result =
            session::call(async move { qq_farm_app::qq_login::save_settings(settings) }).await;
        if let Err(message) = result {
            failure.set(Some(message));
            enabled.set(false);
        }
        pending.set(None);
    });
}

fn start_scan(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    generation: Signal<u64>,
    task_id: Signal<String>,
    image_src: Signal<String>,
    status: Signal<String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let previous = task_id.get_untracked();
    let current = generation.get_untracked().wrapping_add(1);
    generation.set(current);
    task_id.set(String::new());
    image_src.set(String::new());
    status.set("waiting_scan".to_owned());
    pending.set(Some("qq".to_owned()));
    failure.set(None);
    spawn(async move {
        if !previous.is_empty() {
            let _ = session::call({
                let previous = previous.clone();
                async move { qq_farm_app::qq_login::cancel(&previous).await }
            })
            .await;
        }
        if generation.get_untracked() != current {
            pending.set(None);
            return;
        }
        let created =
            session::call(async move { qq_farm_app::qq_login::create_task().await }).await;
        if generation.get_untracked() != current {
            pending.set(None);
            return;
        }
        match created {
            Ok(task) => {
                task_id.set(task.task_id.clone());
                image_src.set(task.qr_image);
                status.set(task.status);
                pending.set(None);
                poll_scan(
                    label,
                    pending,
                    failure,
                    rows,
                    generation,
                    current,
                    task.task_id,
                    task_id,
                    image_src,
                    status,
                );
            }
            Err(message) => {
                failure.set(Some(message));
                status.set(String::new());
                pending.set(None);
            }
        }
    });
}

fn cancel_scan(
    generation: Signal<u64>,
    task_id: Signal<String>,
    image_src: Signal<String>,
    status: Signal<String>,
) {
    let previous = task_id.get_untracked();
    generation.set(generation.get_untracked().wrapping_add(1));
    task_id.set(String::new());
    image_src.set(String::new());
    status.set(String::new());
    if previous.is_empty() {
        return;
    }
    spawn(async move {
        let _ = session::call(async move { qq_farm_app::qq_login::cancel(&previous).await }).await;
    });
}

fn poll_scan(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    generation: Signal<u64>,
    current: u64,
    active_task: String,
    task_id: Signal<String>,
    image_src: Signal<String>,
    status: Signal<String>,
) {
    let runtime = session::session().runtime.clone();
    spawn(async move {
        let _ = runtime
            .spawn(async {
                tokio::time::sleep(qq_farm_core::services::napcat_login::POLL_GAP).await
            })
            .await;
        if !flow_still_current(
            current,
            generation.get_untracked(),
            &active_task,
            &task_id.get_untracked(),
        ) {
            return;
        }
        let polled_id = active_task.clone();
        let result =
            session::call(async move { qq_farm_app::qq_login::query_status(&polled_id).await })
                .await;
        if !flow_still_current(
            current,
            generation.get_untracked(),
            &active_task,
            &task_id.get_untracked(),
        ) {
            return;
        }
        match result {
            Ok(task) => {
                if !task.qr_image.is_empty() {
                    image_src.set(task.qr_image);
                }
                status.set(task.status.clone());
                match qq_farm_app::qq_login::next_action(&task.status) {
                    QqScanAction::Waiting | QqScanAction::Scanned | QqScanAction::Continue => {
                        poll_scan(
                            label,
                            pending,
                            failure,
                            rows,
                            generation,
                            current,
                            active_task,
                            task_id,
                            image_src,
                            status,
                        );
                    }
                    QqScanAction::FetchCode => finish_scan(
                        label,
                        pending,
                        failure,
                        rows,
                        generation,
                        current,
                        active_task,
                        task_id,
                        image_src,
                        status,
                    ),
                    QqScanAction::Expired => {
                        failure.set(Some(text_key("accounts.qqExpired").to_string()))
                    }
                    QqScanAction::Invalid => {
                        failure.set(Some(text_key("accounts.qqInvalid").to_string()))
                    }
                }
            }
            Err(message) => failure.set(Some(message)),
        }
    });
}

fn finish_scan(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    generation: Signal<u64>,
    current: u64,
    active_task: String,
    task_id: Signal<String>,
    image_src: Signal<String>,
    status: Signal<String>,
) {
    if !flow_still_current(
        current,
        generation.get_untracked(),
        &active_task,
        &task_id.get_untracked(),
    ) || pending.get_untracked().is_some()
    {
        return;
    }
    let name = label.get_untracked();
    pending.set(Some("qq".to_owned()));
    status.set("confirmed".to_owned());
    spawn(async move {
        let task_for_code = active_task.clone();
        let active_for_code = task_id.get_untracked();
        let generation_for_code = generation.get_untracked();
        let code = session::call(async move {
            qq_farm_app::qq_login::take_code(
                current,
                generation_for_code,
                &task_for_code,
                &active_for_code,
            )
            .await
        })
        .await;
        if !flow_still_current(
            current,
            generation.get_untracked(),
            &active_task,
            &task_id.get_untracked(),
        ) {
            pending.set(None);
            return;
        }
        let login = match code {
            Ok(Some(login)) => login,
            Ok(None) => {
                pending.set(None);
                return;
            }
            Err(message) => {
                failure.set(Some(message));
                pending.set(None);
                return;
            }
        };
        let context = session::session().context.clone();
        let task_for_store = active_task.clone();
        let active_for_store = task_id.get_untracked();
        let generation_for_store = generation.get_untracked();
        let result = session::call(async move {
            qq_farm_app::qq_login::store_code(
                &context,
                current,
                generation_for_store,
                &task_for_store,
                &active_for_store,
                login,
                &name,
            )
            .await
        })
        .await;
        if !flow_still_current(
            current,
            generation.get_untracked(),
            &active_task,
            &task_id.get_untracked(),
        ) {
            pending.set(None);
            return;
        }
        match result {
            Ok(true) => {
                generation.set(current.wrapping_add(1));
                task_id.set(String::new());
                image_src.set(String::new());
                status.set(String::new());
                label.set(String::new());
                rows.refetch();
            }
            Ok(false) => {}
            Err(message) => failure.set(Some(message)),
        }
        pending.set(None);
    });
}

fn status_key(status: &str) -> &'static str {
    match qq_farm_app::qq_login::next_action(status) {
        QqScanAction::Waiting => "accounts.qqWaiting",
        QqScanAction::Scanned => "accounts.qqScanned",
        QqScanAction::FetchCode => "accounts.qqConfirmed",
        QqScanAction::Expired => "accounts.qqExpired",
        QqScanAction::Invalid => "accounts.qqInvalid",
        QqScanAction::Continue => "accounts.qqWaiting",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displayed_status_follows_the_napcat_action_and_late_flows_drop() {
        assert_eq!(status_key("waiting_scan"), "accounts.qqWaiting");
        assert_eq!(status_key("scanned"), "accounts.qqScanned");
        assert_eq!(status_key("confirmed"), "accounts.qqConfirmed");
        assert_eq!(status_key("expired"), "accounts.qqExpired");
        assert_eq!(status_key("cancelled"), "accounts.qqExpired");
        assert_eq!(status_key(""), "accounts.qqInvalid");
        assert!(flow_still_current(4, 4, "task-1", "task-1"));
        assert!(!flow_still_current(4, 5, "task-1", "task-1"));
        assert!(!flow_still_current(4, 4, "task-1", "task-2"));
    }
}
