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
    protect: String,
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
}

#[derive(Clone, PartialEq, Eq)]
struct FoodLine {
    id: i64,
    name: String,
    count: i64,
}

pub(crate) fn section(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let summary = signal(None::<String>);
    let food_counts = signal(Vec::<(i64, i64)>::new());
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
            move || pet_body(account_id, pending, failure, summary, pets, food_counts),
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
) -> impl IntoElement {
    view()
        .gap_3()
        .child(
            text(move || {
                let protect = pets.get().map(|view| view.protect).unwrap_or_default();
                SharedString::from(format!("{} {protect}", text_key("pets.protect")))
            })
            .text_sm(),
        )
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
                .child(when(
                    move || active,
                    move || badge(move || text_key("pets.active"), BadgeTone::Success),
                ))
                .child(when(
                    move || owned && !active,
                    move || badge(move || text_key("pets.owned"), BadgeTone::Neutral),
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
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(name).text_sm().flex_1())
        .child(text(SharedString::from(owned.to_string())).text_sm())
        .child(food_stepper(food_id, owned, pets, counts, pending))
        .child(
            button(move || text_key("bag.use"), ButtonVariant::Secondary)
                .disabled(move || owned <= 0 || pending.get().is_some())
                .on_press(move || {
                    let count =
                        food_chosen(counts, food_id, current_food_max(pets, food_id, owned));
                    run_food(food_id, count, account_id, pending, failure, summary, pets);
                }),
        )
}

fn current_food_max(pets: Resource<PetView, String>, id: i64, owned: i64) -> i64 {
    let (protect, max_protect) = pets
        .get()
        .map(|view| (view.protect_seconds, view.max_protect_seconds))
        .unwrap_or((0, 30 * SECONDS_PER_DAY));
    food_use_max(owned, food_duration(id).unwrap_or(0), protect, max_protect)
}

fn food_chosen(counts: Signal<Vec<(i64, i64)>>, id: i64, max_count: i64) -> i64 {
    let stored = counts
        .get()
        .into_iter()
        .find(|(food_id, _)| *food_id == id)
        .map(|(_, count)| count)
        .unwrap_or(1);
    stored.clamp(1, max_count.max(1))
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
        protect: protect_label(protect_seconds),
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

fn food_use_max(owned: i64, duration: i64, protect: i64, max_protect: i64) -> i64 {
    if duration <= 0 {
        return owned.max(1);
    }
    let room = max_protect.saturating_sub(protect.max(0)).max(0) / duration;
    room.min(owned.max(0)).max(1)
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
    })
}

fn food_line_from(value: &Value) -> Option<FoodLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    Some(FoodLine {
        id,
        name: value.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        count: value.get("count").and_then(Value::as_i64).unwrap_or(0),
    })
}

fn protect_label(seconds: i64) -> String {
    if seconds <= 0 {
        return "0".to_owned();
    }
    let days = seconds / SECONDS_PER_DAY;
    let hours = (seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
    if days > 0 && hours > 0 {
        format!("{days}d {hours}h")
    } else if days > 0 {
        format!("{days}d")
    } else {
        format!("{hours}h")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{food_use_max, pet_view, protect_label};

    #[test]
    fn protect_time_uses_days_and_hours() {
        assert_eq!(protect_label(0), "0");
        assert_eq!(protect_label(86_400), "1d");
        assert_eq!(protect_label(90_000), "1d 1h");
    }

    #[test]
    fn snapshot_keeps_owned_dogs_and_food() {
        let view = pet_view(&json!({
            "protectDuration": 3600,
            "pendingGiftCount": 2,
            "dogs": [
                { "id": 0, "name": "drop" },
                { "id": 9, "name": "豆豆", "level": 3, "owned": true, "active": true, "skillDescription": "护田" }
            ],
            "foods": [{ "id": 90004, "name": "小袋", "count": 4 }]
        }));
        assert_eq!(view.protect, "1h");
        assert_eq!(view.pending_gifts, 2);
        assert_eq!(view.dogs.len(), 1);
        assert!(view.dogs[0].active);
        assert_eq!(view.foods[0].count, 4);
        assert_eq!(view.protect_seconds, 3600);
        assert_eq!(food_use_max(4, 86_400, 3600, view.max_protect_seconds), 4);
        assert_eq!(food_use_max(10, 5 * 86_400, 28 * 86_400, 30 * 86_400), 1);
        assert_eq!(food_use_max(10, 86_400, 0, 2 * 86_400), 2);
    }
}
