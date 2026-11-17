use nsh_core::prelude::*;
use qq_farm_app::dto::{LandRow, LandsPayload, PanelStatus};
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
    let inspected = signal(None::<i64>);
    effect(move || {
        let _ = page.account_id.get();
        inspected.set(None);
    });
    let status = resource(
        move || inspected.get().and_then(|_| ready_account(page)),
        |id, _| async move {
            session::session()
                .runtime
                .spawn_blocking(move || {
                    qq_farm_app::farm::panel_status_with_progress(&session::session().context, &id)
                })
                .await
                .map_err(|error| error.to_string())
        },
    );
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
            move || {
                lands_column(
                    page,
                    pending,
                    failure,
                    outcome,
                    lands,
                    preferences,
                    pick,
                    fert,
                    inspected,
                    status,
                )
            },
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
    inspected: Signal<Option<i64>>,
    status: Resource<PanelStatus, String>,
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
            move || {
                farm_body(
                    page,
                    pending,
                    failure,
                    outcome,
                    lands,
                    preferences,
                    pick,
                    fert,
                    inspected,
                    status,
                )
            },
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
    inspected: Signal<Option<i64>>,
    status: Resource<PanelStatus, String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(summary(lands))
        .child(when(
            move || lands.get().is_some_and(|payload| payload.career.is_some()),
            move || career_row(lands),
        ))
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
            move |land, _| {
                render_land(land.get_untracked(), pick, pending, failure, fert, inspected, status)
            },
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

fn career_row(lands: Resource<LandsPayload, String>) -> View {
    view()
        .flex_row()
        .flex_wrap()
        .gap_4()
        .child(chip("farm.careerHarvest", move || career_amount(lands, true)))
        .child(chip("farm.careerSteal", move || career_amount(lands, false)))
        .child(chip("farm.careerRatio", move || career_ratio(lands)))
}

fn career_amount(lands: Resource<LandsPayload, String>, harvest: bool) -> SharedString {
    let count = lands
        .get()
        .and_then(|payload| payload.career)
        .map(|career| if harvest { career.harvest } else { career.steal })
        .unwrap_or(0);
    SharedString::from(career_count(count, locale().as_str() == "en-US"))
}

fn career_ratio(lands: Resource<LandsPayload, String>) -> SharedString {
    let (harvest, steal) = lands
        .get()
        .and_then(|payload| payload.career)
        .map(|career| (career.harvest, career.steal))
        .unwrap_or((0, 0));
    SharedString::from(steal_ratio(harvest, steal))
}

pub(crate) fn career_count(count: i64, english: bool) -> String {
    let count = count.max(0);
    if count < 10_000 {
        return count.to_string();
    }
    if english {
        let (value, suffix) = if count >= 1_000_000 {
            (count as f64 / 1_000_000.0, "M")
        } else {
            (count as f64 / 1_000.0, "K")
        };
        return format!("{}{suffix}", trim_number(&format!("{value:.1}")));
    }
    trim_number(&format!("{:.2}", count as f64 / 10_000.0)) + "万"
}

pub(crate) fn steal_ratio(harvest: i64, steal: i64) -> String {
    let harvest = harvest.max(0);
    let steal = steal.max(0);
    if harvest <= 0 {
        return if steal > 0 { "--".to_owned() } else { "0%".to_owned() };
    }
    format!("{}%", trim_number(&format!("{:.1}", steal as f64 / harvest as f64 * 100.0)))
}

fn trim_number(value: &str) -> String {
    let trimmed = value.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
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
    upgrade: Option<UpgradeFact>,
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
        upgrade: land.land_info.as_ref().and_then(upgrade_fact),
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
    inspected: Signal<Option<i64>>,
    status: Resource<PanelStatus, String>,
) -> View {
    let title = SharedString::from(land.title);
    let button_title = title.clone();
    let inspect_title = title.clone();
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
    let upgrade = land.upgrade;
    let can_inspect = upgrade.is_some();
    view()
        .flex_col()
        .gap_1()
        .py_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(when(
                    move || {
                        can_inspect
                            && !super::interaction_items::choosing(pick)
                            && fert.mode.get() == FertMode::Off
                    },
                    move || {
                        let inspect_title = inspect_title.clone();
                        button(move || inspect_title.clone(), ButtonVariant::Quiet)
                            .selected(move || inspected.get() == Some(id))
                            .disabled(move || pending.get().is_some())
                            .on_press(move || toggle_inspected(inspected, id))
                    },
                ))
                .child(when(
                    move || {
                        !can_inspect
                            && !super::interaction_items::choosing(pick)
                            && fert.mode.get() == FertMode::Off
                    },
                    move || text(title.clone()).text_sm().font_weight(FontWeight::MEDIUM),
                ))
                .child(when(
                    move || {
                        !super::interaction_items::choosing(pick)
                            && fert.mode.get() != FertMode::Off
                    },
                    move || {
                        let fert_title = fert_title.clone();
                        button(move || fert_title.clone(), ButtonVariant::Quiet)
                            .selected(move || fert.lands.get().contains(&fert_root))
                            .disabled(move || {
                                pending.get().is_some()
                                    || !land_allowed(fert.mode.get(), fert_ok, care_ok)
                            })
                            .on_press(move || {
                                let allowed =
                                    land_allowed(fert.mode.get_untracked(), fert_ok, care_ok);
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
                                    || super::interaction_items::selection_label(
                                        pick,
                                        &button_target,
                                        id,
                                    )
                                    .is_some()
                            })
                            .on_press(move || {
                                super::interaction_items::press_land(
                                    pick,
                                    &press_target,
                                    id,
                                    failure,
                                )
                            })
                    },
                ))
                .child(when(
                    {
                        let label_target = label_target.clone();
                        move || {
                            super::interaction_items::selection_label(pick, &label_target, id)
                                .is_some()
                        }
                    },
                    move || {
                        let label_target = label_target.clone();
                        badge(
                            move || {
                                text_key(
                                    super::interaction_items::selection_label(
                                        pick,
                                        &label_target,
                                        id,
                                    )
                                    .unwrap_or("interaction.unavailable"),
                                )
                            },
                            BadgeTone::Neutral,
                        )
                    },
                ))
                .child(
                    text(subtitle).text_sm().text_color(move || nsh_base::theme().muted_foreground),
                )
                .child(each(
                    move || marks.clone(),
                    |mark: &LandMark| mark.key.to_owned(),
                    |mark, _| {
                        let mark = mark.get_untracked();
                        badge(move || text_key(mark.key), mark.tone)
                    },
                )),
        )
        .child(when(
            move || {
                can_inspect
                    && inspected.get() == Some(id)
                    && !super::interaction_items::choosing(pick)
                    && fert.mode.get() == FertMode::Off
            },
            move || {
                upgrade.clone().map(|fact| upgrade_panel(fact, status)).unwrap_or_else(|| view())
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

fn toggle_inspected(inspected: Signal<Option<i64>>, id: i64) {
    inspected.set(if inspected.get_untracked() == Some(id) { None } else { Some(id) });
}

fn upgrade_panel(fact: UpgradeFact, status: Resource<PanelStatus, String>) -> View {
    let heading = SharedString::from(upgrade_heading(&fact));
    let notes_fact = fact.clone();
    view().gap_1().child(text(heading).text_sm().font_weight(FontWeight::SEMIBOLD)).child(each(
        move || {
            let wallet = status.get().as_ref().map(wallet_from);
            upgrade_notes(&notes_fact, wallet.as_ref())
        },
        note_key,
        |note, _| {
            let line = SharedString::from(note_text(&note.get_untracked()));
            text(line).text_sm().text_color(move || nsh_base::theme().muted_foreground)
        },
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UpgradeFact {
    mode: &'static str,
    current_name: String,
    next_name: String,
    need_level: Option<i64>,
    prerequisite_land: Option<i64>,
    prerequisite_met: bool,
    preceding_name: String,
    preceding_required: i64,
    preceding_owned: Option<i64>,
    costs: Vec<UpgradeCost>,
    bonuses: Vec<BonusLine>,
    unknown: bool,
    allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UpgradeCost {
    id: i64,
    name: String,
    count: String,
}

#[derive(Clone, Copy)]
struct Wallet {
    level: i64,
    gold: i64,
    coupon: Option<i64>,
    bean: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BonusKind {
    Yield,
    Growth,
    Exp,
    Mutant,
    Resonance,
}

impl BonusKind {
    const ALL: [Self; 5] = [Self::Yield, Self::Growth, Self::Exp, Self::Mutant, Self::Resonance];

    const fn label_key(self) -> &'static str {
        match self {
            Self::Yield => "farm.landYield",
            Self::Growth => "farm.landGrowth",
            Self::Exp => "farm.landExp",
            Self::Mutant => "farm.landMutant",
            Self::Resonance => "farm.landResonance",
        }
    }

    const fn unit_key(self) -> &'static str {
        match self {
            Self::Mutant => "farm.landTimes",
            _ => "farm.landPercent",
        }
    }

    const fn json_key(self) -> &'static str {
        match self {
            Self::Yield => "yieldBonus",
            Self::Growth => "growthReduction",
            Self::Exp => "expBonus",
            Self::Mutant => "mutantMultiplier",
            Self::Resonance => "resonanceBonus",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BonusLine {
    kind: BonusKind,
    current: Option<String>,
    next: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum UpgradeNote {
    Bonus(BonusLine),
    Level { need: i64, have: Option<i64> },
    Prerequisite { land_id: i64, met: bool },
    Preceding { name: String, required: i64, owned: Option<i64> },
    Cost { name: String, count: String, short: Option<i64> },
    Unknown,
    Ready(&'static str),
}

fn wallet_from(status: &PanelStatus) -> Wallet {
    let nested = status.status.as_ref();
    Wallet {
        level: status.level,
        gold: status.gold,
        coupon: nested.and_then(|value| value.get("coupon")).and_then(Value::as_i64),
        bean: nested.and_then(|value| value.get("goldBean")).and_then(Value::as_i64),
    }
}

fn upgrade_fact(info: &Value) -> Option<UpgradeFact> {
    let mode = match info.get("mode").and_then(Value::as_str)? {
        "unlock" => "unlock",
        "upgrade" => "upgrade",
        "max" => "max",
        _ => return None,
    };
    let current_name = json_text(info.pointer("/current/name"));
    let next_name = json_text(info.pointer("/next/name"));
    let need_level = info.get("needLevel").and_then(json_i64);
    let prerequisite_land = info.pointer("/prerequisite/landId").and_then(json_i64);
    let prerequisite_met =
        info.pointer("/prerequisite/met").and_then(Value::as_bool).unwrap_or(false);
    let preceding_name = json_text(info.pointer("/precedingLands/name"));
    let preceding_required =
        info.pointer("/precedingLands/required").and_then(json_i64).unwrap_or(0);
    let preceding_owned = info.pointer("/precedingLands/owned").and_then(json_i64);
    let costs = info
        .get("costs")
        .and_then(Value::as_array)
        .map(|costs| costs.iter().filter_map(upgrade_cost).collect())
        .unwrap_or_default();
    Some(UpgradeFact {
        mode,
        current_name,
        next_name,
        need_level,
        prerequisite_land,
        prerequisite_met,
        preceding_name,
        preceding_required,
        preceding_owned,
        costs,
        bonuses: bonus_lines(info),
        unknown: info.get("unknown").and_then(Value::as_bool).unwrap_or(false),
        allowed: info.get("serverAllowed").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn upgrade_cost(value: &Value) -> Option<UpgradeCost> {
    let id = value.get("id").and_then(json_i64).filter(|id| *id > 0)?;
    let count = value.get("count").and_then(Value::as_str).filter(|count| !count.is_empty())?;
    let name = json_text(value.get("name"));
    let name = if name.is_empty() { format!("物品 {id}") } else { name };
    Some(UpgradeCost { id, name, count: count.to_owned() })
}

fn bonus_lines(info: &Value) -> Vec<BonusLine> {
    BonusKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let current = level_number(info, "current", kind.json_key());
            let next = level_number(info, "next", kind.json_key());
            (current.is_some() || next.is_some()).then_some(BonusLine { kind, current, next })
        })
        .collect()
}

fn level_number(info: &Value, side: &str, key: &str) -> Option<String> {
    match info.get(side)?.get(key)? {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        _ => None,
    }
}

fn upgrade_notes(fact: &UpgradeFact, wallet: Option<&Wallet>) -> Vec<UpgradeNote> {
    let mut notes = fact.bonuses.iter().cloned().map(UpgradeNote::Bonus).collect::<Vec<_>>();
    if fact.mode == "max" {
        return notes;
    }
    if let Some(need) = fact.need_level {
        notes.push(UpgradeNote::Level { need, have: wallet.map(|wallet| wallet.level) });
    }
    if let Some(land_id) = fact.prerequisite_land {
        notes.push(UpgradeNote::Prerequisite { land_id, met: fact.prerequisite_met });
    }
    if fact.preceding_required > 0 {
        notes.push(UpgradeNote::Preceding {
            name: fact.preceding_name.clone(),
            required: fact.preceding_required,
            owned: fact.preceding_owned,
        });
    }
    for cost in &fact.costs {
        notes.push(UpgradeNote::Cost {
            name: cost.name.clone(),
            count: cost.count.clone(),
            short: wallet.and_then(|wallet| cost_short(cost, wallet)),
        });
    }
    if fact.unknown {
        notes.push(UpgradeNote::Unknown);
    }
    if fact.allowed {
        notes.push(UpgradeNote::Ready(if fact.mode == "unlock" {
            "farm.landCanUnlock"
        } else {
            "farm.landCanUpgrade"
        }));
    }
    notes
}

fn cost_short(cost: &UpgradeCost, wallet: &Wallet) -> Option<i64> {
    let balance = match cost.id {
        1001 => Some(wallet.gold),
        1002 => wallet.coupon,
        1005 => wallet.bean,
        _ => None,
    }?;
    let required = cost.count.parse::<i64>().ok()?;
    Some((required - balance).max(0))
}

fn upgrade_heading(fact: &UpgradeFact) -> String {
    match fact.mode {
        "max" => text_key("farm.landMax").to_string(),
        "unlock" if !fact.next_name.is_empty() => {
            text_key("farm.landUnlockTo").replace("{name}", &fact.next_name)
        }
        "upgrade" if !fact.current_name.is_empty() && !fact.next_name.is_empty() => {
            text_key("farm.landUpgradeTo")
                .replace("{current}", &fact.current_name)
                .replace("{next}", &fact.next_name)
        }
        "unlock" => text_key("farm.unlock").to_string(),
        _ => text_key("farm.upgrade").to_string(),
    }
}

fn note_text(note: &UpgradeNote) -> String {
    match note {
        UpgradeNote::Bonus(line) => bonus_text(line),
        UpgradeNote::Level { need, have: Some(have) } => text_key("farm.landLevelNow")
            .replace("{have}", &have.to_string())
            .replace("{need}", &need.to_string()),
        UpgradeNote::Level { need, have: None } => {
            text_key("farm.landLevel").replace("{need}", &need.to_string())
        }
        UpgradeNote::Prerequisite { land_id, met: true } => {
            text_key("farm.landBeforeMet").replace("{id}", &land_id.to_string())
        }
        UpgradeNote::Prerequisite { land_id, met: false } => {
            text_key("farm.landBefore").replace("{id}", &land_id.to_string())
        }
        UpgradeNote::Preceding { name, required, owned: Some(owned) } => text_key("farm.landCount")
            .replace("{name}", name)
            .replace("{owned}", &owned.to_string())
            .replace("{required}", &required.to_string()),
        UpgradeNote::Preceding { name, required, owned: None } => text_key("farm.landCountUnknown")
            .replace("{name}", name)
            .replace("{required}", &required.to_string()),
        UpgradeNote::Cost { name, count, short: Some(0) } => {
            text_key("farm.landEnough").replace("{name}", name).replace("{count}", count)
        }
        UpgradeNote::Cost { name, count, short: Some(short) } => text_key("farm.landShort")
            .replace("{name}", name)
            .replace("{count}", count)
            .replace("{short}", &short.to_string()),
        UpgradeNote::Cost { name, count, short: None } => {
            text_key("farm.landCost").replace("{name}", name).replace("{count}", count)
        }
        UpgradeNote::Unknown => text_key("farm.landUnknown").to_string(),
        UpgradeNote::Ready(key) => text_key(key).to_string(),
    }
}

fn bonus_text(line: &BonusLine) -> String {
    let label = text_key(line.kind.label_key());
    let left = bonus_side(line.kind, line.current.as_deref());
    let right = bonus_side(line.kind, line.next.as_deref());
    match (line.current.is_some(), line.next.is_some()) {
        (true, true) => format!("{label} {left} → {right}"),
        (true, false) => format!("{label} {left}"),
        (false, true) => format!("{label} {right}"),
        (false, false) => label.to_string(),
    }
}

fn bonus_side(kind: BonusKind, value: Option<&str>) -> String {
    let Some(value) = value else {
        return text_key("farm.landDash").to_string();
    };
    if kind == BonusKind::Mutant && value.parse::<f64>().ok().is_some_and(|number| number == 0.0) {
        return text_key("farm.landNone").to_string();
    }
    format!("{value}{}", text_key(kind.unit_key()))
}

fn note_key(note: &UpgradeNote) -> String {
    match note {
        UpgradeNote::Bonus(line) => format!("bonus-{}", line.kind.json_key()),
        UpgradeNote::Level { need, .. } => format!("level-{need}"),
        UpgradeNote::Prerequisite { land_id, .. } => format!("before-{land_id}"),
        UpgradeNote::Preceding { required, .. } => format!("count-{required}"),
        UpgradeNote::Cost { name, count, .. } => format!("cost-{name}-{count}"),
        UpgradeNote::Unknown => "unknown".to_owned(),
        UpgradeNote::Ready(key) => (*key).to_owned(),
    }
}

fn json_text(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or("").trim().to_owned()
}

fn json_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str().and_then(|text| text.parse().ok()))
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
    use serde_json::json;

    use super::{
        can_farm, can_fertilize, care_selectable, career_count, duration_label, fertilize_root,
        land_marks, land_subtitle, land_title, manual_fertilizer, shown_ops, steal_ratio,
        toggle_care, toggle_fertilizer, upgrade_fact, upgrade_notes, BonusKind, BonusLine,
        UpgradeNote, Wallet,
    };

    #[test]
    fn career_counts_use_wan_or_english_units_and_ratio_hides_zero_harvest() {
        assert_eq!(career_count(9_999, false), "9999");
        assert_eq!(career_count(12_340, false), "1.23万");
        assert_eq!(career_count(1_200_000, true), "1.2M");
        assert_eq!(career_count(12_340, true), "12.3K");
        assert_eq!(steal_ratio(10, 2), "20%");
        assert_eq!(steal_ratio(0, 4), "--");
        assert_eq!(steal_ratio(0, 0), "0%");
    }

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

    #[test]
    fn land_upgrade_notes_compare_level_and_currency() {
        let fact = upgrade_fact(&json!({
            "mode": "upgrade",
            "current": { "name": "红土地", "expBonus": 20, "mutantMultiplier": 0 },
            "next": { "name": "黑土地", "expBonus": 25, "mutantMultiplier": 1.2 },
            "needLevel": 150,
            "precedingLands": { "name": "红土地", "required": 1, "owned": 1, "met": true },
            "costs": [
                { "id": "1001", "name": "金币", "count": "450000000" },
                { "id": "1005", "name": "金豆豆", "count": "18670" }
            ],
            "serverAllowed": false,
            "unknown": false
        }))
        .expect("upgrade");
        let wallet = Wallet { level: 12, gold: 100, coupon: None, bean: Some(18670) };
        assert_eq!(
            upgrade_notes(&fact, Some(&wallet)),
            vec![
                UpgradeNote::Bonus(BonusLine {
                    kind: BonusKind::Exp,
                    current: Some("20".to_owned()),
                    next: Some("25".to_owned()),
                }),
                UpgradeNote::Bonus(BonusLine {
                    kind: BonusKind::Mutant,
                    current: Some("0".to_owned()),
                    next: Some("1.2".to_owned()),
                }),
                UpgradeNote::Level { need: 150, have: Some(12) },
                UpgradeNote::Preceding {
                    name: "红土地".to_owned(), required: 1, owned: Some(1)
                },
                UpgradeNote::Cost {
                    name: "金币".to_owned(),
                    count: "450000000".to_owned(),
                    short: Some(449_999_900),
                },
                UpgradeNote::Cost {
                    name: "金豆豆".to_owned(),
                    count: "18670".to_owned(),
                    short: Some(0),
                },
            ]
        );
        let locked = upgrade_fact(&json!({
            "mode": "unlock",
            "next": { "name": "普通土地" },
            "needLevel": 10,
            "prerequisite": { "landId": 7, "met": false },
            "costs": [{ "id": "1001", "name": "金币", "count": "100000" }],
            "serverAllowed": true
        }))
        .expect("unlock");
        assert!(upgrade_notes(&locked, None)
            .iter()
            .any(|note| matches!(note, UpgradeNote::Prerequisite { land_id: 7, met: false })));
        let maxed = upgrade_fact(&json!({
            "mode": "max",
            "current": { "name": "蓝晶土地", "expBonus": 50 }
        }))
        .expect("max");
        assert_eq!(
            upgrade_notes(&maxed, None),
            vec![UpgradeNote::Bonus(BonusLine {
                kind: BonusKind::Exp,
                current: Some("50".to_owned()),
                next: None,
            })]
        );
    }
}
