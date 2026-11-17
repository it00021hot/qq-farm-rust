use nsh_core::prelude::*;
use qq_farm_app::dto::{LandRow, LandsPayload};
use serde_json::Value;

use super::account_gate::{account_gate, gate_notice, AccountGate};
use super::accounts::{failure_line, AccountRow};
use super::confirm;
use super::outcome::{op_detail, outcome_line, Outcome};
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, notice, BadgeTone, ButtonVariant};

const SECONDS_PER_MINUTE: i64 = 60;
const FERTILIZE_SELECTION_LIMIT: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FertMode {
    Off,
    Normal,
    Organic,
    Care,
}

#[derive(Clone, Copy)]
struct FertPick {
    mode: Signal<FertMode>,
    lands: Signal<Vec<i64>>,
}

#[derive(Clone, Copy)]
pub(crate) struct FarmPage {
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<AccountRow>, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FarmOp {
    Harvest,
    Water,
    Weed,
    Bug,
    Fertilize,
    Plant,
    Tend,
    Remove,
    Upgrade,
    Unlock,
    Cycle,
}

impl FarmOp {
    const ALL: [Self; 11] = [
        Self::Cycle,
        Self::Harvest,
        Self::Water,
        Self::Weed,
        Self::Bug,
        Self::Fertilize,
        Self::Plant,
        Self::Tend,
        Self::Remove,
        Self::Upgrade,
        Self::Unlock,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::Harvest => "harvest",
            Self::Water => "water",
            Self::Weed => "weed",
            Self::Bug => "bug",
            Self::Fertilize => "fertilize",
            Self::Plant => "plant",
            Self::Tend => "clear",
            Self::Remove => "remove",
            Self::Upgrade => "upgrade",
            Self::Unlock => "unlock",
            Self::Cycle => "cycle",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Harvest => "farm.harvest",
            Self::Water => "farm.water",
            Self::Weed => "farm.weed",
            Self::Bug => "farm.bug",
            Self::Fertilize => "farm.fertilize",
            Self::Plant => "farm.plant",
            Self::Tend => "farm.tend",
            Self::Remove => "farm.remove",
            Self::Upgrade => "farm.upgrade",
            Self::Unlock => "farm.unlock",
            Self::Cycle => "farm.cycle",
        }
    }

    const fn confirms(self) -> bool {
        matches!(self, Self::Remove)
    }

    const fn variant(self) -> ButtonVariant {
        match self {
            Self::Harvest => ButtonVariant::Primary,
            Self::Remove => ButtonVariant::Danger,
            _ => ButtonVariant::Secondary,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FarmPanel {
    Lands,
    Bag,
    Pets,
    Gifts,
    Illustrated,
}

impl FarmPanel {
    const ALL: [Self; 5] = [Self::Lands, Self::Bag, Self::Pets, Self::Gifts, Self::Illustrated];

    const fn id(self) -> &'static str {
        match self {
            Self::Lands => "lands",
            Self::Bag => "bag",
            Self::Pets => "pets",
            Self::Gifts => "gifts",
            Self::Illustrated => "illustrated",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Lands => "farm.lands",
            Self::Bag => "farm.bag",
            Self::Pets => "farm.pets",
            Self::Gifts => "farm.gifts",
            Self::Illustrated => "farm.illustrated",
        }
    }
}

pub(crate) fn page(page: FarmPage) -> impl IntoElement {
    let panel = signal(FarmPanel::Lands);
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let outcome = signal(None::<Outcome>);
    let preferences = resource(
        move || page.account_id.get().filter(|id| !id.is_empty()),
        |id, _| async move {
            session::session()
                .runtime
                .spawn_blocking(move || qq_farm_app::settings::settings_panel(&id, ""))
                .await
                .map_err(|error| error.to_string())
        },
    );
    let pick = super::interaction_items::farm_pick(page.account_id, page.rows);
    pick.reset_for_account(page.account_id);
    pick.reset_when_scope_changes(signal(None::<i64>));
    let fert = FertPick { mode: signal(FertMode::Off), lands: signal(Vec::new()) };
    effect(move || {
        if super::interaction_items::choosing(pick) && fert.mode.get_untracked() != FertMode::Off {
            fert.mode.set(FertMode::Off);
            fert.lands.set(Vec::new());
        }
    });
    let lands = resource(
        move || (panel.get() == FarmPanel::Lands).then(|| ready_account(page)).flatten(),
        |id, _| async move {
            let context = session::session().context.clone();
            session::call(async move { qq_farm_app::farm::lands(&context, &id).await }).await
        },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(text(move || text_key("farm.title")).text_lg().font_weight(FontWeight::SEMIBOLD))
        .child(panel_switch(panel))
        .child(when(
            move || panel.get() == FarmPanel::Lands,
            move || lands_column(page, pending, failure, outcome, lands, preferences, pick, fert),
        ))
        .child(when(
            move || panel.get() == FarmPanel::Bag,
            move || super::bag::section(page.account_id, page.rows),
        ))
        .child(when(
            move || panel.get() == FarmPanel::Pets,
            move || super::pets::section(page.account_id, page.rows),
        ))
        .child(when(
            move || panel.get() == FarmPanel::Gifts,
            move || super::gifts::section(page.account_id, page.rows),
        ))
        .child(when(
            move || panel.get() == FarmPanel::Illustrated,
            move || super::illustrated::section(page.account_id, page.rows),
        ))
}

fn panel_switch(panel: Signal<FarmPanel>) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        || FarmPanel::ALL.to_vec(),
        |item: &FarmPanel| item.id().to_owned(),
        move |item, _| {
            let item = item.get_untracked();
            button(move || text_key(item.label_key()), ButtonVariant::Quiet)
                .selected(move || panel.get() == item)
                .on_press(move || panel.set(item))
        },
    ))
}

fn lands_column(
    page: FarmPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    lands: Resource<LandsPayload, String>,
    preferences: Resource<Value, String>,
    pick: super::interaction_items::InteractionPick,
    fert: FertPick,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(failure_line(failure))
        .child(when(
            move || outcome.get().is_some(),
            move || notice(move || outcome.get().as_ref().map(outcome_line).unwrap_or_default()),
        ))
        .child(gate_notice(page.account_id, page.rows, "farm.needRunning"))
        .child(when(
            move || ready(page) && lands.loading() && lands.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(move || ready(page) && lands.error().is_some(), move || load_error(lands)))
        .child(when(
            move || ready(page) && lands.get().is_some(),
            move || farm_body(page, pending, failure, outcome, lands, preferences, pick, fert),
        ))
}

fn ready(page: FarmPage) -> bool {
    account_gate(page.account_id, page.rows) == AccountGate::Ready
}

fn ready_account(page: FarmPage) -> Option<String> {
    ready(page).then(|| page.account_id.get()).flatten()
}

fn load_error(lands: Resource<LandsPayload, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(crate::ui::failure(move || {
            lands.error().map(SharedString::from).unwrap_or_default()
        }))
        .child(
            button(move || text_key("common.retry"), ButtonVariant::Secondary)
                .on_press(move || lands.refetch()),
        )
}

fn farm_body(
    page: FarmPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    lands: Resource<LandsPayload, String>,
    preferences: Resource<Value, String>,
    pick: super::interaction_items::InteractionPick,
    fert: FertPick,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(summary(lands))
        .child(actions(page, pending, failure, outcome, lands, preferences))
        .child(fertilizer_bar(page, pending, failure, lands, pick, fert))
        .child(super::interaction_items::farm_tools(page.account_id, pending, failure, lands, pick))
        .child(when(
            move || lands.get().is_some_and(|payload| payload.lands.is_empty()),
            move || notice(move || text_key("farm.noLands")),
        ))
        .child(each(
            move || land_lines(lands),
            |land: &LandLine| land.id.to_string(),
            move |land, _| render_land(land.get_untracked(), pick, pending, failure, fert),
        ))
}

fn summary(lands: Resource<LandsPayload, String>) -> View {
    view()
        .flex_row()
        .flex_wrap()
        .gap_4()
        .child(chip("farm.summaryHarvest", move || {
            count(lands, |payload| payload.summary.harvestable)
        }))
        .child(chip("farm.summaryGrowing", move || count(lands, |payload| payload.summary.growing)))
        .child(chip("farm.summaryEmpty", move || count(lands, |payload| payload.summary.empty)))
        .child(chip("farm.summaryDead", move || count(lands, |payload| payload.summary.dead)))
        .child(chip("farm.summaryWater", move || {
            count(lands, |payload| payload.summary.need_water)
        }))
        .child(chip("farm.summaryWeed", move || count(lands, |payload| payload.summary.need_weed)))
        .child(chip("farm.summaryBug", move || count(lands, |payload| payload.summary.need_bug)))
}

fn count(
    lands: Resource<LandsPayload, String>,
    read: impl Fn(&LandsPayload) -> usize,
) -> SharedString {
    SharedString::from(lands.get().map(|payload| read(&payload)).unwrap_or(0).to_string())
}

fn chip(label: &'static str, value: impl IntoReactive<SharedString> + 'static) -> View {
    let value = value.into_reactive();
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            text(move || text_key(label))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(text(value).text_sm().font_weight(FontWeight::MEDIUM))
}

fn actions(
    page: FarmPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    lands: Resource<LandsPayload, String>,
    preferences: Resource<Value, String>,
) -> View {
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(
            button(move || text_key("farm.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || lands.refetch()),
        )
        .child(each(
            move || shown_ops(manual_fertilizer(&preferences.get().unwrap_or(Value::Null))),
            |op: &FarmOp| op.id().to_owned(),
            move |op, _| op_button(op.get_untracked(), page, pending, failure, outcome, lands),
        ))
}

fn shown_ops(allow_fertilizer: bool) -> Vec<FarmOp> {
    FarmOp::ALL.into_iter().filter(|op| *op != FarmOp::Fertilize || allow_fertilizer).collect()
}

fn manual_fertilizer(value: &Value) -> bool {
    value.pointer("/automation/show_manual_fertilizer").and_then(Value::as_bool).unwrap_or(true)
}

fn op_button(
    op: FarmOp,
    page: FarmPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    lands: Resource<LandsPayload, String>,
) -> nsh_base::ButtonElement {
    button(
        move || {
            if pending.get().as_deref() == Some(op.id()) {
                text_key("farm.working")
            } else {
                text_key(op.label_key())
            }
        },
        op.variant(),
    )
    .disabled(move || pending.get().is_some())
    .on_press(move || press_farm(FarmCommand { op, page, pending, failure, outcome, lands }))
}

struct FarmCommand {
    op: FarmOp,
    page: FarmPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    outcome: Signal<Option<Outcome>>,
    lands: Resource<LandsPayload, String>,
}

fn press_farm(command: FarmCommand) {
    if command.pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = command.page.account_id.get_untracked() else {
        return;
    };
    command.pending.set(Some(command.op.id().to_owned()));
    command.failure.set(None);
    command.outcome.set(None);
    if command.op.confirms() {
        confirm::confirm(
            "farm.confirmRemove.title",
            "farm.confirmRemove.body",
            "remove",
            "farm.remove",
            move |accepted| {
                if !accepted {
                    command.pending.set(None);
                    return;
                }
                spawn(async move { finish_farm(command, id).await });
            },
        );
        return;
    }
    spawn(async move { finish_farm(command, id).await });
}

async fn finish_farm(command: FarmCommand, id: String) {
    let context = session::session().context.clone();
    let op_id = command.op.id();
    let result =
        session::call(async move { qq_farm_app::farm::operate(&context, &id, op_id).await }).await;
    match result {
        Ok(value) => command
            .outcome
            .set(Some(Outcome { label_key: command.op.label_key(), detail: op_detail(&value) })),
        Err(message) => command.failure.set(Some(message)),
    }
    command.pending.set(None);
    command.lands.refetch();
}

#[derive(Clone, PartialEq, Eq)]
struct LandLine {
    id: i64,
    title: String,
    subtitle: String,
    marks: Vec<LandMark>,
    target: super::interaction_items::LandTarget,
    fert_root: i64,
    fert_ok: bool,
    care_ok: bool,
}

fn land_lines(lands: Resource<LandsPayload, String>) -> Vec<LandLine> {
    lands
        .get()
        .map(|payload| {
            payload
                .lands
                .iter()
                .map(|land| land_line(land, &payload.lands, &payload.social_event_ids))
                .collect()
        })
        .unwrap_or_default()
}

fn land_line(land: &LandRow, rows: &[LandRow], social: &[i64]) -> LandLine {
    let fert_root = fertilize_root(land);
    let root = rows.iter().find(|row| row.id == fert_root).unwrap_or(land);
    LandLine {
        id: land.id,
        title: land_title(land),
        subtitle: land_subtitle(land),
        marks: land_marks(land),
        target: super::interaction_items::target_from_row(land),
        fert_root,
        fert_ok: can_fertilize(root),
        care_ok: care_selectable(root, rows, social),
    }
}

fn fertilizer_bar(
    page: FarmPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    lands: Resource<LandsPayload, String>,
    pick: super::interaction_items::InteractionPick,
    fert: FertPick,
) -> View {
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(fert_mode_button(FertMode::Normal, "farm.fertNormal", pick, fert))
        .child(fert_mode_button(FertMode::Organic, "farm.fertOrganic", pick, fert))
        .child(fert_mode_button(FertMode::Care, "farm.care", pick, fert))
        .child(when(
            move || fert.mode.get() != FertMode::Off,
            move || {
                view()
                    .flex_row()
                    .gap_2()
                    .child(
                        button(move || text_key("interaction.clear"), ButtonVariant::Secondary)
                            .disabled(move || {
                                pending.get().is_some() || fert.lands.get().is_empty()
                            })
                            .on_press(move || fert.lands.set(Vec::new())),
                    )
                    .child(
                        button(
                            move || {
                                text_key(if fert.mode.get() == FertMode::Care {
                                    "farm.careApply"
                                } else {
                                    "farm.fertApply"
                                })
                            },
                            ButtonVariant::Primary,
                        )
                        .disabled(move || pending.get().is_some() || fert.lands.get().is_empty())
                        .on_press(move || apply_fertilizer(page, pending, failure, lands, fert)),
                    )
            },
        ))
}

fn fert_mode_button(
    mode: FertMode,
    label: &'static str,
    pick: super::interaction_items::InteractionPick,
    fert: FertPick,
) -> nsh_base::ButtonElement {
    button(move || text_key(label), ButtonVariant::Secondary)
        .selected(move || fert.mode.get() == mode)
        .on_press(move || {
            if fert.mode.get_untracked() == mode {
                fert.mode.set(FertMode::Off);
            } else {
                pick.clear_item();
                fert.mode.set(mode);
            }
            fert.lands.set(Vec::new());
        })
}

fn apply_fertilizer(
    page: FarmPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    lands: Resource<LandsPayload, String>,
    fert: FertPick,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = page.account_id.get_untracked() else { return };
    let mode = fert.mode.get_untracked();
    let kind = match mode {
        FertMode::Normal => "normal",
        FertMode::Organic => "organic",
        FertMode::Care => "care",
        FertMode::Off => return,
    };
    let land_ids = fert.lands.get_untracked();
    if land_ids.is_empty() {
        failure.set(Some(
            text_key(if mode == FertMode::Care { "farm.careNeed" } else { "farm.fertNeed" })
                .to_string(),
        ));
        return;
    }
    if mode == FertMode::Care {
        apply_care(page, pending, failure, lands, fert, land_ids);
        return;
    }
    pending.set(Some(format!("fertilize-selected:{kind}")));
    failure.set(None);
    confirm::confirm(
        "farm.confirmFertilize.title",
        "farm.confirmFertilize.body",
        "fertilize",
        "farm.fertApply",
        move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            let kind = kind.to_owned();
            spawn(async move {
                let context = session::session().context.clone();
                let result = session::call(async move {
                    qq_farm_app::farm::fertilize_selected(&context, &id, &kind, &land_ids).await
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(None);
                fert.lands.set(Vec::new());
                lands.refetch();
            });
        },
    );
}

fn render_land(
    land: LandLine,
    pick: super::interaction_items::InteractionPick,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    fert: FertPick,
) -> View {
    let title = SharedString::from(land.title);
    let button_title = title.clone();
    let fert_title = button_title.clone();
    let fert_root = land.fert_root;
    let fert_ok = land.fert_ok;
    let care_ok = land.care_ok;
    let subtitle = SharedString::from(land.subtitle);
    let marks = land.marks;
    let button_target = land.target.clone();
    let press_target = land.target.clone();
    let label_target = land.target;
    let id = land.id;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(when(
            move || !super::interaction_items::choosing(pick) && fert.mode.get() == FertMode::Off,
            move || text(title.clone()).text_sm().font_weight(FontWeight::MEDIUM),
        ))
        .child(when(
            move || !super::interaction_items::choosing(pick) && fert.mode.get() != FertMode::Off,
            move || {
                let fert_title = fert_title.clone();
                button(move || fert_title.clone(), ButtonVariant::Quiet)
                    .selected(move || fert.lands.get().contains(&fert_root))
                    .disabled(move || {
                        pending.get().is_some() || !land_allowed(fert.mode.get(), fert_ok, care_ok)
                    })
                    .on_press(move || {
                        let allowed = land_allowed(fert.mode.get_untracked(), fert_ok, care_ok);
                        if fert.mode.get_untracked() == FertMode::Care {
                            press_care(fert, fert_root, allowed, failure);
                        } else {
                            press_fertilizer(fert, fert_root, allowed, failure);
                        }
                    })
            },
        ))
        .child(when(
            move || super::interaction_items::choosing(pick),
            move || {
                let button_target = button_target.clone();
                let press_target = press_target.clone();
                let button_title = button_title.clone();
                button(move || button_title.clone(), ButtonVariant::Quiet)
                    .selected(move || super::interaction_items::land_selected(pick, id))
                    .disabled(move || {
                        pending.get().is_some()
                            || super::interaction_items::selection_label(pick, &button_target, id)
                                .is_some()
                    })
                    .on_press(move || {
                        super::interaction_items::press_land(pick, &press_target, id, failure)
                    })
            },
        ))
        .child(when(
            {
                let label_target = label_target.clone();
                move || super::interaction_items::selection_label(pick, &label_target, id).is_some()
            },
            move || {
                let label_target = label_target.clone();
                badge(
                    move || {
                        text_key(
                            super::interaction_items::selection_label(pick, &label_target, id)
                                .unwrap_or("interaction.unavailable"),
                        )
                    },
                    BadgeTone::Neutral,
                )
            },
        ))
        .child(text(subtitle).text_sm().text_color(move || nsh_base::theme().muted_foreground))
        .child(each(
            move || marks.clone(),
            |mark: &LandMark| mark.key.to_owned(),
            |mark, _| {
                let mark = mark.get_untracked();
                badge(move || text_key(mark.key), mark.tone)
            },
        ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct LandMark {
    key: &'static str,
    tone: BadgeTone,
}

fn press_fertilizer(fert: FertPick, root: i64, allowed: bool, failure: Signal<Option<String>>) {
    match toggle_fertilizer(&fert.lands.get_untracked(), root, !allowed) {
        None => failure.set(Some(text_key("farm.fertLimit").to_string())),
        Some(next) => {
            failure.set(None);
            fert.lands.set(next);
        }
    }
}

fn apply_care(
    page: FarmPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    lands: Resource<LandsPayload, String>,
    fert: FertPick,
    land_ids: Vec<i64>,
) {
    let Some(id) = page.account_id.get_untracked() else { return };
    pending.set(Some("farm-selected".to_owned()));
    failure.set(None);
    confirm::confirm(
        "farm.confirmCare.title",
        "farm.confirmCare.body",
        "care",
        "farm.careApply",
        move |accepted| {
            if !accepted {
                pending.set(None);
                return;
            }
            spawn(async move {
                let context = session::session().context.clone();
                let result = session::call(async move {
                    qq_farm_app::farm::farm_selected(&context, &id, &land_ids).await
                })
                .await;
                if let Err(message) = result {
                    failure.set(Some(message));
                }
                pending.set(None);
                fert.lands.set(Vec::new());
                lands.refetch();
            });
        },
    );
}

fn land_allowed(mode: FertMode, fert_ok: bool, care_ok: bool) -> bool {
    if mode == FertMode::Care {
        care_ok
    } else {
        fert_ok
    }
}

fn press_care(fert: FertPick, root: i64, allowed: bool, failure: Signal<Option<String>>) {
    let next = toggle_care(&fert.lands.get_untracked(), root, !allowed);
    failure.set(None);
    fert.lands.set(next);
}

fn care_selectable(root: &LandRow, rows: &[LandRow], social: &[i64]) -> bool {
    can_farm(root) || (!social.is_empty() && first_farming_land(rows) == Some(root.id))
}

fn first_farming_land(rows: &[LandRow]) -> Option<i64> {
    rows.iter().find(|land| valid_farming_land(land)).map(|land| land.id)
}

fn valid_farming_land(land: &LandRow) -> bool {
    let name = land.plant_name.as_deref().unwrap_or("").trim();
    land.unlocked
        && !land.occupied_by_master
        && !name.is_empty()
        && land.status != "locked"
        && land.status != "empty"
}

fn can_farm(land: &LandRow) -> bool {
    let name = land.plant_name.as_deref().unwrap_or("").trim();
    land.unlocked
        && !land.occupied_by_master
        && !name.is_empty()
        && land.status != "locked"
        && land.status != "empty"
        && (land.need_water || land.need_weed || land.need_bug || land.need_interaction_cleanup)
}

fn can_fertilize(land: &LandRow) -> bool {
    land.unlocked
        && !land.occupied_by_master
        && land.status == "growing"
        && land.mature_in_sec.unwrap_or(0) > 0
}

fn fertilize_root(land: &LandRow) -> i64 {
    if land.occupied_by_master && land.master_land_id > 0 {
        land.master_land_id
    } else {
        land.id
    }
}

fn toggle_care(selected: &[i64], root: i64, blocked: bool) -> Vec<i64> {
    if blocked || root <= 0 {
        return selected.to_vec();
    }
    let mut next: Vec<i64> = selected.iter().copied().filter(|id| *id != root).collect();
    if next.len() == selected.len() {
        next.push(root);
        next.sort_unstable();
    }
    next
}

fn toggle_fertilizer(selected: &[i64], root: i64, blocked: bool) -> Option<Vec<i64>> {
    if blocked || root <= 0 {
        return Some(selected.to_vec());
    }
    let mut next: Vec<i64> = selected.iter().copied().filter(|id| *id != root).collect();
    if next.len() != selected.len() {
        return Some(next);
    }
    if next.len() >= FERTILIZE_SELECTION_LIMIT {
        return None;
    }
    next.push(root);
    next.sort_unstable();
    Some(next)
}

fn land_title(land: &LandRow) -> String {
    land.plant_name
        .clone()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("#{}", land.id))
}

fn land_subtitle(land: &LandRow) -> String {
    let mut parts = Vec::new();
    if let Some(phase) = land.phase_name.as_ref().filter(|phase| !phase.is_empty()) {
        parts.push(phase.clone());
    }
    if let Some(seconds) = land.mature_in_sec.filter(|seconds| *seconds > 0) {
        parts.push(duration_label(seconds));
    }
    parts.join(" · ")
}

fn land_marks(land: &LandRow) -> Vec<LandMark> {
    if !land.unlocked {
        return vec![LandMark { key: "farm.locked", tone: BadgeTone::Neutral }];
    }
    let mut marks = Vec::new();
    if land.harvestable {
        marks.push(LandMark { key: "farm.harvestable", tone: BadgeTone::Success });
    }
    if land.need_water {
        marks.push(LandMark { key: "farm.needWater", tone: BadgeTone::Neutral });
    }
    if land.need_weed {
        marks.push(LandMark { key: "farm.needWeed", tone: BadgeTone::Neutral });
    }
    if land.need_bug {
        marks.push(LandMark { key: "farm.needBug", tone: BadgeTone::Neutral });
    }
    if marks.is_empty() && land.plant_name.as_deref().unwrap_or("").is_empty() {
        marks.push(LandMark { key: "farm.emptyPlot", tone: BadgeTone::Neutral });
    }
    marks
}

fn duration_label(seconds: i64) -> String {
    if seconds < SECONDS_PER_MINUTE {
        return format!("{seconds}s");
    }
    let minutes = seconds / SECONDS_PER_MINUTE;
    let rest = seconds % SECONDS_PER_MINUTE;
    if rest == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {rest}s")
    }
}

#[cfg(test)]
mod tests {
    use qq_farm_app::dto::LandRow;

    use super::{
        can_farm, can_fertilize, care_selectable, duration_label, fertilize_root, land_marks,
        land_subtitle, land_title, manual_fertilizer, shown_ops, toggle_care, toggle_fertilizer,
    };

    #[test]
    fn title_uses_the_plant_name() {
        let land = LandRow { id: 3, plant_name: Some("萝卜".to_owned()), ..LandRow::default() };
        assert_eq!(land_title(&land), "萝卜");
    }

    #[test]
    fn missing_plant_uses_the_land_number() {
        let land = LandRow { id: 8, plant_name: Some(String::new()), ..LandRow::default() };
        assert_eq!(land_title(&land), "#8");
    }

    #[test]
    fn subtitle_joins_phase_and_remaining_time() {
        let land = LandRow {
            phase_name: Some("成熟".to_owned()),
            mature_in_sec: Some(90),
            ..LandRow::default()
        };
        assert_eq!(land_subtitle(&land), "成熟 · 1m 30s");
        assert_eq!(duration_label(60), "1m");
        assert_eq!(duration_label(12), "12s");
    }

    #[test]
    fn locked_land_has_only_the_locked_mark() {
        let land = LandRow { unlocked: false, harvestable: true, ..LandRow::default() };
        assert_eq!(land_marks(&land).len(), 1);
        assert_eq!(land_marks(&land)[0].key, "farm.locked");
    }

    #[test]
    fn empty_unlocked_land_is_marked_empty() {
        let land = LandRow { id: 1, unlocked: true, ..LandRow::default() };
        assert_eq!(land_marks(&land)[0].key, "farm.emptyPlot");
    }

    #[test]
    fn manual_fertilizer_stays_until_it_is_turned_off() {
        assert!(manual_fertilizer(&serde_json::json!({})));
        assert!(!manual_fertilizer(&serde_json::json!({
            "automation": { "show_manual_fertilizer": false }
        })));
        assert!(shown_ops(true).contains(&super::FarmOp::Fertilize));
        assert!(!shown_ops(false).contains(&super::FarmOp::Fertilize));
    }

    #[test]
    fn fertilizer_selection_keeps_growing_roots_and_stops_at_sixty_four() {
        let growing = LandRow {
            id: 2,
            unlocked: true,
            status: "growing".to_owned(),
            mature_in_sec: Some(30),
            ..LandRow::default()
        };
        let slave = LandRow {
            id: 11,
            unlocked: true,
            occupied_by_master: true,
            master_land_id: 10,
            ..LandRow::default()
        };
        assert!(can_fertilize(&growing));
        assert!(!can_fertilize(&slave));
        assert_eq!(fertilize_root(&slave), 10);
        assert_eq!(toggle_fertilizer(&[2], 10, false), Some(vec![2, 10]));
        assert_eq!(toggle_fertilizer(&[2, 10], 10, false), Some(vec![2]));
        let full: Vec<i64> = (1..=64).collect();
        assert_eq!(toggle_fertilizer(&full, 65, false), None);
        assert_eq!(toggle_fertilizer(&[2], 3, true), Some(vec![2]));
    }

    #[test]
    fn care_selection_keeps_lands_that_need_work_and_uses_the_master_plot() {
        let dry = LandRow {
            id: 4,
            unlocked: true,
            status: "growing".to_owned(),
            plant_name: Some("萝卜".to_owned()),
            need_water: true,
            ..LandRow::default()
        };
        let idle = LandRow {
            id: 5,
            unlocked: true,
            status: "growing".to_owned(),
            plant_name: Some("白菜".to_owned()),
            ..LandRow::default()
        };
        let slave = LandRow {
            id: 11,
            unlocked: true,
            occupied_by_master: true,
            master_land_id: 4,
            ..LandRow::default()
        };
        assert!(can_farm(&dry));
        assert!(!can_farm(&idle));
        assert!(!can_farm(&slave));
        let rows = [idle.clone(), dry.clone()];
        assert!(care_selectable(&idle, &rows, &[5005]));
        assert!(care_selectable(&dry, &rows, &[]));
        assert!(!care_selectable(&idle, &rows, &[]));
        assert_eq!(fertilize_root(&slave), 4);
        assert_eq!(toggle_care(&[4], 8, false), vec![4, 8]);
        assert_eq!(toggle_care(&[4, 8], 8, false), vec![4]);
        assert_eq!(toggle_care(&[4], 8, true), vec![4]);
    }
}
