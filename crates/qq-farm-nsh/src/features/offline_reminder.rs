use std::time::Duration;

use nsh_core::prelude::*;
use qq_farm_core::services::qq_bot::BindPollStatus;
use serde_json::{json, Value};

use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, field, notice, ButtonVariant};

const OWNER: &str = "local";
const DOCS_URL: &str = "https://bot.q.qq.com/wiki/";

const PROVIDERS: &[(&str, &str)] = &[
    ("none", "offline.none"),
    ("qq_bot", "offline.qq"),
    ("ding_talk", "offline.ding"),
    ("wechat_bot", "offline.wechat"),
];

pub(crate) fn section() -> impl IntoElement {
    let draft = signal(None::<Value>);
    let status = signal(Value::Null);
    let phase = signal("");
    let qr = signal(String::new());
    let listening = signal(false);
    let generation = signal(0_u64);
    let pending = signal(false);
    let failure = signal(None::<String>);
    let outcome = signal(None::<String>);
    spawn(async move {
        let loaded = session::session()
            .runtime
            .spawn_blocking(|| {
                (
                    qq_farm_app::settings::offline_reminder_view(Some(OWNER)),
                    qq_farm_app::qq_bot_bind::qq_bot_bind_status(Some(OWNER)),
                )
            })
            .await;
        match loaded {
            Ok((config, binding)) => {
                draft.set(Some(config));
                status.set(binding);
            }
            Err(error) => failure.set(Some(error.to_string())),
        }
    });
    view()
        .gap_3()
        .child(text(move || text_key("offline.title")).text_sm().font_weight(FontWeight::SEMIBOLD))
        .child(text(move || text_key("offline.hint")).text_sm())
        .child(crate::ui::failure(move || {
            failure.get().map(SharedString::from).unwrap_or_default()
        }))
        .child(when(
            move || outcome.get().is_some(),
            move || notice(move || outcome.get().map(SharedString::from).unwrap_or_default()),
        ))
        .child(when(
            move || draft.get().is_none() && failure.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || draft.get().is_some(),
            move || {
                reminder_form(
                    draft, status, phase, qr, listening, generation, pending, failure, outcome,
                )
            },
        ))
}

fn reminder_form(
    draft: Signal<Option<Value>>,
    status: Signal<Value>,
    phase: Signal<&'static str>,
    qr: Signal<String>,
    listening: Signal<bool>,
    generation: Signal<u64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<String>>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            || PROVIDERS.to_vec(),
            |choice: &(&str, &str)| choice.0.to_owned(),
            move |choice, _| {
                let (id, label) = choice.get_untracked();
                button(move || text_key(label), ButtonVariant::Quiet)
                    .selected(move || provider(draft) == id)
                    .disabled(move || pending.get())
                    .on_press(move || write_pointer(draft, "/provider", id))
            },
        )))
        .child(
            button(move || text_key("offline.docs"), ButtonVariant::Quiet)
                .disabled(move || provider(draft) != "qq_bot" || pending.get())
                .on_press(move || open_link(DOCS_URL, failure)),
        )
        .child(when(
            move || provider(draft) == "qq_bot",
            move || qq_fields(draft, status, phase, qr, listening, generation, pending, failure),
        ))
        .child(when(move || provider(draft) == "ding_talk", move || ding_fields(draft, pending)))
        .child(when(
            move || provider(draft) == "wechat_bot",
            move || notice(move || text_key("offline.wechatMissing")),
        ))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    button(move || text_key("offline.test"), ButtonVariant::Secondary)
                        .disabled(move || pending.get() || !can_test(draft, status))
                        .on_press(move || send_test(draft, pending, failure, outcome)),
                )
                .child(
                    button(move || text_key("offline.save"), ButtonVariant::Primary)
                        .disabled(move || pending.get())
                        .on_press(move || save_reminder(draft, pending, failure, outcome)),
                ),
        )
}

fn qq_fields(
    draft: Signal<Option<Value>>,
    status: Signal<Value>,
    phase: Signal<&'static str>,
    qr: Signal<String>,
    listening: Signal<bool>,
    generation: Signal<u64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(text_row("offline.appId", &["qqBot", "appId"], draft, pending))
        .child(text_row("offline.appSecret", &["qqBot", "clientSecret"], draft, pending))
        .child(text(move || text_key("offline.qqHint")).text_sm())
        .child(
            text(move || {
                let key = if is_bound(&status.get()) { "offline.bound" } else { "offline.unbound" };
                text_key(key)
            })
            .text_sm(),
        )
        .child(when(
            move || !nickname(&status.get()).is_empty(),
            move || text(move || SharedString::from(nickname(&status.get()))).text_sm(),
        ))
        .child(when(
            move || !phase.get().is_empty(),
            move || text(move || text_key(phase.get())).text_sm(),
        ))
        .child(when(
            move || !qr.get().is_empty(),
            move || image(move || qr.get()).w(px(180.0)).h(px(180.0)),
        ))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    button(move || text_key("offline.bind"), ButtonVariant::Primary)
                        .disabled(move || {
                            pending.get() || listening.get() || !credentials_ready(draft)
                        })
                        .on_press(move || {
                            begin_bind(
                                draft, status, phase, qr, listening, generation, pending, failure,
                            )
                        }),
                )
                .child(
                    button(move || text_key("offline.openBot"), ButtonVariant::Quiet)
                        .disabled(move || invite_url(&status.get()).is_empty())
                        .on_press(move || {
                            let url = invite_url(&status.get_untracked());
                            if !url.is_empty() {
                                open_link(&url, failure);
                            }
                        }),
                )
                .child(
                    button(move || text_key("offline.unbind"), ButtonVariant::Secondary)
                        .disabled(move || pending.get() || !is_bound(&status.get()))
                        .on_press(move || {
                            clear_binding(
                                draft, status, phase, qr, listening, generation, pending, failure,
                            )
                        }),
                ),
        )
}

fn ding_fields(draft: Signal<Option<Value>>, pending: Signal<bool>) -> impl IntoElement {
    view()
        .gap_2()
        .child(text_row("offline.endpoint", &["endpoint"], draft, pending))
        .child(text_row("offline.token", &["token"], draft, pending))
        .child(text_row("offline.secret", &["secret"], draft, pending))
        .child(text(move || text_key("offline.dingHint")).text_sm())
}

fn text_row(
    label: &'static str,
    path: &'static [&'static str],
    draft: Signal<Option<Value>>,
    pending: Signal<bool>,
) -> View {
    view().gap_1().child(text(move || text_key(label)).text_sm()).child(field(
        move || read_path(draft, path),
        move || text_key(label).to_string(),
        move |text: &str| {
            if pending.get_untracked() {
                return;
            }
            write_path(draft, path, text);
        },
    ))
}

fn save_reminder(
    draft: Signal<Option<Value>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<String>>,
) {
    if pending.get_untracked() {
        return;
    }
    let Some(config) = draft.get_untracked() else { return };
    let body = prepare(&config);
    pending.set(true);
    failure.set(None);
    outcome.set(None);
    spawn(async move {
        let saved = session::session()
            .runtime
            .spawn_blocking(move || {
                qq_farm_app::settings::set_offline_reminder(Some(OWNER), body);
                qq_farm_app::settings::offline_reminder_view(Some(OWNER))
            })
            .await
            .map_err(|error| error.to_string());
        match saved {
            Ok(config) => {
                draft.set(Some(config));
                outcome.set(Some(text_key("offline.saved").to_string()));
            }
            Err(message) => failure.set(Some(message)),
        }
        pending.set(false);
    });
}

fn send_test(
    draft: Signal<Option<Value>>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<String>>,
) {
    if pending.get_untracked() {
        return;
    }
    let Some(config) = draft.get_untracked() else { return };
    let body = prepare(&config);
    pending.set(true);
    failure.set(None);
    outcome.set(None);
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::settings::test_offline_reminder(&context, Some(OWNER), body).await
        })
        .await;
        match result {
            Ok(value) if value.get("ok").and_then(Value::as_bool) == Some(true) => {
                outcome.set(Some(message_of(&value, "offline.sent")));
            }
            Ok(value) => failure.set(Some(message_of(&value, "offline.failed"))),
            Err(message) => failure.set(Some(message)),
        }
        pending.set(false);
    });
}

fn begin_bind(
    draft: Signal<Option<Value>>,
    status: Signal<Value>,
    phase: Signal<&'static str>,
    qr: Signal<String>,
    listening: Signal<bool>,
    generation: Signal<u64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) {
    if pending.get_untracked() || listening.get_untracked() || !credentials_ready(draft) {
        return;
    }
    let Some(config) = draft.get_untracked() else { return };
    let body = prepare(&config);
    let current = generation.get_untracked().wrapping_add(1);
    generation.set(current);
    qr.set(String::new());
    phase.set("offline.creating");
    pending.set(true);
    failure.set(None);
    spawn(async move {
        let context = session::session().context.clone();
        let started = session::session()
            .runtime
            .spawn_blocking(move || {
                qq_farm_app::settings::set_offline_reminder(Some(OWNER), body);
                qq_farm_app::qq_bot_bind::start_qq_bot_bind(&context, OWNER)
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|started| started.map_err(|error| error.to_string()));
        if generation.get_untracked() != current {
            pending.set(false);
            return;
        }
        match started {
            Ok(start) => {
                let image = start.qr_data_url;
                phase.set(if image.is_empty() { "offline.waitingManual" } else { "offline.scan" });
                qr.set(image);
                listening.set(true);
                pending.set(false);
                poll_bind(
                    draft,
                    status,
                    phase,
                    qr,
                    listening,
                    generation,
                    current,
                    start.session_id,
                );
            }
            Err(message) => {
                phase.set("");
                failure.set(Some(message));
                pending.set(false);
            }
        }
    });
}

fn poll_bind(
    draft: Signal<Option<Value>>,
    status: Signal<Value>,
    phase: Signal<&'static str>,
    qr: Signal<String>,
    listening: Signal<bool>,
    generation: Signal<u64>,
    current: u64,
    session_id: String,
) {
    let runtime = session::session().runtime.clone();
    spawn(async move {
        let _ = runtime
            .spawn(async {
                tokio::time::sleep(Duration::from_secs(2)).await;
            })
            .await;
        if generation.get_untracked() != current {
            return;
        }
        let context = session::session().context.clone();
        let polled_id = session_id.clone();
        let polled = session::session()
            .runtime
            .spawn_blocking(move || {
                let poll = qq_farm_app::qq_bot_bind::poll_qq_bot_bind(&context, &polled_id);
                if let Some(binding) = poll.binding.clone() {
                    qq_farm_app::qq_bot_bind::save_qq_bot_binding(OWNER, binding);
                }
                poll
            })
            .await;
        if generation.get_untracked() != current {
            return;
        }
        let Ok(poll) = polled else { return };
        match poll.status {
            BindPollStatus::Pending => {
                phase.set("offline.waiting");
                poll_bind(draft, status, phase, qr, listening, generation, current, session_id);
            }
            BindPollStatus::Bound => {
                refresh_binding(draft, status);
                qr.set(String::new());
                listening.set(false);
                phase.set("offline.bound");
            }
            BindPollStatus::Expired => {
                qr.set(String::new());
                listening.set(false);
                phase.set("offline.expired");
            }
        }
    });
}

fn clear_binding(
    draft: Signal<Option<Value>>,
    status: Signal<Value>,
    phase: Signal<&'static str>,
    qr: Signal<String>,
    listening: Signal<bool>,
    generation: Signal<u64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) {
    if pending.get_untracked() {
        return;
    }
    generation.set(generation.get_untracked().wrapping_add(1));
    listening.set(false);
    qr.set(String::new());
    pending.set(true);
    failure.set(None);
    spawn(async move {
        let cleared = session::session()
            .runtime
            .spawn_blocking(|| qq_farm_app::qq_bot_bind::unbind_qq_bot(OWNER))
            .await
            .map_err(|error| error.to_string());
        if let Err(message) = cleared {
            failure.set(Some(message));
        } else {
            refresh_binding(draft, status);
            phase.set("offline.unboundDone");
        }
        pending.set(false);
    });
}

fn refresh_binding(draft: Signal<Option<Value>>, status: Signal<Value>) {
    draft.set(Some(qq_farm_app::settings::offline_reminder_view(Some(OWNER))));
    status.set(qq_farm_app::qq_bot_bind::qq_bot_bind_status(Some(OWNER)));
}

fn open_link(url: &str, failure: Signal<Option<String>>) {
    let url = url.to_owned();
    spawn(async move {
        let opened = open_url(url).await;
        if !opened {
            failure.set(Some(text_key("offline.openFailed").to_string()));
        }
    });
}

fn provider(draft: Signal<Option<Value>>) -> String {
    draft
        .get()
        .and_then(|value| value.pointer("/provider").and_then(Value::as_str).map(str::to_owned))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "none".to_owned())
}

fn credentials_ready(draft: Signal<Option<Value>>) -> bool {
    let Some(config) = draft.get() else { return false };
    !read_pointer(&config, "/qqBot/appId").trim().is_empty()
        && !read_pointer(&config, "/qqBot/clientSecret").trim().is_empty()
}

fn can_test(draft: Signal<Option<Value>>, status: Signal<Value>) -> bool {
    let Some(config) = draft.get() else { return false };
    can_test_provider(
        config.pointer("/provider").and_then(Value::as_str).unwrap_or("none"),
        &read_pointer(&config, "/endpoint"),
        &read_pointer(&config, "/token"),
        is_bound(&status.get()),
    )
}

fn is_bound(status: &Value) -> bool {
    status.get("bound").and_then(Value::as_bool).unwrap_or(false)
}

fn nickname(status: &Value) -> String {
    status.pointer("/binding/nickname").and_then(Value::as_str).unwrap_or("").trim().to_owned()
}

fn invite_url(status: &Value) -> String {
    status.get("botInviteUrl").and_then(Value::as_str).unwrap_or("").trim().to_owned()
}

fn message_of(value: &Value, fallback: &'static str) -> String {
    value
        .get("msg")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| text_key(fallback).to_string())
}

fn read_path(draft: Signal<Option<Value>>, path: &[&str]) -> String {
    draft.get().map(|value| read_pointer(&value, &json_pointer(path))).unwrap_or_default()
}

fn read_pointer(value: &Value, pointer: &str) -> String {
    value.pointer(pointer).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn write_path(draft: Signal<Option<Value>>, path: &[&str], text: &str) {
    let Some(mut config) = draft.get_untracked() else { return };
    put_string(&mut config, path, text);
    draft.set(Some(config));
}

fn write_pointer(draft: Signal<Option<Value>>, pointer: &str, text: &str) {
    let owned = pointer
        .trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let refs = owned.iter().map(String::as_str).collect::<Vec<_>>();
    write_path(draft, &refs, text);
}

fn prepare(config: &Value) -> Value {
    let mut config = if config.is_object() { config.clone() } else { json!({}) };
    let provider = read_pointer(&config, "/provider");
    let provider =
        if provider.trim().is_empty() { "none".to_owned() } else { provider.trim().to_owned() };
    let app_id = read_pointer(&config, "/qqBot/appId").trim().to_owned();
    let secret = read_pointer(&config, "/qqBot/clientSecret").trim().to_owned();
    let endpoint = read_pointer(&config, "/endpoint").trim().to_owned();
    let token = read_pointer(&config, "/token").trim().to_owned();
    let sign = read_pointer(&config, "/secret").trim().to_owned();
    put_string(&mut config, &["provider"], &provider);
    put_string(&mut config, &["qqBot", "appId"], &app_id);
    put_string(&mut config, &["qqBot", "clientSecret"], &secret);
    put_string(&mut config, &["endpoint"], &endpoint);
    put_string(&mut config, &["token"], &token);
    put_string(&mut config, &["secret"], &sign);
    config
}

fn can_test_provider(provider: &str, endpoint: &str, token: &str, bound: bool) -> bool {
    match provider {
        "qq_bot" => bound,
        "ding_talk" => !endpoint.trim().is_empty() || !token.trim().is_empty(),
        _ => false,
    }
}

fn put_string(value: &mut Value, path: &[&str], text: &str) {
    let Some((key, rest)) = path.split_first() else { return };
    let Some(object) = value.as_object_mut() else { return };
    if rest.is_empty() {
        object.insert((*key).to_owned(), Value::String(text.to_owned()));
        return;
    }
    let child = object.entry((*key).to_owned()).or_insert_with(|| json!({}));
    if !child.is_object() {
        *child = json!({});
    }
    put_string(child, rest, text);
}

fn json_pointer(path: &[&str]) -> String {
    format!("/{}", path.join("/"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{can_test_provider, prepare};

    #[test]
    fn test_button_follows_provider_setup() {
        assert!(!can_test_provider("none", "", "", false));
        assert!(!can_test_provider("qq_bot", "", "", false));
        assert!(can_test_provider("qq_bot", "", "", true));
        assert!(!can_test_provider("ding_talk", "  ", "", false));
        assert!(can_test_provider("ding_talk", "https://hook", "", false));
        assert!(can_test_provider("ding_talk", "", "token", false));
        assert!(!can_test_provider("wechat_bot", "https://hook", "token", true));
    }

    #[test]
    fn prepare_trims_credentials_and_keeps_the_message() {
        let prepared = prepare(&json!({
            "provider": " ding_talk ",
            "title": "账号下线提醒",
            "msg": "账号下线",
            "endpoint": " https://hook ",
            "token": " token ",
            "qqBot": { "appId": " app ", "clientSecret": " secret " }
        }));
        assert_eq!(prepared["provider"], "ding_talk");
        assert_eq!(prepared["title"], "账号下线提醒");
        assert_eq!(prepared["endpoint"], "https://hook");
        assert_eq!(prepared["token"], "token");
        assert_eq!(prepared["qqBot"]["appId"], "app");
        assert_eq!(prepared["qqBot"]["clientSecret"], "secret");
        assert_eq!(prepare(&json!([]))["provider"], "none");
    }
}
