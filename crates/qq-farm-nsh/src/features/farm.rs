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
    Interaction,
}

impl FarmPanel {
    const ALL: [Self; 6] =
        [Self::Lands, Self::Bag, Self::Pets, Self::Gifts, Self::Illustrated, Self::Interaction];

    const fn id(self) -> &'static str {
        match self {
            Self::Lands => "lands",
            Self::Bag => "bag",
            Self::Pets => "pets",
            Self::Gifts => "gifts",
            Self::Illustrated => "illustrated",
            Self::Interaction => "interaction",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::Lands => "farm.lands",
            Self::Bag => "farm.bag",
            Self::Pets => "farm.pets",
            Self::Gifts => "farm.gifts",
            Self::Illustrated => "farm.illustrated",
            Self::Interaction => "farm.interaction",
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
            move || lands_column(page, pending, failure, outcome, lands, preferences),
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
        .child(when(
            move || panel.get() == FarmPanel::Interaction,
            move || super::interaction_items::section(page.account_id, page.rows),
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
            move || farm_body(page, pending, failure, outcome, lands, preferences),
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
) -> impl IntoElement {
    view()
        .gap_3()
        .child(summary(lands))
        .child(actions(page, pending, failure, outcome, lands, preferences))
        .child(when(
            move || lands.get().is_some_and(|payload| payload.lands.is_empty()),
            move || notice(move || text_key("farm.noLands")),
        ))
        .child(each(
            move || land_lines(lands),
            |land: &LandLine| land.id.to_string(),
            move |land, _| render_land(land.get_untracked()),
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
}

fn land_lines(lands: Resource<LandsPayload, String>) -> Vec<LandLine> {
    lands.get().map(|payload| payload.lands.iter().map(land_line).collect()).unwrap_or_default()
}

fn land_line(land: &LandRow) -> LandLine {
    LandLine {
        id: land.id,
        title: land_title(land),
        subtitle: land_subtitle(land),
        marks: land_marks(land),
    }
}

fn render_land(land: LandLine) -> View {
    let title = SharedString::from(land.title);
    let subtitle = SharedString::from(land.subtitle);
    let marks = land.marks;
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(text(title).text_sm().font_weight(FontWeight::MEDIUM))
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
        duration_label, land_marks, land_subtitle, land_title, manual_fertilizer, shown_ops,
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
}
