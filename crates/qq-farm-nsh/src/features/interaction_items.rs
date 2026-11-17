use nsh_core::prelude::*;
use serde_json::Value;

use super::account_gate::gate_notice;
use super::accounts::{account_is_running, failure_line, AccountRow};
use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, field, notice, ButtonVariant};

const LIGHTNING_BOTTLE: i64 = 5003;
const THUNDER_BOTTLE: i64 = 5004;
const LIGHTNING_MUTANT: i64 = 12;
const TIANGONG_RARITY_LOW: i64 = 4;
const TIANGONG_RARITY_HIGH: i64 = 5;
const SINGLE_PLOT: i64 = 1;

#[derive(Clone, PartialEq, Eq)]
struct ItemLine {
    id: i64,
    name: String,
    count: i64,
    sellable: i64,
    description: String,
}

#[derive(Clone, Copy)]
pub(crate) struct InteractionPick {
    selected_item: Signal<Option<i64>>,
    selected_lands: Signal<Vec<i64>>,
    used_lands: Signal<Vec<i64>>,
    items: Resource<Vec<ItemLine>, String>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LandTarget {
    pub unlocked: bool,
    pub occupied: bool,
    pub plant_name: String,
    pub status: String,
    pub phase_name: String,
    pub rarity: i64,
    pub plant_size: i64,
    pub mutants: Vec<i64>,
    pub effects: Vec<i64>,
}

#[derive(Clone)]
pub(crate) struct LandChoice {
    pub id: i64,
    pub target: LandTarget,
}

enum LandToggle {
    Updated(Vec<i64>),
    Shortage,
}

pub(crate) fn section(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> impl IntoElement {
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let selected = signal(None::<i64>);
    let lands = signal(String::new());
    let items = resource(
        move || running_account(account_id, rows),
        |id, _| async move {
            let context = session::session().context.clone();
            let value = session::call(async move {
                qq_farm_app::friend::farm_interaction_items(&context, &id).await
            })
            .await?;
            Ok(item_lines(&value))
        },
    );
    view()
        .gap_3()
        .child(failure_line(failure))
        .child(gate_notice(account_id, rows, "farm.needRunning"))
        .child(when(
            move || running(account_id, rows) && items.loading() && items.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || running(account_id, rows) && items.error().is_some(),
            move || load_error(items),
        ))
        .child(when(
            move || running(account_id, rows) && items.get().is_some(),
            move || item_body(account_id, pending, failure, selected, lands, items),
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

fn load_error(items: Resource<Vec<ItemLine>, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            items.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || items.refetch()),
        )
}

fn item_body(
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    selected: Signal<Option<i64>>,
    lands: Signal<String>,
    items: Resource<Vec<ItemLine>, String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || items.refetch()),
        )
        .child(when(
            move || items.get().is_some_and(|rows| rows.is_empty()),
            move || notice(move || text_key("interaction.empty")),
        ))
        .child(each(
            move || items.get().unwrap_or_default(),
            |item: &ItemLine| item.id.to_string(),
            move |item, _| item_row(item.get_untracked(), selected, items),
        ))
        .child(when(
            move || items.get().is_some_and(|rows| !rows.is_empty()),
            move || {
                view()
                    .gap_2()
                    .child(field(
                        move || lands.get(),
                        move || text_key("interaction.lands").to_string(),
                        move |text: &str| lands.set(text.to_owned()),
                    ))
                    .child(
                        button(move || text_key("interaction.use"), ButtonVariant::Primary)
                            .disabled(move || {
                                pending.get().is_some() || active_id(items, selected).is_none()
                            })
                            .on_press(move || {
                                use_item(account_id, pending, failure, selected, lands, items)
                            }),
                    )
            },
        ))
}

fn item_row(
    item: ItemLine,
    selected: Signal<Option<i64>>,
    items: Resource<Vec<ItemLine>, String>,
) -> View {
    let id = item.id;
    let name = SharedString::from(item.name);
    let count = item.count;
    let sellable = item.sellable;
    let description = SharedString::from(item.description);
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(
            button(move || name.clone(), ButtonVariant::Quiet)
                .selected(move || active_id(items, selected) == Some(id))
                .on_press(move || selected.set(Some(id))),
        )
        .child(text(SharedString::from(format!("x{count}"))).text_sm())
        .child(when(
            {
                let description = description.clone();
                move || !description.is_empty()
            },
            {
                let description = description.clone();
                move || text(description.clone()).text_sm().flex_1()
            },
        ))
        .child(when(
            move || sellable > 0,
            move || {
                text(move || {
                    SharedString::from(format!("{} {sellable}", text_key("interaction.sellable")))
                })
                .text_sm()
            },
        ))
}

fn active_id(items: Resource<Vec<ItemLine>, String>, selected: Signal<Option<i64>>) -> Option<i64> {
    let rows = items.get().unwrap_or_default();
    selected
        .get()
        .filter(|id| rows.iter().any(|item| item.id == *id))
        .or_else(|| rows.first().map(|item| item.id))
}

fn use_item(
    account_id: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    selected: Signal<Option<i64>>,
    lands: Signal<String>,
    items: Resource<Vec<ItemLine>, String>,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = account_id.get_untracked() else { return };
    let Some(item_id) = active_id(items, selected) else { return };
    let land_ids = parse_land_ids(&lands.get_untracked());
    if land_ids.is_empty() {
        failure.set(Some(text_key("interaction.needLands").to_string()));
        return;
    }
    pending.set(Some(format!("use:{item_id}")));
    failure.set(None);
    confirm::confirm(
        "interaction.confirm.title",
        "interaction.confirm.body",
        "use",
        "interaction.use",
        move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            spawn(async move {
                let context = session::session().context.clone();
                let result = session::call(async move {
                    qq_farm_app::friend::farm_interaction_items_use(
                        &context, &id, item_id, land_ids,
                    )
                    .await
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(None);
                items.refetch();
            });
        },
    );
}

pub(crate) fn interaction_pick(
    account_id: Signal<Option<String>>,
    rows: Resource<Vec<AccountRow>, String>,
) -> InteractionPick {
    InteractionPick {
        selected_item: signal(None),
        selected_lands: signal(Vec::new()),
        used_lands: signal(Vec::new()),
        items: resource(
            move || running_account(account_id, rows),
            |id, _| async move {
                let context = session::session().context.clone();
                let value = session::call(async move {
                    qq_farm_app::friend::friend_interaction_items(&context, &id).await
                })
                .await?;
                Ok(item_lines(&value))
            },
        ),
    }
}

pub(crate) fn expanded_tools(
    account_id: Signal<Option<String>>,
    gid: i64,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    lands: Resource<Value, String>,
    pick: InteractionPick,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(
            text(move || text_key("interaction.friendTitle"))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(when(
            move || pick.items.get().is_some_and(|rows| rows.is_empty()),
            move || notice(move || text_key("interaction.friendEmpty")),
        ))
        .child(each(
            move || pick.items.get().unwrap_or_default(),
            |item: &ItemLine| item.id.to_string(),
            move |item, _| friend_item_row(item.get_untracked(), pick),
        ))
        .child(when(
            move || chosen_item(pick).is_some(),
            move || {
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        button(move || text_key("interaction.selectAll"), ButtonVariant::Secondary)
                            .disabled(move || pending.get().is_some())
                            .on_press(move || {
                                let Some(item) = chosen_item(pick) else { return };
                                let choices = lands
                                    .get()
                                    .map(|value| choices_from_lands(&value))
                                    .unwrap_or_default();
                                pick.selected_lands.set(select_available(
                                    &choices,
                                    item.id,
                                    item.count,
                                    &pick.used_lands.get_untracked(),
                                ));
                            }),
                    )
                    .child(
                        button(move || text_key("interaction.clear"), ButtonVariant::Secondary)
                            .disabled(move || {
                                pending.get().is_some() || pick.selected_lands.get().is_empty()
                            })
                            .on_press(move || pick.selected_lands.set(Vec::new())),
                    )
                    .child(
                        button(
                            move || {
                                let count = pick.selected_lands.get().len();
                                SharedString::from(
                                    text_key("interaction.useSelected")
                                        .to_string()
                                        .replace("{count}", &count.to_string()),
                                )
                            },
                            ButtonVariant::Primary,
                        )
                        .disabled(move || {
                            pending.get().is_some() || pick.selected_lands.get().is_empty()
                        })
                        .on_press(move || {
                            use_on_friend(account_id, gid, pending, failure, lands, pick)
                        }),
                    )
            },
        ))
}

impl InteractionPick {
    pub(crate) fn reset_when_scope_changes(self, open_lands: Signal<Option<i64>>) {
        let seen = signal((None::<i64>, None::<i64>));
        effect(move || {
            let current = (open_lands.get(), self.selected_item.get());
            if seen.get_untracked() != current {
                seen.set(current);
                self.selected_lands.set(Vec::new());
                self.used_lands.set(Vec::new());
            }
        });
    }
}

pub(crate) fn choosing(pick: InteractionPick) -> bool {
    chosen_item(pick).is_some()
}

pub(crate) fn selection_label(
    pick: InteractionPick,
    target: &LandTarget,
    land_id: i64,
) -> Option<&'static str> {
    let item = chosen_item(pick)?;
    let reason = target_reason(item.id, target);
    if reason == Some("interaction.active") {
        return reason;
    }
    if pick.used_lands.get().contains(&land_id) {
        return Some("interaction.usedNow");
    }
    reason
}

pub(crate) fn press_land(
    pick: InteractionPick,
    target: &LandTarget,
    land_id: i64,
    failure: Signal<Option<String>>,
) {
    let Some(item) = chosen_item(pick) else { return };
    let blocked = selection_label(pick, target, land_id).is_some() || item.count < 1;
    match toggle_land(&pick.selected_lands.get_untracked(), land_id, item.count, blocked) {
        LandToggle::Shortage => {
            failure.set(Some(text_key("interaction.notEnough").to_string()));
        }
        LandToggle::Updated(next) => {
            failure.set(None);
            pick.selected_lands.set(next);
        }
    }
}

pub(crate) fn land_selected(pick: InteractionPick, land_id: i64) -> bool {
    pick.selected_lands.get().contains(&land_id)
}

fn friend_item_row(item: ItemLine, pick: InteractionPick) -> View {
    let id = item.id;
    let name = SharedString::from(item.name);
    let count = item.count;
    let sellable = item.sellable;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(
            button(move || name.clone(), ButtonVariant::Quiet)
                .selected(move || pick.selected_item.get() == Some(id))
                .on_press(move || {
                    if pick.selected_item.get_untracked() == Some(id) {
                        pick.selected_item.set(None);
                    } else {
                        pick.selected_item.set(Some(id));
                    }
                }),
        )
        .child(text(SharedString::from(format!("x{count}"))).text_sm())
        .child(when(
            move || sellable > 0,
            move || {
                text(move || {
                    SharedString::from(format!("{} {sellable}", text_key("interaction.sellable")))
                })
                .text_sm()
            },
        ))
}

fn chosen_item(pick: InteractionPick) -> Option<ItemLine> {
    let id = pick.selected_item.get()?;
    pick.items.get()?.into_iter().find(|item| item.id == id)
}

fn use_on_friend(
    account_id: Signal<Option<String>>,
    gid: i64,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    lands: Resource<Value, String>,
    pick: InteractionPick,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = account_id.get_untracked() else { return };
    let Some(item) = chosen_item(pick) else { return };
    let land_ids = pick.selected_lands.get_untracked();
    if land_ids.is_empty() {
        failure.set(Some(text_key("interaction.needPick").to_string()));
        return;
    }
    let item_id = item.id;
    pending.set(Some(format!("friend-use:{item_id}:{gid}")));
    failure.set(None);
    confirm::confirm(
        "interaction.confirmFriend.title",
        "interaction.confirmFriend.body",
        "use",
        "interaction.use",
        move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            spawn(async move {
                let context = session::session().context.clone();
                let result = session::call(async move {
                    qq_farm_app::friend::friend_interaction_items_use(
                        &context, &id, gid, item_id, land_ids,
                    )
                    .await
                })
                .await;
                match result {
                    Ok(value) => remember_use(&value, pick, failure),
                    Err(message) => failure.set(Some(message)),
                }
                pending.set(None);
                pick.items.refetch();
                lands.refetch();
            });
        },
    );
}

fn remember_use(value: &Value, pick: InteractionPick, failure: Signal<Option<String>>) {
    let used = used_land_ids(value);
    let mut remembered = pick.used_lands.get_untracked();
    for id in &used {
        if !remembered.contains(id) {
            remembered.push(*id);
        }
    }
    remembered.sort_unstable();
    pick.used_lands.set(remembered.clone());
    let selected = pick
        .selected_lands
        .get_untracked()
        .into_iter()
        .filter(|id| !remembered.contains(id))
        .collect();
    pick.selected_lands.set(selected);
    let failed = value.get("failureCount").and_then(Value::as_i64).unwrap_or(0);
    if failed > 0 {
        let message = value.get("message").and_then(Value::as_str).unwrap_or("").trim().to_owned();
        if !message.is_empty() {
            failure.set(Some(message));
        }
    }
}

fn item_lines(value: &Value) -> Vec<ItemLine> {
    value
        .get("items")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(item_line).collect())
        .unwrap_or_default()
}

fn item_line(value: &Value) -> Option<ItemLine> {
    let id = value
        .get("itemId")
        .and_then(|id| id.as_i64().or_else(|| id.as_str().and_then(|text| text.parse().ok())))
        .filter(|id| *id > 0)?;
    let name = value.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
    Some(ItemLine {
        id,
        name: if name.is_empty() { format!("#{id}") } else { name },
        count: value.get("count").and_then(Value::as_i64).unwrap_or(0),
        sellable: value.get("saleConditionSatisfiedCount").and_then(Value::as_i64).unwrap_or(0),
        description: value.get("description").and_then(Value::as_str).unwrap_or("").to_owned(),
    })
}

pub(crate) fn land_target(value: &Value) -> LandTarget {
    LandTarget {
        unlocked: value.get("unlocked").and_then(Value::as_bool).unwrap_or(false),
        occupied: value.get("occupiedByMaster").and_then(Value::as_bool).unwrap_or(false),
        plant_name: value.get("plantName").and_then(Value::as_str).unwrap_or("").trim().to_owned(),
        status: value.get("status").and_then(Value::as_str).unwrap_or("").to_owned(),
        phase_name: value.get("phaseName").and_then(Value::as_str).unwrap_or("").to_owned(),
        rarity: value.get("rarity").and_then(Value::as_i64).unwrap_or(0),
        plant_size: value
            .get("plantSize")
            .and_then(Value::as_i64)
            .filter(|size| *size > 0)
            .unwrap_or(1),
        mutants: number_list(value.get("mutantConfigIds")),
        effects: effect_ids(value.get("interactionEffects")),
    }
}

fn choices_from_lands(value: &Value) -> Vec<LandChoice> {
    value
        .get("lands")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|land| {
                    let target = land_target(land);
                    if target.occupied {
                        return None;
                    }
                    let id = land.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
                    Some(LandChoice { id, target })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn number_list(value: Option<&Value>) -> Vec<i64> {
    value
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|item| {
                    item.as_i64().or_else(|| item.as_str().and_then(|text| text.parse().ok()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn effect_ids(value: Option<&Value>) -> Vec<i64> {
    value
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|effect| {
                    effect.get("itemId").and_then(|id| {
                        id.as_i64().or_else(|| id.as_str().and_then(|text| text.parse().ok()))
                    })
                })
                .filter(|id| *id > 0)
                .collect()
        })
        .unwrap_or_default()
}

fn target_reason(item_id: i64, land: &LandTarget) -> Option<&'static str> {
    if !land.unlocked || land.occupied || land.plant_name.trim().is_empty() {
        return Some("interaction.unavailable");
    }
    if matches!(land.status.as_str(), "locked" | "empty") {
        return Some("interaction.unavailable");
    }
    if land.status == "dead" {
        return Some("interaction.dead");
    }
    if matches!(land.status.as_str(), "harvestable" | "stealable" | "harvested") {
        return Some("interaction.mature");
    }
    if land.effects.contains(&item_id) {
        return Some("interaction.active");
    }
    let bottle = item_id == LIGHTNING_BOTTLE || item_id == THUNDER_BOTTLE;
    if bottle && land.phase_name.contains("种子") {
        return Some("interaction.seed");
    }
    if bottle && (land.rarity == TIANGONG_RARITY_LOW || land.rarity == TIANGONG_RARITY_HIGH) {
        return Some("interaction.tiangong");
    }
    if item_id == LIGHTNING_BOTTLE && land.plant_size != SINGLE_PLOT {
        return Some("interaction.size");
    }
    if item_id == LIGHTNING_BOTTLE && land.mutants.contains(&LIGHTNING_MUTANT) {
        return Some("interaction.lightning");
    }
    None
}

fn toggle_land(selected: &[i64], land_id: i64, count: i64, blocked: bool) -> LandToggle {
    if blocked || land_id <= 0 {
        return LandToggle::Updated(selected.to_vec());
    }
    let mut next: Vec<i64> = selected.iter().copied().filter(|id| *id != land_id).collect();
    if next.len() != selected.len() {
        return LandToggle::Updated(next);
    }
    if count < 1 || i64::try_from(next.len()).unwrap_or(i64::MAX) >= count {
        return LandToggle::Shortage;
    }
    next.push(land_id);
    next.sort_unstable();
    LandToggle::Updated(next)
}

fn select_available(lands: &[LandChoice], item_id: i64, count: i64, used: &[i64]) -> Vec<i64> {
    if count < 1 {
        return Vec::new();
    }
    let mut ids: Vec<i64> = lands
        .iter()
        .filter(|land| land.id > 0)
        .filter(|land| target_reason(item_id, &land.target).is_none())
        .filter(|land| !used.contains(&land.id))
        .map(|land| land.id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let limit = usize::try_from(count).unwrap_or(usize::MAX);
    ids.truncate(limit);
    ids
}

fn used_land_ids(value: &Value) -> Vec<i64> {
    value
        .get("usedLandIds")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|id| {
                    id.as_i64().or_else(|| id.as_str().and_then(|text| text.parse().ok()))
                })
                .filter(|id| *id > 0)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_land_ids(value: &str) -> Vec<i64> {
    value
        .split(|ch: char| ch == ',' || ch == '，' || ch.is_whitespace())
        .filter_map(|part| part.trim().parse::<i64>().ok())
        .filter(|id| *id > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        item_lines, parse_land_ids, select_available, target_reason, toggle_land, used_land_ids,
        LandChoice, LandTarget, LandToggle,
    };

    #[test]
    fn lands_and_items_follow_the_desktop_picker() {
        assert_eq!(parse_land_ids("1， 2,3\n4\t5 -1 0 bad"), vec![1, 2, 3, 4, 5]);
        let lines = item_lines(&json!({
            "items": [
                { "itemId": "44", "name": "雨水瓶", "count": 2, "saleConditionSatisfiedCount": 1 },
                { "itemId": 0, "name": "drop" },
                { "itemId": 55, "count": 3 }
            ]
        }));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].id, 44);
        assert_eq!(lines[0].sellable, 1);
        assert_eq!(lines[1].name, "#55");
    }

    fn growing() -> LandTarget {
        LandTarget {
            unlocked: true,
            occupied: false,
            plant_name: "萝卜".to_owned(),
            status: "growing".to_owned(),
            phase_name: "发芽".to_owned(),
            rarity: 1,
            plant_size: 1,
            mutants: Vec::new(),
            effects: Vec::new(),
        }
    }

    fn choice(id: i64, target: LandTarget) -> LandChoice {
        LandChoice { id, target }
    }

    #[test]
    fn land_rules_reject_locked_mature_and_bottle_limits() {
        assert_eq!(target_reason(1, &growing()), None);
        let mut locked = growing();
        locked.unlocked = false;
        assert_eq!(target_reason(1, &locked), Some("interaction.unavailable"));
        let mut dead = growing();
        dead.status = "dead".to_owned();
        assert_eq!(target_reason(1, &dead), Some("interaction.dead"));
        let mut ripe = growing();
        ripe.status = "stealable".to_owned();
        assert_eq!(target_reason(1, &ripe), Some("interaction.mature"));
        let mut active = growing();
        active.effects = vec![9];
        assert_eq!(target_reason(9, &active), Some("interaction.active"));
        let mut seed = growing();
        seed.phase_name = "种子期".to_owned();
        assert_eq!(target_reason(5003, &seed), Some("interaction.seed"));
        let mut tiangong = growing();
        tiangong.rarity = 4;
        assert_eq!(target_reason(5004, &tiangong), Some("interaction.tiangong"));
        let mut wide = growing();
        wide.plant_size = 2;
        assert_eq!(target_reason(5003, &wide), Some("interaction.size"));
        assert_eq!(target_reason(5004, &wide), None);
        let mut lightning = growing();
        lightning.mutants = vec![12];
        assert_eq!(target_reason(5003, &lightning), Some("interaction.lightning"));
    }

    #[test]
    fn selection_keeps_available_lands_in_id_order_and_stops_at_the_count() {
        let lands = vec![
            choice(3, growing()),
            choice(1, {
                let mut land = growing();
                land.status = "empty".to_owned();
                land
            }),
            choice(2, growing()),
            choice(4, growing()),
        ];
        assert_eq!(select_available(&lands, 1, 2, &[2]), vec![3, 4]);
        assert!(select_available(&lands, 1, 0, &[]).is_empty());
        assert!(
            matches!(toggle_land(&[3], 4, 2, false), LandToggle::Updated(ids) if ids == vec![3, 4])
        );
        assert!(
            matches!(toggle_land(&[3, 4], 3, 2, false), LandToggle::Updated(ids) if ids == vec![4])
        );
        assert!(matches!(toggle_land(&[3, 4], 5, 2, false), LandToggle::Shortage));
        assert!(
            matches!(toggle_land(&[3], 4, 2, true), LandToggle::Updated(ids) if ids == vec![3])
        );
        assert_eq!(used_land_ids(&json!({ "usedLandIds": ["2", 4, "0", "bad"] })), vec![2, 4]);
    }
}
