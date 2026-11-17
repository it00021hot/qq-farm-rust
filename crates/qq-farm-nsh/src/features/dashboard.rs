use nsh_base::{theme, MenuEvent};
use nsh_core::prelude::*;
use qq_farm_app::dto::PanelStatus;
use qq_farm_core::constants::{event_label, module_label};
use qq_farm_core::runtime::runtime_state::{LogEntry, LogFilters, GLOBAL_LOG_CAP};

use super::account_toggle::{toggle_account, AccountToggle};
use super::accounts::{failure_line, AccountRow};
use super::confirm::confirm;
use crate::i18n::text_key;
use crate::navigation::Section;
use crate::session;
use crate::ui::{button, field, notice, ButtonVariant};

const LOG_WINDOW: usize = 80;
const MODULES: &[(&str, &str)] = &[
    ("", "dashboard.allModules"),
    ("farm", "dashboard.modFarm"),
    ("friend", "dashboard.modFriend"),
    ("warehouse", "dashboard.modWarehouse"),
    ("task", "dashboard.modTask"),
    ("system", "dashboard.modSystem"),
];
const LEVELS: &[(&str, &str)] = &[
    ("", "dashboard.allLevels"),
    ("info", "dashboard.levelInfo"),
    ("warn", "dashboard.levelWarn"),
];
const EVENTS: &[(&str, &str)] = &[
    ("", "dashboard.allEvents"),
    ("farm_cycle", "dashboard.evFarmCycle"),
    ("harvest_crop", "dashboard.evHarvest"),
    ("remove_plant", "dashboard.evRemove"),
    ("plant_seed", "dashboard.evPlant"),
    ("fertilize", "dashboard.evFertilize"),
    ("lands_notify", "dashboard.evLands"),
    ("seed_pick", "dashboard.evSeedPick"),
    ("seed_buy", "dashboard.evSeedBuy"),
    ("fertilizer_buy", "dashboard.evFertilizerBuy"),
    ("fertilizer_gift_open", "dashboard.evGift"),
    ("task_scan", "dashboard.evTaskScan"),
    ("task_claim", "dashboard.evTaskClaim"),
    ("mall_free_gifts", "dashboard.evFreeGifts"),
    ("daily_share", "dashboard.evShare"),
    ("vip_daily_gift", "dashboard.evVip"),
    ("month_card_gift", "dashboard.evMonth"),
    ("illustrated_rewards", "dashboard.evIllustrated"),
    ("email_rewards", "dashboard.evEmail"),
    ("sell_success", "dashboard.evSell"),
    ("upgrade_land", "dashboard.evUpgrade"),
    ("unlock_land", "dashboard.evUnlock"),
    ("friend_cycle", "dashboard.evFriendCycle"),
    ("visit_friend", "dashboard.evVisit"),
];

#[derive(Clone, Copy)]
pub(crate) struct DashboardPage {
    pub section: Signal<Section>,
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<AccountRow>, String>,
    pub pending: Signal<Option<String>>,
    pub command_failure: Signal<Option<String>>,
}

pub(crate) fn page(page: DashboardPage) -> impl IntoElement {
    let status = resource(
        move || page.account_id.get(),
        |id, _| async move {
            let panel = session::session()
                .runtime
                .spawn_blocking(move || {
                    qq_farm_app::farm::panel_status_with_progress(&session::session().context, &id)
                })
                .await
                .map_err(|error| error.to_string())?;
            Ok(panel)
        },
    );
    let module = signal(String::new());
    let event = signal(String::new());
    let level = signal(String::new());
    let keyword = signal(String::new());
    let expanded = signal(false);
    let logs = resource(
        move || {
            page.account_id.get().map(|id| LogRequest {
                account_id: id,
                module: module.get(),
                event: event.get(),
                level: level.get(),
                keyword: keyword.get(),
            })
        },
        |request, _| async move {
            let entries = session::session()
                .runtime
                .spawn_blocking(move || {
                    let loaded = qq_farm_app::farm::engine_global_logs(
                        &session::session().context,
                        Some(&request.account_id),
                        GLOBAL_LOG_CAP,
                    );
                    session::session().context.engine.runtime_state().filter_logs(
                        &loaded,
                        &filters_of(&LogQuery {
                            module: request.module,
                            event: request.event,
                            level: request.level,
                            keyword: request.keyword,
                        }),
                    )
                })
                .await
                .map_err(|error| error.to_string())?;
            Ok(entries.into_iter().rev().map(log_line).collect::<Vec<_>>())
        },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(
            text(move || text_key("dashboard.title")).text_lg().font_weight(FontWeight::SEMIBOLD),
        )
        .child(failure_line(page.command_failure))
        .child(when(
            move || page.account_id.get().is_none() && !status.loading(),
            move || {
                view().gap_3().child(notice(move || text_key("dashboard.empty"))).child(
                    button(move || text_key("dashboard.openAccounts"), ButtonVariant::Primary)
                        .on_press(move || page.section.set(Section::Accounts)),
                )
            },
        ))
        .child(when(
            move || status.loading() && status.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || status.error().is_some(),
            move || {
                view().gap_2().child(load_error(status)).child(
                    button(move || text_key("common.retry"), ButtonVariant::Secondary)
                        .on_press(move || status.refetch()),
                )
            },
        ))
        .child(when(move || status.get().is_some(), move || status_block(page, status)))
        .child(when(
            move || page.account_id.get().is_some(),
            move || log_block(LogBoard { logs, module, event, level, keyword, expanded, page }),
        ))
}

#[derive(Clone, Copy)]
struct LogBoard {
    logs: Resource<Vec<LogLine>, String>,
    module: Signal<String>,
    event: Signal<String>,
    level: Signal<String>,
    keyword: Signal<String>,
    expanded: Signal<bool>,
    page: DashboardPage,
}

#[derive(Clone, PartialEq, Eq)]
struct LogRequest {
    account_id: String,
    module: String,
    event: String,
    level: String,
    keyword: String,
}

struct LogQuery {
    module: String,
    event: String,
    level: String,
    keyword: String,
}

#[derive(Clone, PartialEq, Eq)]
struct LogLine {
    id: String,
    module: String,
    event: String,
    message: String,
    time: String,
    warn: bool,
}

struct LogWindow {
    lines: Vec<LogLine>,
    earlier: usize,
}

fn log_block(board: LogBoard) -> impl IntoElement {
    view()
        .gap_2()
        .child(text(move || text_key("dashboard.logs")).text_sm().font_weight(FontWeight::SEMIBOLD))
        .child(filter_choices(MODULES, board.module, board.expanded))
        .child(filter_choices(LEVELS, board.level, board.expanded))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(event_menu(board.event, board.expanded))
                .child(
                    field(
                        move || board.keyword.get(),
                        move || text_key("dashboard.search").to_string(),
                        move |text| {
                            board.keyword.set(text.to_owned());
                            board.expanded.set(false);
                        },
                    )
                    .flex_1(),
                )
                .child(
                    button(move || text_key("dashboard.clear"), ButtonVariant::Danger)
                        .disabled(move || board.page.pending.get().is_some())
                        .on_press(move || clear_account_logs(board)),
                ),
        )
        .child(when(
            move || board.logs.loading() && board.logs.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || board.logs.get().is_some_and(|rows| rows.is_empty()) && !board_filtered(board),
            move || notice(move || text_key("dashboard.noLogs")),
        ))
        .child(when(
            move || board.logs.get().is_some_and(|rows| rows.is_empty()) && board_filtered(board),
            move || notice(move || text_key("dashboard.noMatch")),
        ))
        .child(each(
            move || visible_logs(&board.logs.get().unwrap_or_default(), board.expanded.get()).lines,
            |line: &LogLine| line.id.clone(),
            move |line, _| log_row(line.get_untracked()),
        ))
        .child(when(
            move || {
                visible_logs(&board.logs.get().unwrap_or_default(), board.expanded.get()).earlier
                    > 0
            },
            move || {
                button(
                    move || {
                        let hidden = visible_logs(
                            &board.logs.get().unwrap_or_default(),
                            board.expanded.get(),
                        )
                        .earlier;
                        SharedString::from(
                            text_key("dashboard.earlier")
                                .to_string()
                                .replace("{count}", &hidden.to_string()),
                        )
                    },
                    ButtonVariant::Secondary,
                )
                .on_press(move || board.expanded.set(true))
            },
        ))
}

fn filter_choices(
    choices: &'static [(&str, &str)],
    selected: Signal<String>,
    expanded: Signal<bool>,
) -> impl IntoElement {
    view().flex_row().flex_wrap().gap_2().child(each(
        move || choices.to_vec(),
        |choice: &(&str, &str)| choice.0.to_owned(),
        move |choice, _| {
            let (value, label) = choice.get_untracked();
            button(move || text_key(label), ButtonVariant::Quiet)
                .selected(move || selected.get() == value)
                .on_press(move || {
                    selected.set(value.to_owned());
                    expanded.set(false);
                })
        },
    ))
}

fn event_menu(event: Signal<String>, expanded: Signal<bool>) -> impl IntoElement {
    let open = signal(false);
    let reference = element_ref();
    nsh_base::menu()
        .open(move || Some(open.get()))
        .placement(Placement::Bottom)
        .align(Align::Start)
        .offset(px(4.0))
        .trigger(
            view().child(
                button(move || choice_label(EVENTS, &event.get()), ButtonVariant::Secondary)
                    .element_ref(&reference)
                    .on_press(move || open.update(|value| *value = !*value)),
            ),
        )
        .on_open_change(move |opened: &MenuEvent| {
            if let MenuEvent::OpenChange(value, _) = opened {
                open.set(*value);
            }
        })
        .on_close(move || reference.focus())
        .popup_style(|style| {
            style
                .min_w(px(160.0))
                .p_1()
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().popover)
                .text_color(move || theme().popover_foreground)
                .shadow_md()
        })
        .popup_starting_style(|style| style.opacity(0.0))
        .popup_ending_style(|style| style.opacity(0.0))
        .child(dynamic(move || {
            view().children(EVENTS.iter().map(move |&(value, label)| {
                nsh_base::menu_item()
                    .accessibility_label(move || Some(text_key(label)))
                    .role(Some(nsh_core::AccessibilityRole::MenuItemRadio))
                    .checked(move || Some(event.get() == value))
                    .min_h(px(32.0))
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .flex_row()
                    .items_center()
                    .child(text(move || text_key(label)).text_sm().flex_1())
                    .on_select(move || {
                        event.set(value.to_owned());
                        expanded.set(false);
                        open.set(false);
                    })
            }))
        }))
}

fn log_row(line: LogLine) -> View {
    let event = line.event.clone();
    let event_label = event.clone();
    let time = line.time.clone();
    let time_label = time.clone();
    let warn = line.warn;
    view()
        .gap_1()
        .child(view().flex_row().items_center().gap_2().child(text(line.module).text_sm()).child(
            when(
                move || !time.is_empty(),
                move || {
                    text(time_label.clone()).text_sm().text_color(move || theme().muted_foreground)
                },
            ),
        ))
        .child(when(
            move || !event.is_empty(),
            move || {
                text(event_label.clone()).text_sm().text_color(move || theme().muted_foreground)
            },
        ))
        .child(text(line.message).text_sm().text_color(move || {
            if warn {
                crate::ui::token("destructive")
            } else {
                theme().foreground
            }
        }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{filters_active, filters_of, log_line, visible_logs, LogLine, LogQuery};
    use qq_farm_core::runtime::runtime_state::LogEntry;

    #[test]
    fn ranch_log_uses_the_ranch_module_and_chinese_event() {
        let line = log_line(LogEntry {
            time: "10:00".into(),
            tag: "任务".into(),
            msg: "牧场每日分享已领取".into(),
            meta: json!({ "module": "task", "event": "ranch_daily_share" }),
            ts: 1,
            search_text: String::new(),
            account_id: None,
            account_name: None,
            is_warn: false,
        });
        assert_eq!(line.module, "牧场");
        assert_eq!(line.event, "牧场分享礼包");
        assert!(log_line(LogEntry { meta: json!({ "event": "not_a_real_event" }), ..line_base() })
            .event
            .is_empty());
    }

    #[test]
    fn log_filters_keep_blank_fields_open_and_map_warn_and_info() {
        let open = filters_of(&query("", "", "", "  "));
        assert!(open.module.is_none());
        assert!(open.event.is_none());
        assert!(open.keyword.is_none());
        assert!(open.is_warn.is_none());
        assert!(!filters_active(&query("", "", "", "  ")));

        let narrowed = filters_of(&query("farm", "harvest_crop", "warn", " 收获 "));
        assert_eq!(narrowed.module.as_deref(), Some("farm"));
        assert_eq!(narrowed.event.as_deref(), Some("harvest_crop"));
        assert_eq!(narrowed.keyword.as_deref(), Some("收获"));
        assert_eq!(narrowed.is_warn, Some(true));
        assert!(filters_active(&query("farm", "", "", "")));
        assert_eq!(filters_of(&query("", "", "info", "")).is_warn, Some(false));
    }

    #[test]
    fn log_window_keeps_the_latest_page_until_earlier_entries_are_opened() {
        let lines = (0..81)
            .map(|index| LogLine {
                id: index.to_string(),
                module: String::new(),
                event: String::new(),
                message: String::new(),
                time: String::new(),
                warn: false,
            })
            .collect::<Vec<_>>();
        let first = visible_logs(&lines, false);
        assert_eq!(first.lines.len(), 80);
        assert_eq!(first.lines[0].id, "0");
        assert_eq!(first.earlier, 1);
        let opened = visible_logs(&lines, true);
        assert_eq!(opened.lines.len(), 81);
        assert_eq!(opened.earlier, 0);
        assert_eq!(visible_logs(&lines[..1], false).earlier, 0);
    }

    fn query(module: &str, event: &str, level: &str, keyword: &str) -> LogQuery {
        LogQuery {
            module: module.to_owned(),
            event: event.to_owned(),
            level: level.to_owned(),
            keyword: keyword.to_owned(),
        }
    }

    fn line_base() -> LogEntry {
        LogEntry {
            time: String::new(),
            tag: "农场".into(),
            msg: String::new(),
            meta: json!({}),
            ts: 2,
            search_text: String::new(),
            account_id: None,
            account_name: None,
            is_warn: false,
        }
    }
}

fn log_line(entry: LogEntry) -> LogLine {
    let event = entry.meta.get("event").and_then(|value| value.as_str()).unwrap_or("");
    let module = entry.meta.get("module").and_then(|value| value.as_str()).unwrap_or("");
    LogLine {
        id: format!("{}:{}", entry.ts, entry.msg),
        module: module_label(module, event, &entry.tag),
        event: event_label(event),
        message: entry.msg,
        time: entry.time,
        warn: entry.is_warn,
    }
}

fn filters_of(query: &LogQuery) -> LogFilters {
    LogFilters {
        keyword: filled(&query.keyword),
        module: filled(&query.module),
        event: filled(&query.event),
        is_warn: match query.level.as_str() {
            "warn" => Some(true),
            "info" => Some(false),
            _ => None,
        },
        ..LogFilters::default()
    }
}

fn filled(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn filters_active(query: &LogQuery) -> bool {
    filled(&query.module).is_some()
        || filled(&query.event).is_some()
        || !query.level.is_empty()
        || filled(&query.keyword).is_some()
}

fn board_filtered(board: LogBoard) -> bool {
    filters_active(&LogQuery {
        module: board.module.get(),
        event: board.event.get(),
        level: board.level.get(),
        keyword: board.keyword.get(),
    })
}

fn choice_label(choices: &'static [(&'static str, &'static str)], selected: &str) -> SharedString {
    choices
        .iter()
        .find(|(value, _)| *value == selected)
        .map(|(_, label)| text_key(label))
        .unwrap_or_else(|| text_key("dashboard.allEvents"))
}

fn visible_logs(lines: &[LogLine], expanded: bool) -> LogWindow {
    if expanded || lines.len() <= LOG_WINDOW {
        return LogWindow { lines: lines.to_vec(), earlier: 0 };
    }
    LogWindow { lines: lines[..LOG_WINDOW].to_vec(), earlier: lines.len() - LOG_WINDOW }
}

fn clear_account_logs(board: LogBoard) {
    let Some(id) = untracked(|| board.page.account_id.get()) else {
        return;
    };
    confirm(
        "dashboard.confirmClear.title",
        "dashboard.confirmClear.body",
        "clear",
        "dashboard.clear",
        move |accepted| {
            if !accepted {
                return;
            }
            spawn(async move {
                let cleared = session::session()
                    .runtime
                    .spawn_blocking(move || {
                        qq_farm_app::farm::clear_global_logs(
                            &session::session().context,
                            Some(&id),
                        );
                    })
                    .await;
                if let Err(error) = cleared {
                    board.page.command_failure.set(Some(error.to_string()));
                    return;
                }
                board.logs.refetch();
            });
        },
    );
}

fn load_error(status: Resource<PanelStatus, String>) -> impl IntoElement {
    crate::ui::failure(move || status.error().map(SharedString::from).unwrap_or_default())
}

fn status_block(page: DashboardPage, status: Resource<PanelStatus, String>) -> impl IntoElement {
    view()
        .gap_2()
        .child(metric("dashboard.online", move || online_label(status)))
        .child(metric("dashboard.level", move || number(status, |panel| panel.level)))
        .child(metric("dashboard.gold", move || number(status, |panel| panel.gold)))
        .child(metric("dashboard.exp", move || number(status, |panel| panel.exp)))
        .child(metric("dashboard.sessionExp", move || {
            optional_number(status, |panel| panel.session_exp_gained)
        }))
        .child(metric("dashboard.sessionGold", move || {
            optional_number(status, |panel| panel.session_gold_gained)
        }))
        .child(crate::ui::failure(move || {
            status
                .get()
                .and_then(|panel| panel.last_error)
                .map(SharedString::from)
                .unwrap_or_default()
        }))
        .child(toggle(page, status))
}

fn online_label(status: Resource<PanelStatus, String>) -> SharedString {
    let online = status.get().is_some_and(|panel| panel.online);
    text_key(if online { "dashboard.online" } else { "dashboard.offline" })
}

fn number(
    status: Resource<PanelStatus, String>,
    read: impl Fn(&PanelStatus) -> i64,
) -> SharedString {
    SharedString::from(status.get().map(|panel| read(&panel)).unwrap_or(0).to_string())
}

fn optional_number(
    status: Resource<PanelStatus, String>,
    read: impl Fn(&PanelStatus) -> Option<i64>,
) -> SharedString {
    SharedString::from(
        status
            .get()
            .and_then(|panel| read(&panel))
            .map(|value| value.to_string())
            .unwrap_or_else(|| "0".to_owned()),
    )
}

fn metric(label: &'static str, value: impl IntoReactive<SharedString> + 'static) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_4()
        .child(text(move || text_key(label)).text_sm())
        .child(text(value).text_sm().font_weight(FontWeight::MEDIUM))
}

fn toggle(page: DashboardPage, status: Resource<PanelStatus, String>) -> impl IntoElement {
    each(
        move || current_running(page).into_iter().collect::<Vec<_>>(),
        |active: &bool| active.to_string(),
        move |active, _| command_button(page, status, active.get_untracked()),
    )
}

fn current_running(page: DashboardPage) -> Option<bool> {
    let id = page.account_id.get()?;
    page.rows.get()?.into_iter().find(|row| row.id == id).map(|row| row.running)
}

fn command_button(
    page: DashboardPage,
    status: Resource<PanelStatus, String>,
    running: bool,
) -> impl IntoElement {
    button(
        move || text_key(if running { "accounts.stop" } else { "accounts.start" }),
        if running { ButtonVariant::Danger } else { ButtonVariant::Primary },
    )
    .disabled(move || page.pending.get().is_some())
    .on_press(move || {
        let Some(id) = untracked(|| page.account_id.get()) else {
            return;
        };
        toggle_account(
            AccountToggle { id, running, pending: page.pending, failure: page.command_failure },
            move || {
                page.rows.refetch();
                status.refetch();
            },
        );
    })
}
