use nsh_core::prelude::*;
use serde_json::Value;

use qq_farm_app::accounts::{AclPolicy, UpsertAccountRequest};

use super::account_toggle::{toggle_account, AccountToggle};
use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, field, notice, BadgeTone, ButtonVariant};

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct AccountRow {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub running: bool,
}

pub(crate) fn account_rows(value: &Value) -> Vec<AccountRow> {
    value
        .get("accounts")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(account_row).collect())
        .unwrap_or_default()
}

fn account_row(value: &Value) -> Option<AccountRow> {
    let id = value.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let title = ["nick", "name", "remark"]
        .into_iter()
        .find_map(|field| nonempty(value, field))
        .unwrap_or_else(|| id.to_owned());
    Some(AccountRow {
        id: id.to_owned(),
        title,
        platform: nonempty(value, "platform").unwrap_or_default(),
        running: value.get("running").and_then(Value::as_bool).unwrap_or(false),
    })
}

pub(crate) fn account_is_running(rows: &[AccountRow], id: &str) -> bool {
    rows.iter().any(|row| row.id == id && row.running)
}

fn nonempty(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

pub(crate) fn load_accounts() -> Resource<Vec<AccountRow>, String> {
    resource(
        || Some(()),
        |(), _| async {
            let value = crate::session::session()
                .runtime
                .spawn_blocking(|| {
                    qq_farm_app::accounts::list_accounts_enriched(
                        &crate::session::session().context,
                        None,
                    )
                })
                .await
                .map_err(|error| error.to_string())?;
            Ok(account_rows(&value))
        },
    )
}

pub(crate) struct AccountPage {
    pub rows: Resource<Vec<AccountRow>, String>,
    pub selected: Signal<Option<String>>,
    pub pending: Signal<Option<String>>,
    pub failure: Signal<Option<String>>,
}

pub(crate) fn page(page: AccountPage) -> impl IntoElement {
    let AccountPage { rows, selected, pending, failure } = page;
    let code = signal(String::new());
    let label = signal(String::new());
    let platform = signal("qq".to_owned());
    let wx_generation = signal(0_u64);
    let wx_image = signal(String::new());
    let wx_status = signal(String::new());
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(text(move || text_key("accounts.title")).text_lg().font_weight(FontWeight::SEMIBOLD))
        .child(failure_line(failure))
        .child(add_form(
            code,
            label,
            platform,
            pending,
            failure,
            rows,
            wx_generation,
            wx_image,
            wx_status,
        ))
        .child(when(
            move || rows.loading() && rows.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || rows.error().is_some(),
            move || {
                view().gap_2().child(failure_text(rows)).child(
                    button(move || text_key("common.retry"), ButtonVariant::Secondary)
                        .on_press(move || rows.refetch()),
                )
            },
        ))
        .child(when(
            move || rows.get().is_some_and(|rows| rows.is_empty()),
            move || notice(move || text_key("accounts.empty")),
        ))
        .child(each(
            move || rows.get().unwrap_or_default(),
            |row: &AccountRow| row.id.clone(),
            move |row, _| account_line(row.get_untracked(), selected, pending, failure, rows),
        ))
}

fn failure_text(rows: Resource<Vec<AccountRow>, String>) -> impl IntoElement {
    crate::ui::failure(move || rows.error().map(SharedString::from).unwrap_or_default())
}

fn account_line(
    row: AccountRow,
    selected: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let select_id = row.id.clone();
    let title = SharedString::from(row.title.clone());
    let platform = platform_label(&row.platform);
    let running = row.running;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(
            button(title, ButtonVariant::Quiet)
                .flex_1()
                .justify_start()
                .selected({
                    let selected_id = select_id.clone();
                    move || selected.get().as_deref() == Some(selected_id.as_str())
                })
                .on_press(move || selected.set(Some(select_id.clone()))),
        )
        .child(text(platform).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(badge(
            move || text_key(if running { "accounts.running" } else { "accounts.stopped" }),
            if running { BadgeTone::Success } else { BadgeTone::Neutral },
        ))
        .child(command_button(row.clone(), pending, failure, rows))
        .child(delete_button(row, selected, pending, failure, rows))
}

fn command_button(
    row: AccountRow,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> nsh_base::ButtonElement {
    let id = row.id.clone();
    let running = row.running;
    let command_id = id.clone();
    button(
        move || {
            if pending.get().as_deref() == Some(id.as_str()) {
                text_key("accounts.working")
            } else if running {
                text_key("accounts.stop")
            } else {
                text_key("accounts.start")
            }
        },
        if running { ButtonVariant::Danger } else { ButtonVariant::Primary },
    )
    .disabled(move || pending.get().is_some())
    .on_press(move || {
        toggle_account(
            AccountToggle { id: command_id.clone(), running, pending, failure },
            move || rows.refetch(),
        );
    })
}

fn add_form(
    code: Signal<String>,
    label: Signal<String>,
    platform: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    wx_generation: Signal<u64>,
    wx_image: Signal<String>,
    wx_status: Signal<String>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(field(
            move || code.get(),
            move || text_key("accounts.code").to_string(),
            move |text: &str| code.set(text.to_owned()),
        ))
        .child(field(
            move || label.get(),
            move || text_key("accounts.remark").to_string(),
            move |text: &str| label.set(text.to_owned()),
        ))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    button(move || text_key("platform.qq"), ButtonVariant::Quiet)
                        .selected(move || platform.get() == "qq")
                        .on_press(move || {
                            platform.set("qq".to_owned());
                            stop_wx(wx_generation, wx_image, wx_status);
                        }),
                )
                .child(
                    button(move || text_key("platform.wx"), ButtonVariant::Quiet)
                        .selected(move || platform.get() == "wx")
                        .on_press(move || platform.set("wx".to_owned())),
                )
                .child(
                    button(move || text_key("accounts.add"), ButtonVariant::Primary)
                        .disabled(move || pending.get().is_some() || code.get().trim().is_empty())
                        .on_press(move || {
                            add_account(code, label, platform, pending, failure, rows)
                        }),
                ),
        )
        .child(when(
            move || platform.get() == "wx",
            move || wx_panel(label, pending, failure, rows, wx_generation, wx_image, wx_status),
        ))
}

fn add_account(
    code: Signal<String>,
    label: Signal<String>,
    platform: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let login = code.get_untracked().trim().to_owned();
    if login.is_empty() {
        return;
    }
    let name = label.get_untracked().trim().to_owned();
    let platform = platform.get_untracked();
    pending.set(Some("add".to_owned()));
    failure.set(None);
    let context = session::session().context.clone();
    spawn(async move {
        let result = session::call(async move {
            qq_farm_app::accounts::upsert_account(
                &context,
                &AclPolicy::LocalOwner,
                UpsertAccountRequest {
                    code: Some(login),
                    name: (!name.is_empty()).then(|| name.clone()),
                    remark: (!name.is_empty()).then_some(name),
                    platform: Some(platform),
                    username: Some("local".to_owned()),
                    ..UpsertAccountRequest::default()
                },
            )
            .await
        })
        .await;
        match result {
            Ok(_) => {
                code.set(String::new());
                label.set(String::new());
            }
            Err(message) => failure.set(Some(message)),
        }
        pending.set(None);
        rows.refetch();
    });
}

fn wx_panel(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    generation: Signal<u64>,
    image_src: Signal<String>,
    status: Signal<String>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(
            button(move || text_key("accounts.wxQr"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    start_wx(label, pending, failure, rows, generation, image_src, status)
                }),
        )
        .child(when(
            move || !status.get().is_empty(),
            move || text(move || text_key(wx_status_key(&status.get()))).text_sm(),
        ))
        .child(when(
            move || !image_src.get().is_empty(),
            move || image(move || image_src.get()).w(px(180.0)).h(px(180.0)),
        ))
}

fn stop_wx(generation: Signal<u64>, image_src: Signal<String>, status: Signal<String>) {
    generation.set(generation.get_untracked().wrapping_add(1));
    image_src.set(String::new());
    status.set(String::new());
}

fn start_wx(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    generation: Signal<u64>,
    image_src: Signal<String>,
    status: Signal<String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let current = generation.get_untracked().wrapping_add(1);
    generation.set(current);
    image_src.set(String::new());
    status.set("loading".to_owned());
    pending.set(Some("wx".to_owned()));
    failure.set(None);
    spawn(async move {
        let context = session::session().context.clone();
        let result =
            session::call(
                async move { qq_farm_app::wx_login::create_task(&context.wx_login).await },
            )
            .await;
        if generation.get_untracked() != current {
            pending.set(None);
            return;
        }
        match result {
            Ok(task) => {
                let task_id = task.task_id;
                image_src.set(jpeg_data_url(&task.qr_jpeg));
                status.set("waiting".to_owned());
                pending.set(None);
                poll_wx(
                    label, pending, failure, rows, generation, current, task_id, image_src, status,
                );
            }
            Err(message) => {
                failure.set(Some(message));
                status.set("failed".to_owned());
                pending.set(None);
            }
        }
    });
}

fn poll_wx(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    generation: Signal<u64>,
    current: u64,
    task_id: String,
    image_src: Signal<String>,
    status: Signal<String>,
) {
    let runtime = session::session().runtime.clone();
    spawn(async move {
        let _ = runtime
            .spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
            })
            .await;
        if generation.get_untracked() != current {
            return;
        }
        let context = session::session().context.clone();
        let polled = task_id.clone();
        let result = session::call(async move {
            qq_farm_app::wx_login::poll_status(&context.wx_login, &polled).await
        })
        .await;
        if generation.get_untracked() != current {
            return;
        }
        match result {
            Ok(polled) => {
                status.set(polled.status.clone());
                match polled.status.as_str() {
                    "waiting" | "scanned" => poll_wx(
                        label, pending, failure, rows, generation, current, task_id, image_src,
                        status,
                    ),
                    "authorized" => finish_wx(
                        label, pending, failure, rows, generation, current, task_id, image_src,
                        status,
                    ),
                    _ => {}
                }
            }
            Err(message) => {
                failure.set(Some(message));
                status.set("failed".to_owned());
            }
        }
    });
}

fn finish_wx(
    label: Signal<String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
    generation: Signal<u64>,
    current: u64,
    task_id: String,
    image_src: Signal<String>,
    status: Signal<String>,
) {
    if generation.get_untracked() != current || pending.get_untracked().is_some() {
        return;
    }
    let name = label.get_untracked().trim().to_owned();
    pending.set(Some("wx".to_owned()));
    status.set("authorized".to_owned());
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::wx_login::confirm(&context.wx_login, &task_id).await?;
            let login = qq_farm_app::wx_login::issue_code(&context.wx_login, &task_id).await?;
            let display = if name.is_empty() {
                login.nickname.clone().unwrap_or_default()
            } else {
                name.clone()
            };
            qq_farm_app::accounts::upsert_account(
                &context,
                &AclPolicy::LocalOwner,
                UpsertAccountRequest {
                    code: Some(login.code),
                    name: (!display.is_empty()).then(|| display.clone()),
                    remark: (!name.is_empty()).then_some(name),
                    platform: Some("wx".to_owned()),
                    username: Some("local".to_owned()),
                    ..UpsertAccountRequest::default()
                },
            )
            .await
        })
        .await;
        if generation.get_untracked() != current {
            pending.set(None);
            return;
        }
        match result {
            Ok(_) => {
                generation.set(current.wrapping_add(1));
                image_src.set(String::new());
                status.set(String::new());
                label.set(String::new());
            }
            Err(message) => {
                failure.set(Some(message));
                status.set("failed".to_owned());
            }
        }
        pending.set(None);
        rows.refetch();
    });
}

fn jpeg_data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn wx_status_key(status: &str) -> &'static str {
    match status {
        "loading" => "accounts.wxLoading",
        "waiting" => "accounts.wxWaiting",
        "scanned" => "accounts.wxScanned",
        "authorized" => "accounts.wxAuthorized",
        "cancelled" | "expired" => "accounts.wxExpired",
        _ => "accounts.wxFailed",
    }
}

fn delete_button(
    row: AccountRow,
    selected: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> nsh_base::ButtonElement {
    let id = row.id.clone();
    let key = format!("delete:{}", row.id);
    button(
        move || {
            if pending.get().as_deref() == Some(key.as_str()) {
                text_key("accounts.working")
            } else {
                text_key("accounts.delete")
            }
        },
        ButtonVariant::Danger,
    )
    .disabled(move || pending.get().is_some())
    .on_press(move || delete_account(id.clone(), selected, pending, failure, rows))
}

fn delete_account(
    id: String,
    selected: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let key = format!("delete:{id}");
    pending.set(Some(key));
    failure.set(None);
    let account_id = id.clone();
    confirm::confirm(
        "accounts.confirmDelete.title",
        "accounts.confirmDelete.body",
        "delete",
        "accounts.delete",
        move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            let context = session::session().context.clone();
            spawn(async move {
                let result = session::session()
                    .runtime
                    .spawn_blocking(move || {
                        qq_farm_app::accounts::delete_account(
                            &context,
                            &AclPolicy::LocalOwner,
                            &account_id,
                        )
                    })
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|deleted| deleted.map_err(|error| error.to_string()));
                if result.is_ok() && selected.get().as_deref() == Some(id.as_str()) {
                    selected.set(None);
                }
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(None);
                rows.refetch();
            });
        },
    );
}

fn platform_label(platform: &str) -> SharedString {
    match platform {
        "wx" => text_key("platform.wx"),
        "qq" => text_key("platform.qq"),
        other => SharedString::from(other.to_owned()),
    }
}

pub(crate) fn failure_line(message: Signal<Option<String>>) -> impl IntoElement {
    crate::ui::failure(move || message.get().map(SharedString::from).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{account_is_running, account_rows, jpeg_data_url, wx_status_key, AccountRow};

    #[test]
    fn wechat_status_and_qr_use_stable_keys() {
        assert_eq!(wx_status_key("waiting"), "accounts.wxWaiting");
        assert_eq!(wx_status_key("scanned"), "accounts.wxScanned");
        assert_eq!(wx_status_key("expired"), "accounts.wxExpired");
        assert_eq!(wx_status_key("cancelled"), "accounts.wxExpired");
        assert!(jpeg_data_url(&[0xff, 0xd8]).starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn missing_accounts_are_empty() {
        assert!(account_rows(&json!({})).is_empty());
        assert!(account_rows(&json!({ "accounts": {} })).is_empty());
    }

    #[test]
    fn one_account_prefers_nick_then_name() {
        let rows = account_rows(&json!({
            "accounts": [{ "id": "7", "name": "stored", "nick": "shown", "platform": "qq", "running": true }]
        }));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "7");
        assert_eq!(rows[0].title, "shown");
        assert!(rows[0].running);
    }

    #[test]
    fn blank_id_is_skipped_and_order_is_kept() {
        let rows = account_rows(&json!({
            "accounts": [
                { "id": "", "name": "drop" },
                { "id": "a", "remark": "first" },
                { "name": "no-id" },
                { "id": "b", "name": "second" }
            ]
        }));
        assert_eq!(rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(rows[0].title, "first");
        assert!(!rows[1].running);
    }

    #[test]
    fn running_matches_only_the_selected_id() {
        let rows = vec![
            AccountRow {
                id: "a".to_owned(),
                title: "a".to_owned(),
                platform: "qq".to_owned(),
                running: false,
            },
            AccountRow {
                id: "b".to_owned(),
                title: "b".to_owned(),
                platform: "qq".to_owned(),
                running: true,
            },
        ];
        assert!(!account_is_running(&rows, "a"));
        assert!(account_is_running(&rows, "b"));
        assert!(!account_is_running(&rows, "c"));
    }
}
