use nsh_core::prelude::*;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, notice, BadgeTone, ButtonVariant};

const SECONDS_PER_HOUR: i64 = 3600;
const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;

#[derive(Clone, PartialEq, Eq)]
struct PetView {
    protect_seconds: i64,
    max_protect_seconds: i64,
    pending_gifts: i64,
    dogs: Vec<DogLine>,
    foods: Vec<FoodLine>,
}

#[derive(Clone, PartialEq, Eq)]
struct DogLine {
    id: i64,
    name: String,
    level: i64,
    owned: bool,
    active: bool,
    activatable: bool,
    skill: String,
    status_key: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
struct FoodLine {
    id: i64,
    name: String,
    count: i64,
    span_key: &'static str,
    span_count: i64,
}

#[derive(Clone, PartialEq, Eq)]
struct ProtectBoard {
    total: i64,
    logs: Vec<ProtectLog>,
}

#[derive(Clone, PartialEq, Eq)]
struct ProtectLog {
    id: String,
    dog: String,
    friend: String,
    stolen: i64,
    gold: i64,
    when: String,
}

pub(crate) fn section(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let summary = signal(None::<String>);
    let food_counts = signal(Vec::<(i64, i64)>::new());
    let logs_open = signal(false);
    let logs = resource(
        move || {
            if !logs_open.get() {
                return None;
            }
            running_account(account_id, rows)
        },
        |id, _| async move { load_protect_logs(id).await },
    );
    let pets = resource(
        move || running_account(account_id, rows),
        |id, _| async move {
            let context = session::session().context.clone();
            let value =
                session::call(async move { qq_farm_app::pets::pet_info(&context, &id).await })
                    .await?;
            Ok(pet_view(&value))
        },
    );
    view()
        .gap_3()
        .child(failure_line(failure))
        .child(when(
            move || summary.get().is_some(),
            move || notice(move || summary.get().map(SharedString::from).unwrap_or_default()),
        ))
        .child(gate_notice(account_id, rows, "farm.needRunning"))
        .child(when(
            move || running(account_id, rows) && pets.loading() && pets.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || running(account_id, rows) && pets.error().is_some(),
            move || load_error(pets),
        ))
        .child(when(
            move || running(account_id, rows) && pets.get().is_some(),
            move || {
                pet_body(account_id, pending, failure, summary, pets, food_counts, logs, logs_open)
            },
        ))
}

fn running(account_id: Signal<Option<String>>, rows: Resource<Vec<AccountRow>, String>) -> bool {
    account_id.get().zip(rows.get()).is_some_and(|(id, rows)| account_is_running(&rows, &id))
}

fn running_account(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> Option<String> {
    running(account_id, rows).then(|| account_id.get()).flatten()
}

fn load_error(pets: Resource<PetView, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || pets.error().map(SharedString::from).unwrap_or_default()))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || pets.refetch()),
        )
}

fn pet_body(
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    pets: Resource<PetView, String>,
    food_counts: Signal<Vec<(i64, i64)>>,
    logs: Resource<ProtectBoard, String>,
    logs_open: Signal<bool>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(
            text(move || {
                let seconds = pets.get().map(|view| view.protect_seconds).unwrap_or(0);
                SharedString::from(format!(
                    "{} {}",
                    text_key("pets.protect"),
                    duration_text(seconds)
                ))
            })
            .text_sm(),
        )
        .child(when(
            move || {
                let view = pets.get();
                view.as_ref().is_some_and(|view| {
                    view.protect_seconds < SECONDS_PER_DAY && view.dogs.iter().any(|dog| dog.active)
                })
            },
            move || notice(move || text_key("pets.protectLow")),
        ))
        .child(when(
            move || pets.get().is_some_and(|view| view.pending_gifts > 0),
            move || {
                let pending_count = pets.get().map(|view| view.pending_gifts).unwrap_or(0);
                button(
                    move || {
                        SharedString::from(format!(
                            "{} {pending_count}",
                            text_key("pets.claimGifts")
                        ))
                    },
                    ButtonVariant::Primary,
                )
                .disabled(move || pending.get().is_some())
                .on_press(move || run_pet("claim", 0, account_id, pending, failure, summary, pets))
            },
        ))
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || pets.refetch()),
        )
        .child(
            button(move || text_key("pets.logs"), ButtonVariant::Quiet)
                .selected(move || logs_open.get())
                .disabled(move || pending.get().is_some())
                .on_press(move || logs_open.set(!logs_open.get_untracked())),
        )
        .child(when(move || logs_open.get(), move || protect_log_panel(logs)))
        .child(when(
            move || pets.get().is_some_and(|view| view.dogs.is_empty()),
            move || notice(move || text_key("pets.none")),
        ))
        .child(each(
            move || pets.get().map(|view| view.dogs).unwrap_or_default(),
            |dog: &DogLine| dog.id.to_string(),
            move |dog, _| {
                dog_line(dog.get_untracked(), account_id, pending, failure, summary, pets)
            },
        ))
        .child(text(move || text_key("pets.food")).text_sm().font_weight(FontWeight::SEMIBOLD))
        .child(each(
            move || pets.get().map(|view| view.foods).unwrap_or_default(),
            |food: &FoodLine| food.id.to_string(),
            move |food, _| {
                food_line(
                    food.get_untracked(),
                    account_id,
                    pending,
                    failure,
                    summary,
                    pets,
                    food_counts,
                )
            },
        ))
}

fn dog_line(
    dog: DogLine,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    pets: Resource<PetView, String>,
) -> View {
    let name = SharedString::from(dog.name.clone());
    let skill = SharedString::from(dog.skill.clone());
    let level = dog.level;
    let id = dog.id;
    let active = dog.active;
    let owned = dog.owned;
    let activatable = dog.activatable;
    let status_key = dog.status_key;
    view()
        .gap_2()
        .py_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(text(name).text_sm().font_weight(FontWeight::MEDIUM))
                .child(
                    text(move || {
                        SharedString::from(format!("{} {level}", text_key("friends.level")))
                    })
                    .text_sm(),
                )
                .child(badge(
                    move || text_key(status_key),
                    if active { BadgeTone::Success } else { BadgeTone::Neutral },
                )),
        )
        .child(text(skill).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(when(
                    move || owned && !active,
                    move || {
                        action_button(
                            "pets.deploy",
                            "deploy",
                            id,
                            account_id,
                            pending,
                            failure,
                            summary,
                            pets,
                        )
                    },
                ))
                .child(when(
                    move || active,
                    move || {
                        action_button(
                            "pets.withdraw",
                            "withdraw",
                            id,
                            account_id,
                            pending,
                            failure,
                            summary,
                            pets,
                        )
                    },
                ))
                .child(when(
                    move || activatable,
                    move || {
                        action_button(
                            "pets.activate",
                            "activate",
                            id,
                            account_id,
                            pending,
                            failure,
                            summary,
                            pets,
                        )
                    },
                )),
        )
}

fn food_line(
    food: FoodLine,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    pets: Resource<PetView, String>,
    counts: Signal<Vec<(i64, i64)>>,
) -> View {
    let name = SharedString::from(food.name);
    let owned = food.count;
    let food_id = food.id;
    let span_key = food.span_key;
    let span_count = food.span_count;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(name).text_sm().flex_1())
        .child(when(
            move || !span_key.is_empty(),
            move || {
                text(move || {
                    SharedString::from(
                        text_key(span_key).replace("{count}", &span_count.to_string()),
                    )
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(text(SharedString::from(owned.to_string())).text_sm())
        .child(food_stepper(food_id, owned, pets, counts, pending))
        .child(
            button(move || text_key("bag.use"), ButtonVariant::Secondary)
                .disabled(move || {
                    pending.get().is_some() || current_food_max(pets, food_id, owned) <= 0
                })
                .on_press(move || {
                    let count =
                        food_chosen(counts, food_id, current_food_max(pets, food_id, owned));
                    if count > 0 {
                        run_food(food_id, count, account_id, pending, failure, summary, pets);
                    }
                }),
        )
        .child(when(
            move || owned > 0 && current_food_max(pets, food_id, owned) <= 0,
            move || {
                text(move || text_key(food_block_key(pets)))
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
}

fn current_food_max(pets: Resource<PetView, String>, id: i64, owned: i64) -> i64 {
    let Some(view) = pets.get() else {
        return 0;
    };
    let active = view.dogs.iter().any(|dog| dog.active);
    food_use_max(
        active,
        owned,
        food_duration(id).unwrap_or(0),
        view.protect_seconds,
        view.max_protect_seconds,
    )
}

fn food_block_key(pets: Resource<PetView, String>) -> &'static str {
    if pets.get().is_some_and(|view| view.dogs.iter().any(|dog| dog.active)) {
        "pets.feedFull"
    } else {
        "pets.feedNeedDog"
    }
}

fn food_chosen(counts: Signal<Vec<(i64, i64)>>, id: i64, max_count: i64) -> i64 {
    let stored = counts
        .get()
        .into_iter()
        .find(|(food_id, _)| *food_id == id)
        .map(|(_, count)| count)
        .unwrap_or(1);
    if max_count <= 0 {
        return 0;
    }
    stored.clamp(1, max_count)
}

fn food_set(counts: Signal<Vec<(i64, i64)>>, id: i64, count: i64) {
    let mut rows = counts.get_untracked();
    if let Some(row) = rows.iter_mut().find(|(food_id, _)| *food_id == id) {
        row.1 = count;
    } else {
        rows.push((id, count));
    }
    counts.set(rows);
}

fn food_stepper(
    id: i64,
    owned: i64,
    pets: Resource<PetView, String>,
    counts: Signal<Vec<(i64, i64)>>,
    pending: Signal<Option<String>>,
) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Quiet)
                .disabled(move || {
                    let limit = current_food_max(pets, id, owned);
                    pending.get().is_some() || food_chosen(counts, id, limit) <= 1
                })
                .on_press(move || {
                    let limit = current_food_max(pets, id, owned);
                    let next = food_chosen(counts, id, limit).saturating_sub(1).max(1);
                    food_set(counts, id, next);
                }),
        )
        .child(
            text(move || {
                let limit = current_food_max(pets, id, owned);
                SharedString::from(food_chosen(counts, id, limit).to_string())
            })
            .text_sm(),
        )
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Quiet)
                .disabled(move || {
                    let limit = current_food_max(pets, id, owned);
                    pending.get().is_some() || food_chosen(counts, id, limit) >= limit
                })
                .on_press(move || {
                    let limit = current_food_max(pets, id, owned);
                    let next = food_chosen(counts, id, limit).saturating_add(1).min(limit);
                    food_set(counts, id, next);
                }),
        )
}

fn run_food(
    food_id: i64,
    count: i64,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    pets: Resource<PetView, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = account_id.get_untracked() else {
        return;
    };
    pending.set(Some(format!("food:{food_id}")));
    failure.set(None);
    summary.set(None);
    spawn(async move {
        let context = session::session().context.clone();
        let result = session::call(async move {
            qq_farm_app::pets::pet_food_use(&context, &id, food_id, count, 0).await
        })
        .await;
        match result {
            Ok(_) => summary.set(Some(text_key("bag.done").to_string())),
            Err(message) => failure.set(Some(message)),
        }
        pending.set(None);
        pets.refetch();
    });
}

fn action_button(
    label_key: &'static str,
    action: &'static str,
    target: i64,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    pets: Resource<PetView, String>,
) -> nsh_base::ButtonElement {
    let key = format!("{action}:{target}");
    button(
        move || {
            if pending.get().as_deref() == Some(key.as_str()) {
                text_key("farm.working")
            } else {
                text_key(label_key)
            }
        },
        ButtonVariant::Secondary,
    )
    .disabled(move || pending.get().is_some())
    .on_press(move || run_pet(action, target, account_id, pending, failure, summary, pets))
}

fn run_pet(
    action: &'static str,
    target: i64,
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    summary: Signal<Option<String>>,
    pets: Resource<PetView, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = account_id.get_untracked() else {
        return;
    };
    pending.set(Some(format!("{action}:{target}")));
    failure.set(None);
    summary.set(None);
    spawn(async move {
        let context = session::session().context.clone();
        let result =
            session::call(async move { perform_pet(&context, &id, action, target).await }).await;
        match result {
            Ok(_) => summary.set(Some(text_key("bag.done").to_string())),
            Err(message) => failure.set(Some(message)),
        }
        pending.set(None);
        pets.refetch();
    });
}

async fn perform_pet(
    context: &qq_farm_app::AppContext,
    account_id: &str,
    action: &str,
    target: i64,
) -> qq_farm_app::AppResult<Value> {
    match action {
        "deploy" => qq_farm_app::pets::pet_deploy(context, account_id, target).await,
        "withdraw" => qq_farm_app::pets::pet_withdraw(context, account_id).await,
        "activate" => qq_farm_app::pets::pet_activate(context, account_id, target).await,
        "food" => qq_farm_app::pets::pet_food_use(context, account_id, target, 1, 0).await,
        "claim" => qq_farm_app::pets::dog_skill_gifts_claim(context, account_id).await,
        other => Err(qq_farm_app::AppError::BadRequest(format!("unknown pet action: {other}"))),
    }
}

fn pet_view(value: &Value) -> PetView {
    let dogs = value
        .get("dogs")
        .and_then(Value::as_array)
        .map(|dogs| dogs.iter().filter_map(dog_from).collect())
        .unwrap_or_default();
    let foods = value
        .get("foods")
        .and_then(Value::as_array)
        .map(|foods| foods.iter().filter_map(food_line_from).collect())
        .unwrap_or_default();
    let protect_seconds = value.get("protectDuration").and_then(Value::as_i64).unwrap_or(0).max(0);
    let configured = value.get("maxProtectDuration").and_then(Value::as_i64).unwrap_or(0);
    let max_protect_seconds = if configured > 0 { configured } else { 30 * SECONDS_PER_DAY };
    PetView {
        protect_seconds,
        max_protect_seconds: max_protect_seconds.max(protect_seconds),
        pending_gifts: value.get("pendingGiftCount").and_then(Value::as_i64).unwrap_or(0),
        dogs,
        foods,
    }
}

fn food_duration(id: i64) -> Option<i64> {
    match id {
        90004 => Some(SECONDS_PER_DAY),
        90005 => Some(3 * SECONDS_PER_DAY),
        90006 => Some(5 * SECONDS_PER_DAY),
        _ => None,
    }
}

fn food_use_max(active: bool, owned: i64, duration: i64, protect: i64, max_protect: i64) -> i64 {
    if !active || owned <= 0 || duration <= 0 {
        return 0;
    }
    let room = max_protect.saturating_sub(protect.max(0)).max(0) / duration;
    room.min(owned)
}

fn dog_from(value: &Value) -> Option<DogLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).filter(|name| !name.is_empty());
    Some(DogLine {
        id,
        name: name.unwrap_or("").to_owned(),
        level: value.get("level").and_then(Value::as_i64).unwrap_or(0),
        owned: value.get("owned").and_then(Value::as_bool).unwrap_or(false),
        active: value.get("active").and_then(Value::as_bool).unwrap_or(false),
        activatable: value.get("activatable").and_then(Value::as_bool).unwrap_or(false),
        skill: value.get("skillDescription").and_then(Value::as_str).unwrap_or("").to_owned(),
        status_key: dog_status_key(
            value.get("owned").and_then(Value::as_bool).unwrap_or(false),
            value.get("active").and_then(Value::as_bool).unwrap_or(false),
            value.get("activatable").and_then(Value::as_bool).unwrap_or(false),
        ),
    })
}

fn dog_status_key(owned: bool, active: bool, activatable: bool) -> &'static str {
    if active {
        "pets.active"
    } else if owned {
        "pets.owned"
    } else if activatable {
        "pets.ready"
    } else {
        "pets.missing"
    }
}

async fn load_protect_logs(id: String) -> Result<ProtectBoard, String> {
    let context = session::session().context.clone();
    let value =
        session::call(async move { qq_farm_app::pets::pet_protect_logs(&context, &id).await })
            .await?;
    Ok(protect_board(&value))
}

fn protect_log_panel(logs: Resource<ProtectBoard, String>) -> View {
    view()
        .gap_2()
        .child(when(
            move || logs.loading() && logs.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || logs.error().is_some(),
            move || {
                crate::ui::failure(move || logs.error().map(SharedString::from).unwrap_or_default())
            },
        ))
        .child(when(move || logs.get().is_some(), move || protect_log_body(logs)))
}

fn protect_log_body(logs: Resource<ProtectBoard, String>) -> View {
    view()
        .gap_2()
        .child(
            text(move || {
                let total = logs.get().map(|board| board.total).unwrap_or(0);
                SharedString::from(format!("{} {total}", text_key("pets.logsTotal")))
            })
            .text_sm()
            .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(when(
            move || logs.get().is_some_and(|board| board.logs.is_empty()),
            move || notice(move || text_key("pets.logsEmpty")),
        ))
        .child(each(
            move || logs.get().map(|board| board.logs).unwrap_or_default(),
            |log: &ProtectLog| log.id.clone(),
            move |log, _| {
                let log = log.get_untracked();
                let dog = SharedString::from(log.dog);
                let friend = SharedString::from(log.friend);
                let when = SharedString::from(log.when);
                let stolen = log.stolen;
                let gold = log.gold;
                text(move || {
                    let dog = dog.clone();
                    let friend = friend.clone();
                    let when = when.clone();
                    SharedString::from(format!(
                        "{dog} {} {friend} · {} {stolen} · {} {gold} · {when}",
                        text_key("pets.logsStopped"),
                        text_key("pets.logsFruit"),
                        text_key("pets.logsGold")
                    ))
                })
                .text_sm()
            },
        ))
}

fn protect_board(value: &Value) -> ProtectBoard {
    ProtectBoard {
        total: value.get("total").and_then(Value::as_i64).unwrap_or(0).max(0),
        logs: value
            .get("logs")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(protect_log).collect())
            .unwrap_or_default(),
    }
}

fn protect_log(value: &Value) -> Option<ProtectLog> {
    let id = value.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
    let dog = value.get("dogName").and_then(Value::as_str).unwrap_or("");
    let friend = value.get("friendName").and_then(Value::as_str).unwrap_or("");
    let timestamp = value.get("timestamp").and_then(Value::as_i64).unwrap_or(0);
    Some(ProtectLog {
        id: id.to_owned(),
        dog: if dog.is_empty() { "宠物".to_owned() } else { dog.to_owned() },
        friend: if friend.is_empty() { "好友".to_owned() } else { friend.to_owned() },
        stolen: value.get("stolenCount").and_then(Value::as_i64).unwrap_or(0).max(0),
        gold: value.get("protectedGold").and_then(Value::as_i64).unwrap_or(0).max(0),
        when: protect_time(timestamp),
    })
}

fn protect_time(seconds: i64) -> String {
    if seconds <= 0 {
        return String::new();
    }
    chrono::FixedOffset::east_opt(8 * 3600)
        .and_then(|zone| {
            chrono::DateTime::from_timestamp(seconds, 0)
                .map(|time| time.with_timezone(&zone).format("%m-%d %H:%M").to_string())
        })
        .unwrap_or_default()
}

fn food_line_from(value: &Value) -> Option<FoodLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let (span_key, span_count) = food_span(food_duration(id));
    Some(FoodLine {
        id,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        count: value.get("count").and_then(Value::as_i64).unwrap_or(0),
        span_key,
        span_count,
    })
}

fn duration_parts(seconds: i64) -> (i64, i64, i64) {
    let seconds = seconds.max(0);
    let days = seconds / SECONDS_PER_DAY;
    let hours = (seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
    let minutes = (seconds % SECONDS_PER_HOUR) / 60;
    (days, hours, minutes)
}

fn duration_text(seconds: i64) -> String {
    let (days, hours, minutes) = duration_parts(seconds);
    if days > 0 {
        text_key("pets.durationDays")
            .replace("{days}", &days.to_string())
            .replace("{hours}", &hours.to_string())
            .replace("{minutes}", &minutes.to_string())
    } else if hours > 0 {
        text_key("pets.durationHours")
            .replace("{hours}", &hours.to_string())
            .replace("{minutes}", &minutes.to_string())
    } else {
        text_key("pets.durationMinutes").replace("{minutes}", &minutes.to_string())
    }
}

fn food_span(duration: Option<i64>) -> (&'static str, i64) {
    let Some(seconds) = duration.filter(|seconds| *seconds > 0) else {
        return ("", 0);
    };
    let days = (seconds as f64 / SECONDS_PER_DAY as f64).round() as i64;
    if days > 0 {
        ("pets.foodDays", days)
    } else {
        let hours = (seconds as f64 / SECONDS_PER_HOUR as f64).round().max(1.0) as i64;
        ("pets.foodHours", hours)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{dog_status_key, duration_parts, food_span, food_use_max, pet_view, protect_board};

    #[test]
    fn protect_time_uses_days_hours_and_minutes() {
        assert_eq!(duration_parts(0), (0, 0, 0));
        assert_eq!(duration_parts(90), (0, 0, 1));
        assert_eq!(duration_parts(3_600), (0, 1, 0));
        assert_eq!(duration_parts(86_400), (1, 0, 0));
        assert_eq!(duration_parts(90_060), (1, 1, 1));
        assert_eq!(food_span(Some(86_400)), ("pets.foodDays", 1));
        assert_eq!(food_span(Some(5 * 86_400)), ("pets.foodDays", 5));
        assert_eq!(food_span(Some(3_600)), ("pets.foodHours", 1));
        assert_eq!(food_span(None), ("", 0));
    }

    #[test]
    fn snapshot_keeps_owned_dogs_and_food() {
        let view = pet_view(&json!({
            "protectDuration": 3600,
            "pendingGiftCount": 2,
            "dogs": [
                { "id": 0, "name": "drop" },
                { "id": 9, "name": "豆豆", "level": 3, "owned": true, "active": true, "skillDescription": "护田" },
                { "id": 8, "name": "待激活", "owned": false, "activatable": true },
                { "id": 7, "name": "未获得", "owned": false }
            ],
            "foods": [{ "id": 90004, "name": "小袋", "count": 4 }]
        }));
        assert_eq!(view.pending_gifts, 2);
        assert_eq!(view.dogs.len(), 3);
        assert!(view.dogs[0].active);
        assert_eq!(view.dogs[0].status_key, "pets.active");
        assert_eq!(view.dogs[1].status_key, "pets.ready");
        assert_eq!(view.dogs[2].status_key, "pets.missing");
        assert_eq!(dog_status_key(true, false, false), "pets.owned");
        assert_eq!(view.foods[0].count, 4);
        assert_eq!(view.foods[0].span_key, "pets.foodDays");
        assert_eq!(view.foods[0].span_count, 1);
        assert_eq!(view.protect_seconds, 3600);
        assert_eq!(food_use_max(true, 4, 86_400, 3600, view.max_protect_seconds), 4);
        assert_eq!(food_use_max(true, 10, 5 * 86_400, 28 * 86_400, 30 * 86_400), 0);
        assert_eq!(food_use_max(true, 10, 86_400, 0, 2 * 86_400), 2);
        assert_eq!(food_use_max(false, 4, 86_400, 0, 30 * 86_400), 0);
        assert_eq!(food_use_max(true, 4, 0, 0, 30 * 86_400), 0);
        let logs = protect_board(&json!({
            "total": 3,
            "logs": [{
                "id": "7-1700000000-0",
                "dogName": "豆豆",
                "friendName": "阿宁",
                "timestamp": 1700000000,
                "stolenCount": 2,
                "protectedGold": 40
            }, { "id": "" }]
        }));
        assert_eq!(logs.total, 3);
        assert_eq!(logs.logs.len(), 1);
        assert_eq!(logs.logs[0].stolen, 2);
        assert_eq!(logs.logs[0].gold, 40);
        assert_eq!(logs.logs[0].when, "11-15 06:13");
    }
}
