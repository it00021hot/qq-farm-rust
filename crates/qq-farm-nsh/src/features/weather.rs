use nsh_core::prelude::*;
use serde_json::Value;

use super::activities::ActivityPage;
use super::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, notice, BadgeTone, ButtonVariant};

#[derive(Clone, PartialEq, Eq)]
struct ResearchNode {
    id: i64,
    state: &'static str,
    cost: i64,
    can_advance: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct WeatherFriend {
    gid: i64,
    name: String,
}

pub(crate) fn loaded(
    board: Resource<Value, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    friends: Resource<Vec<WeatherFriend>, String>,
    visit: Resource<Value, String>,
    selected: Signal<i64>,
    picks: Signal<Vec<i64>>,
    scans: Signal<Option<Value>>,
    live_generation: Signal<u64>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(when(
            move || board.loading() && board.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || board.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        board.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || board.refetch()),
                    )
            },
        ))
        .child(when(
            move || board.get().is_some(),
            move || {
                research_body(
                    board,
                    pending,
                    failure,
                    page,
                    friends,
                    visit,
                    selected,
                    picks,
                    scans,
                    live_generation,
                )
            },
        ))
}

fn research_body(
    board: Resource<Value, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    friends: Resource<Vec<WeatherFriend>, String>,
    visit: Resource<Value, String>,
    selected: Signal<i64>,
    picks: Signal<Vec<i64>>,
    scans: Signal<Option<Value>>,
    live_generation: Signal<u64>,
) -> View {
    view()
        .gap_2()
        .child(
            button(move || text_key("common.refresh"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some())
                .on_press(move || board.refetch()),
        )
        .child(when(
            move || {
                board.get().is_some_and(|value| {
                    !value.get("active").and_then(Value::as_bool).unwrap_or(false)
                })
            },
            move || notice(move || text_key("activities.weatherOff")),
        ))
        .child(
            text(move || weather_line(board.get().as_ref()))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(when(
            move || board.get().is_some_and(|value| summon_available(&value)),
            move || {
                button(move || text_key("activities.weatherSummon"), ButtonVariant::Primary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        confirm::confirm(
                            "activities.confirmSummon.title",
                            "activities.confirmSummon.body",
                            "summon",
                            "activities.weatherSummon",
                            move |accepted| {
                                if accepted {
                                    spawn(async move {
                                        finish_action(
                                            page,
                                            "summon",
                                            failure,
                                            pending,
                                            board,
                                            WeatherCall::Summon,
                                        )
                                        .await
                                    });
                                }
                            },
                        );
                    })
            },
        ))
        .child(when(
            move || board.get().is_some_and(|value| shop_line(&value).is_some()),
            move || shop_row(board, pending, failure, page),
        ))
        .child(each(
            move || board.get().map(|value| task_lines(&value)).unwrap_or_default(),
            |task: &TaskLine| task.id.to_string(),
            move |task, _| {
                let task = task.get_untracked();
                text(move || {
                    let title = task.title.clone();
                    SharedString::from(format!("{title} {}/{}", task.current, task.limit))
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(
            text(move || inventory_line(board.get().as_ref()))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(
            text(move || {
                let count = board
                    .get()
                    .and_then(|value| value.get("badge").and_then(Value::as_i64))
                    .unwrap_or(0);
                SharedString::from(format!("{} {count}", text_key("activities.weatherBadge")))
            })
            .text_sm()
            .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(each(
            move || board.get().map(|value| research_nodes(&value)).unwrap_or_default(),
            |node: &ResearchNode| node.id.to_string(),
            move |node, _| research_row(node.get_untracked(), pending, failure, page, board),
        ))
        .child(
            text(move || text_key("activities.weatherFriends"))
                .text_sm()
                .font_weight(FontWeight::MEDIUM),
        )
        .child(when(
            move || friends.error().is_some(),
            move || {
                crate::ui::failure(move || {
                    friends.error().map(SharedString::from).unwrap_or_default()
                })
            },
        ))
        .child(when(
            move || friends.error().is_none() && friends.get().is_some_and(|rows| rows.is_empty()),
            move || notice(move || text_key("activities.weatherNoFriends")),
        ))
        .child(when(
            move || !picks.get().is_empty(),
            move || {
                button(move || text_key("activities.weatherScan"), ButtonVariant::Secondary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        if !accept_scan(
                            pending.get_untracked().as_deref(),
                            picks.get_untracked().len(),
                        ) {
                            return;
                        }
                        let Some(id) = page.account_id.get_untracked() else {
                            return;
                        };
                        let gids = picks.get_untracked();
                        pending.set(Some("scan".to_owned()));
                        failure.set(None);
                        spawn(async move {
                            finish_scan(page, id, gids, pending, failure, scans, live_generation)
                                .await
                        });
                    })
            },
        ))
        .child(when(
            move || visible_scans(scans.get(), live_generation.get()).is_some(),
            move || scan_result(scans, live_generation, friends),
        ))
        .child(each(
            move || friends.get().unwrap_or_default(),
            |friend: &WeatherFriend| friend.gid.to_string(),
            move |friend, _| {
                let friend = friend.get_untracked();
                let gid = friend.gid;
                let name = friend.name.clone();
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(text(move || SharedString::from(name.clone())).text_sm())
                    .child(
                        button(move || text_key("activities.weatherChoose"), ButtonVariant::Quiet)
                            .selected(move || picks.get().contains(&gid))
                            .disabled(move || {
                                let current = picks.get();
                                current.len() >= 5 && !current.contains(&gid)
                            })
                            .on_press(move || toggle_pick(picks, gid)),
                    )
                    .child(
                        button(move || text_key("activities.weatherLook"), ButtonVariant::Quiet)
                            .selected(move || selected.get() == gid)
                            .on_press(move || {
                                if selected.get() == gid {
                                    selected.set(0);
                                } else {
                                    selected.set(gid);
                                }
                            }),
                    )
            },
        ))
        .child(when(
            move || selected.get() > 0,
            move || visit_panel(board, visit, selected, pending, failure, page),
        ))
}

fn toggle_pick(picks: Signal<Vec<i64>>, gid: i64) {
    let mut current = picks.get();
    if let Some(index) = current.iter().position(|item| *item == gid) {
        current.remove(index);
    } else if current.len() < 5 {
        current.push(gid);
    }
    picks.set(current);
}

fn accept_scan(pending: Option<&str>, count: usize) -> bool {
    pending.is_none() && count > 0
}

fn scan_still_current(started: &str, current: Option<&str>) -> bool {
    current == Some(started)
}

async fn finish_scan(
    page: ActivityPage,
    id: String,
    gids: Vec<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    scans: Signal<Option<Value>>,
    live_generation: Signal<u64>,
) {
    let context = session::session().context.clone();
    let account_id = id.clone();
    let result =
        session::call(
            async move { qq_farm_app::weather::scan(&context, &account_id, &gids).await },
        )
        .await;
    if !scan_still_current(&id, page.account_id.get_untracked().as_deref()) {
        scans.set(None);
        pending.set(None);
        return;
    }
    match result {
        Ok(value) => {
            live_generation.set(value.get("visitGeneration").and_then(Value::as_u64).unwrap_or(0));
            scans.set(Some(value));
        }
        Err(message) => failure.set(Some(message)),
    }
    pending.set(None);
}

fn visible_scans(scans: Option<Value>, live_generation: u64) -> Option<Value> {
    let scans = scans?;
    let captured = scans.get("visitGeneration").and_then(Value::as_u64).unwrap_or(0);
    (captured == live_generation).then_some(scans)
}

fn scan_result(
    scans: Signal<Option<Value>>,
    live_generation: Signal<u64>,
    friends: Resource<Vec<WeatherFriend>, String>,
) -> View {
    view()
        .gap_2()
        .child(each(
            move || {
                visible_scans(scans.get(), live_generation.get())
                    .map(|value| scan_lines(&value))
                    .unwrap_or_default()
            },
            |line: &ScanLine| line.gid.to_string(),
            move |line, _| scan_line_view(line.get_untracked(), friends),
        ))
        .child(when(
            move || {
                visible_scans(scans.get(), live_generation.get())
                    .is_some_and(|value| deferred_count(&value) > 0)
            },
            move || {
                notice(move || {
                    let count = visible_scans(scans.get(), live_generation.get())
                        .map(|value| deferred_count(&value))
                        .unwrap_or(0);
                    SharedString::from(format!(
                        "{} {count}",
                        text_key("activities.weatherDeferred")
                    ))
                })
            },
        ))
}

fn friend_name(friends: Option<&[WeatherFriend]>, gid: i64) -> String {
    friends
        .and_then(|rows| rows.iter().find(|friend| friend.gid == gid))
        .map(|friend| friend.name.clone())
        .unwrap_or_else(|| format!("GID:{gid}"))
}

fn scan_line_view(line: ScanLine, friends: Resource<Vec<WeatherFriend>, String>) -> View {
    let gid = line.gid;
    let state = line.state;
    let error = line.error;
    let failed = !error.is_empty();
    view()
        .child(when(
            move || !failed,
            move || {
                text(move || {
                    let name = friend_name(friends.get().as_deref(), gid);
                    SharedString::from(format!("{name} {}", text_key(state)))
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(
            move || failed,
            move || {
                let detail = error.clone();
                crate::ui::failure(move || {
                    let name = friend_name(friends.get().as_deref(), gid);
                    let detail = detail.clone();
                    SharedString::from(format!(
                        "{name} {} {detail}",
                        text_key("activities.weatherScanFailed")
                    ))
                })
            },
        ))
}

#[derive(Clone, PartialEq, Eq)]
struct ScanLine {
    gid: i64,
    state: &'static str,
    error: String,
}

fn scan_lines(value: &Value) -> Vec<ScanLine> {
    value
        .get("friends")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(scan_line).collect())
        .unwrap_or_default()
}

fn scan_line(value: &Value) -> Option<ScanLine> {
    let gid = value.get("gid").and_then(Value::as_i64).filter(|gid| *gid > 0)?;
    let error = value.get("error").and_then(Value::as_str).unwrap_or("").to_owned();
    let state = if error.is_empty() {
        visit_state_key(value.get("state").and_then(Value::as_str).unwrap_or(""))
    } else {
        "activities.weatherScanFailed"
    };
    Some(ScanLine { gid, state, error })
}

fn visit_state_key(state: &str) -> &'static str {
    match state {
        "ready" => "activities.weatherCanCollect",
        "collected" => "activities.weatherCollected",
        "ended" => "activities.weatherEnded",
        _ => "activities.weatherFriendDry",
    }
}

fn deferred_count(value: &Value) -> usize {
    value.get("deferredGids").and_then(Value::as_array).map(|rows| rows.len()).unwrap_or(0)
}

fn visit_panel(
    board: Resource<Value, String>,
    visit: Resource<Value, String>,
    selected: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || visit.loading() && visit.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || visit.error().is_some(),
            move || {
                crate::ui::failure(move || {
                    visit.error().map(SharedString::from).unwrap_or_default()
                })
            },
        ))
        .child(
            text(move || visit_label(visit.get().as_ref()))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(when(
            move || {
                board
                    .get()
                    .as_ref()
                    .zip(visit.get().as_ref())
                    .is_some_and(|(board, visit)| collect_ready(board, visit))
            },
            move || {
                button(move || text_key("activities.weatherCollect"), ButtonVariant::Primary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        confirm::confirm(
                            "activities.confirmCollect.title",
                            "activities.confirmCollect.body",
                            "collect",
                            "activities.weatherCollect",
                            move |accepted| {
                                if accepted {
                                    let gid = selected.get();
                                    spawn(async move {
                                        finish_action(
                                            page,
                                            "collect",
                                            failure,
                                            pending,
                                            board,
                                            WeatherCall::Collect(gid),
                                        )
                                        .await;
                                        visit.refetch();
                                    });
                                }
                            },
                        );
                    })
            },
        ))
        .child(when(
            move || board.get().is_some_and(|value| bottle_count(&value, "frogCount") > 0),
            move || mischief_button(board, visit, selected, pending, failure, page, Mischief::Frog),
        ))
        .child(when(
            move || {
                board
                    .get()
                    .as_ref()
                    .zip(visit.get().as_ref())
                    .is_some_and(|(board, visit)| cloud_ready(board, visit))
            },
            move || cloud_actions(board, visit, selected, pending, failure, page),
        ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mischief {
    Frog,
}

impl Mischief {
    const fn title(self) -> &'static str {
        match self {
            Self::Frog => "activities.confirmFrog.title",
        }
    }

    const fn body(self) -> &'static str {
        match self {
            Self::Frog => "activities.confirmFrog.body",
        }
    }

    const fn action(self) -> &'static str {
        match self {
            Self::Frog => "frog",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Frog => "activities.weatherFrog",
        }
    }
}

fn mischief_button(
    board: Resource<Value, String>,
    visit: Resource<Value, String>,
    selected: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    kind: Mischief,
) -> impl IntoElement {
    button(move || text_key(kind.label()), ButtonVariant::Secondary)
        .disabled(move || pending.get().is_some())
        .on_press(move || {
            confirm::confirm(
                kind.title(),
                kind.body(),
                kind.action(),
                kind.label(),
                move |accepted| {
                    if accepted {
                        let gid = selected.get();
                        let call = WeatherCall::Frog(gid);
                        spawn(async move {
                            finish_action(page, kind.action(), failure, pending, board, call).await;
                            visit.refetch();
                        });
                    }
                },
            );
        })
}

fn bottle_count(board: &Value, key: &str) -> i64 {
    board.get(key).and_then(Value::as_i64).unwrap_or(0).max(0)
}

fn inventory_counts(board: &Value) -> [i64; 6] {
    [
        bottle_count(board, "lightningMutationCount"),
        bottle_count(board, "lightningAttractCount"),
        bottle_count(board, "frogCount"),
        bottle_count(board, "cloudCount"),
        bottle_count(board, "senseCount"),
        bottle_count(board, "sensePercent"),
    ]
}

fn inventory_line(board: Option<&Value>) -> SharedString {
    let counts = board.map(inventory_counts).unwrap_or([0; 6]);
    let sense = if counts[4] > 0 {
        format!("{} {} +{}%", text_key("activities.weatherSense"), counts[4], counts[5])
    } else {
        text_key("activities.weatherSenseNone").to_string()
    };
    SharedString::from(format!(
        "{} {} · {} {} · {} {} · {} {} · {sense}",
        text_key("activities.weatherMutation"),
        counts[0],
        text_key("activities.weatherAttract"),
        counts[1],
        text_key("activities.weatherFrog"),
        counts[2],
        text_key("activities.weatherCloud"),
        counts[3],
    ))
}

fn cloud_lands(visit: &Value) -> Option<&Vec<Value>> {
    visit.get("cloudLands").and_then(Value::as_array)
}

fn cloud_land_numbers(visit: &Value) -> Vec<i64> {
    cloud_lands(visit)
        .map(|lands| lands.iter().filter_map(Value::as_i64).filter(|id| *id > 0).collect())
        .unwrap_or_default()
}

fn cloud_actions(
    board: Resource<Value, String>,
    visit: Resource<Value, String>,
    selected: Signal<i64>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        move || visit.get().as_ref().map(cloud_land_numbers).unwrap_or_default(),
        |land_id: &i64| land_id.to_string(),
        move |land_id, _| {
            let land_id = land_id.get_untracked();
            button(
                move || {
                    SharedString::from(format!("{} {land_id}", text_key("activities.weatherCloud")))
                },
                ButtonVariant::Secondary,
            )
            .disabled(move || pending.get().is_some())
            .on_press(move || {
                confirm::confirm(
                    "activities.confirmCloud.title",
                    "activities.confirmCloud.body",
                    "cloud",
                    "activities.weatherCloud",
                    move |accepted| {
                        if accepted {
                            let gid = selected.get();
                            spawn(async move {
                                finish_action(
                                    page,
                                    "cloud",
                                    failure,
                                    pending,
                                    board,
                                    WeatherCall::Cloud(gid, land_id),
                                )
                                .await;
                                visit.refetch();
                            });
                        }
                    },
                );
            })
        },
    ))
}

fn cloud_ready(board: &Value, visit: &Value) -> bool {
    bottle_count(board, "cloudCount") > 0
        && cloud_lands(visit).is_some_and(|lands| !lands.is_empty())
}

fn visit_label(visit: Option<&Value>) -> SharedString {
    let Some(visit) = visit else {
        return SharedString::from("");
    };
    let state = visit.get("state").and_then(Value::as_str).unwrap_or("");
    text_key(match state {
        "ready" => "activities.weatherCanCollect",
        "collected" => "activities.weatherCollected",
        "ended" => "activities.weatherEnded",
        _ => "activities.weatherFriendDry",
    })
}

fn collect_ready(board: &Value, visit: &Value) -> bool {
    visit.get("state").and_then(Value::as_str) == Some("ready")
        && board.get("collectorCount").and_then(Value::as_i64).unwrap_or(0) > 0
}

pub(crate) fn weather_friends(rows: Vec<qq_farm_app::dto::FriendSummary>) -> Vec<WeatherFriend> {
    rows.into_iter()
        .filter(|friend| friend.gid > 0)
        .map(|friend| {
            let trimmed = friend.name.trim();
            WeatherFriend {
                gid: friend.gid,
                name: if trimmed.is_empty() {
                    format!("GID:{}", friend.gid)
                } else {
                    trimmed.to_owned()
                },
            }
        })
        .collect()
}

fn research_row(
    node: ResearchNode,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
    board: Resource<Value, String>,
) -> View {
    let id = node.id;
    let state = node.state;
    let cost = node.cost;
    let can_advance = node.can_advance;
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .py_2()
        .child(text(format!("#{id}")).text_sm().font_weight(FontWeight::MEDIUM))
        .child(badge(
            move || text_key(state),
            if can_advance { BadgeTone::Success } else { BadgeTone::Neutral },
        ))
        .child(when(
            move || cost > 0,
            move || {
                text(move || {
                    SharedString::from(format!("{} {cost}", text_key("activities.weatherCost")))
                })
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(
            move || can_advance,
            move || {
                button(move || text_key("activities.weatherAdvance"), ButtonVariant::Primary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        confirm::confirm(
                            "activities.confirmWeather.title",
                            "activities.confirmWeather.body",
                            "advance",
                            "activities.weatherAdvance",
                            move |accepted| {
                                if accepted {
                                    spawn(async move {
                                        finish_action(
                                            page,
                                            "advance",
                                            failure,
                                            pending,
                                            board,
                                            WeatherCall::Advance(id),
                                        )
                                        .await
                                    });
                                }
                            },
                        );
                    })
            },
        ))
}

enum WeatherCall {
    Advance(i64),
    Exchange,
    Summon,
    Collect(i64),
    Frog(i64),
    Cloud(i64, i64),
}

async fn finish_action(
    page: ActivityPage,
    pending_key: &'static str,
    failure: Signal<Option<String>>,
    pending: Signal<Option<String>>,
    board: Resource<Value, String>,
    call: WeatherCall,
) {
    let Some(id) = page.account_id.get_untracked() else {
        return;
    };
    pending.set(Some(pending_key.to_owned()));
    failure.set(None);
    let context = session::session().context.clone();
    let result = session::call(async move {
        match call {
            WeatherCall::Advance(node_id) => {
                qq_farm_app::weather::advance(&context, &id, node_id).await
            }
            WeatherCall::Exchange => qq_farm_app::weather::exchange(&context, &id).await,
            WeatherCall::Summon => qq_farm_app::weather::summon(&context, &id).await,
            WeatherCall::Collect(host_gid) => {
                qq_farm_app::weather::collect(&context, &id, host_gid).await
            }
            WeatherCall::Frog(host_gid) => {
                qq_farm_app::weather::frog(&context, &id, host_gid).await
            }
            WeatherCall::Cloud(host_gid, land_id) => {
                qq_farm_app::weather::cloud(&context, &id, host_gid, land_id).await
            }
        }
    })
    .await;
    if let Err(message) = result {
        failure.set(Some(message));
    }
    pending.set(None);
    board.refetch();
}

fn shop_row(
    board: Resource<Value, String>,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    page: ActivityPage,
) -> View {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(text(move || text_key("activities.collectorBottle")).text_sm())
        .child(
            text(move || shop_cost(board.get().as_ref()))
                .text_sm()
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(when(
            move || {
                board.get().is_some_and(|value| shop_line(&value).is_some_and(|shop| shop.owned))
            },
            move || badge(move || text_key("activities.weatherOwned"), BadgeTone::Neutral),
        ))
        .child(when(
            move || {
                board
                    .get()
                    .is_some_and(|value| shop_line(&value).is_some_and(|shop| shop.available))
            },
            move || {
                button(move || text_key("activities.weatherExchange"), ButtonVariant::Secondary)
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        confirm::confirm(
                            "activities.confirmExchange.title",
                            "activities.confirmExchange.body",
                            "exchange",
                            "activities.weatherExchange",
                            move |accepted| {
                                if accepted {
                                    spawn(async move {
                                        finish_action(
                                            page,
                                            "exchange",
                                            failure,
                                            pending,
                                            board,
                                            WeatherCall::Exchange,
                                        )
                                        .await
                                    });
                                }
                            },
                        );
                    })
            },
        ))
}

fn weather_line(board: Option<&Value>) -> SharedString {
    let Some(board) = board else {
        return SharedString::from("");
    };
    let remain = remaining_sec(board);
    let label = text_key(weather_key(board));
    if remain <= 0 {
        return label;
    }
    let minutes = (remain + 59) / 60;
    SharedString::from(format!("{label} {minutes} {}", text_key("activities.minutes")))
}

fn shop_cost(board: Option<&Value>) -> SharedString {
    let Some(shop) = board.and_then(shop_line) else {
        return SharedString::from("");
    };
    let unit = match shop.cost_id {
        1002 => "dashboard.coupon",
        1005 => "dashboard.bean",
        _ => "dashboard.gold",
    };
    SharedString::from(format!("{} {}", shop.cost, text_key(unit)))
}

#[derive(Clone, PartialEq, Eq)]
struct ShopLine {
    cost_id: i64,
    cost: i64,
    available: bool,
    owned: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct TaskLine {
    id: i64,
    title: String,
    current: i64,
    limit: i64,
}

fn weather_key(value: &Value) -> &'static str {
    let weather = value.get("weather");
    if flag(weather, "thunderstorm") {
        "activities.weatherStorm"
    } else if flag(weather, "active") {
        "activities.weatherOther"
    } else {
        "activities.weatherCalm"
    }
}

fn flag(value: Option<&Value>, key: &str) -> bool {
    value.and_then(|item| item.get(key)).and_then(Value::as_bool).unwrap_or(false)
}

fn remaining_sec(value: &Value) -> i64 {
    value
        .get("weather")
        .and_then(|weather| weather.get("remainingSec"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0)
}

fn summon_available(value: &Value) -> bool {
    value.get("summonAvailable").and_then(Value::as_bool).unwrap_or(false)
}

fn shop_line(value: &Value) -> Option<ShopLine> {
    let shop = value.get("shop")?;
    Some(ShopLine {
        cost_id: shop.get("costId").and_then(Value::as_i64).unwrap_or(0),
        cost: shop.get("costCount").and_then(Value::as_i64).unwrap_or(0).max(0),
        available: shop.get("available").and_then(Value::as_bool).unwrap_or(false),
        owned: shop.get("owned").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn task_lines(value: &Value) -> Vec<TaskLine> {
    value
        .get("tasks")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(task_line).collect())
        .unwrap_or_default()
}

fn task_line(value: &Value) -> Option<TaskLine> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    Some(TaskLine {
        id,
        title: value.get("title").and_then(Value::as_str).unwrap_or("").to_owned(),
        current: value.get("current").and_then(Value::as_i64).unwrap_or(0).max(0),
        limit: value.get("limit").and_then(Value::as_i64).unwrap_or(0).max(0),
    })
}

fn research_nodes(value: &Value) -> Vec<ResearchNode> {
    value
        .get("nodes")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(research_node).collect())
        .unwrap_or_default()
}

fn research_node(value: &Value) -> Option<ResearchNode> {
    let id = value.get("id").and_then(Value::as_i64).filter(|id| *id > 0)?;
    let completed = value.get("completed").and_then(Value::as_bool).unwrap_or(false);
    let available = value.get("available").and_then(Value::as_bool).unwrap_or(false);
    let affordable = value.get("affordable").and_then(Value::as_bool).unwrap_or(false);
    Some(ResearchNode {
        id,
        state: if completed {
            "activities.weatherDone"
        } else if available {
            "activities.weatherReady"
        } else {
            "activities.weatherLocked"
        },
        cost: value.get("costCount").and_then(Value::as_i64).unwrap_or(0).max(0),
        can_advance: available && affordable && !completed,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn ready_affordable_nodes_can_be_lit() {
        let nodes = super::research_nodes(&json!({
            "nodes": [
                { "id": 1, "completed": true, "available": false, "affordable": true, "costCount": 1 },
                { "id": 2, "completed": false, "available": true, "affordable": true, "costCount": 3 },
                { "id": 3, "completed": false, "available": true, "affordable": false, "costCount": 5 }
            ]
        }));
        assert_eq!(nodes[0].state, "activities.weatherDone");
        assert!(!nodes[0].can_advance);
        assert!(nodes[1].can_advance);
        assert!(!nodes[2].can_advance);
    }

    #[test]
    fn calm_weather_can_summon_and_a_storm_shows_its_state() {
        let calm = json!({
            "weather": { "active": false, "thunderstorm": false, "remainingSec": 0 },
            "summonAvailable": true,
            "shop": { "costId": 1005, "costCount": 10, "available": true, "owned": false },
            "tasks": [{ "id": 1, "title": "采集", "current": 2, "limit": 10 }]
        });
        assert_eq!(super::weather_key(&calm), "activities.weatherCalm");
        assert!(super::summon_available(&calm));
        assert!(super::shop_line(&calm).is_some_and(|shop| shop.available));
        assert_eq!(super::task_lines(&calm)[0].current, 2);
        let storm = json!({
            "weather": { "active": true, "thunderstorm": true, "remainingSec": 90 },
            "summonAvailable": false,
            "shop": { "costId": 1005, "costCount": 10, "available": false, "owned": true }
        });
        assert_eq!(super::weather_key(&storm), "activities.weatherStorm");
        assert_eq!(super::remaining_sec(&storm), 90);
        assert!(!super::summon_available(&storm));
        assert!(super::shop_line(&storm).is_some_and(|shop| shop.owned && !shop.available));
    }

    #[test]
    fn collector_bottle_is_required_before_a_ready_friend_can_be_collected() {
        let board = json!({ "collectorCount": 1 });
        let ready = json!({ "state": "ready" });
        let collected = json!({ "state": "collected" });
        assert!(super::collect_ready(&board, &ready));
        assert!(!super::collect_ready(&json!({ "collectorCount": 0 }), &ready));
        assert!(!super::collect_ready(&board, &collected));
        assert!(super::bottle_count(&json!({ "frogCount": 1 }), "frogCount") > 0);
        assert_eq!(
            super::inventory_counts(&json!({
                "lightningMutationCount": 2,
                "lightningAttractCount": 1,
                "frogCount": 0,
                "cloudCount": 4,
                "senseCount": 3,
                "sensePercent": 6
            })),
            [2, 1, 0, 4, 3, 6]
        );
        assert_eq!(super::cloud_land_numbers(&json!({ "cloudLands": [4, 0, 8] })), vec![4, 8]);
        assert!(!super::cloud_ready(&json!({ "cloudCount": 1 }), &json!({ "cloudLands": [] })));
        let scanned = json!({
            "friends": [{ "gid": 8, "state": "ready" }],
            "deferredGids": [9]
        });
        assert_eq!(super::scan_lines(&scanned)[0].state, "activities.weatherCanCollect");
        assert!(super::scan_lines(&scanned)[0].error.is_empty());
        assert_eq!(super::deferred_count(&scanned), 1);
        let failed = json!({ "friends": [{ "gid": 3, "state": "ready", "error": "进入失败" }] });
        let line = &super::scan_lines(&failed)[0];
        assert_eq!(line.state, "activities.weatherScanFailed");
        assert_eq!(line.error, "进入失败");
        assert!(!super::accept_scan(Some("scan"), 1));
        assert!(!super::accept_scan(None, 0));
        assert!(super::accept_scan(None, 2));
        assert!(super::scan_still_current("a", Some("a")));
        assert!(!super::scan_still_current("a", Some("b")));
        assert!(!super::scan_still_current("a", None));
    }

    #[test]
    fn displayed_scan_hides_when_the_visit_generation_moves() {
        let shown = json!({
            "friends": [{ "gid": 8, "state": "ready" }],
            "deferredGids": [],
            "visitGeneration": 2
        });
        let visible = super::visible_scans(Some(shown.clone()), 2).expect("current");
        assert_eq!(super::scan_lines(&visible)[0].gid, 8);
        assert!(super::visible_scans(Some(shown), 3).is_none());
        let unnamed = json!({ "friends": [{ "gid": 9, "state": "ready" }] });
        assert!(super::visible_scans(Some(unnamed), 1).is_none());
    }
}
