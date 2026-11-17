use nsh_base::{theme, MenuEvent};
use nsh_core::prelude::*;
use qq_farm_app::dto::PanelStatus;
use qq_farm_core::constants::{event_label, module_label};
use qq_farm_core::runtime::runtime_state::{LogEntry, LogFilters, GLOBAL_LOG_CAP};
use serde_json::Value;

use super::account_toggle::{toggle_account, AccountToggle};
use super::accounts::{failure_line, platform_label, selected_account, AccountRow};
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
    let inventory = resource(
        move || running_id(page),
        |id, _| async move {
            let context = session::session().context.clone();
            let bag =
                session::call(async move { qq_farm_app::farm::bag(&context, &id).await }).await?;
            Ok(inventory_from(&bag))
        },
    );
    let diamond = resource(
        move || running_id(page),
        |id, _| async move {
            let context = session::session().context.clone();
            session::call(async move { qq_farm_app::farm::diamond_balance(&context, &id).await })
                .await
        },
    );
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
        .child(when(
            move || status.get().is_some(),
            move || status_block(page, status, diamond, inventory),
        ))
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
    count: usize,
    started: String,
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
    let count = line.count;
    let started = line.started.clone();
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
        .child(when(
            move || count > 1,
            move || {
                let started = started.clone();
                text(move || {
                    text_key("dashboard.repeat")
                        .replace("{count}", &count.to_string())
                        .replace("{time}", &started)
                })
                .text_sm()
                .text_color(move || theme().muted_foreground)
            },
        ))
}

#[cfg(test)]
mod tests {
    use qq_farm_app::dto::PanelStatus;
    use qq_farm_core::services::warehouse::{BagDetail, BagItemView, SystemBagItem};
    use serde_json::json;

    use super::{
        clock_text, exp_per_hour, filters_active, filters_of, forecast_notes, group_logs,
        inventory_from, log_line, patrol_text, progress_copy, shown_operations, shown_uptime,
        visible_logs, ForecastNote, LogLine, LogQuery, PatrolText,
    };
    use qq_farm_core::runtime::runtime_state::LogEntry;

    #[test]
    fn overview_keeps_next_checks_and_todays_counts() {
        let panel = PanelStatus::from_engine_value(
            &json!({
                "connection": { "connected": true },
                "uptime": 3661,
                "status": { "name": "农场主", "coupon": 12, "goldBean": 3 },
                "nextChecks": { "farmRemainSec": 3661, "helpRemainSec": 90, "friendQuiet": true },
                "operations": { "harvest": 2, "plant": 0, "upgrade": 9, "levelUp": 1, "extra": 4 }
            }),
            "account",
            true,
        );
        assert_eq!(
            patrol_text(true, &panel),
            PatrolText::Clock { days: 0, clock: "01:01:01".to_owned() }
        );
        assert_eq!(patrol_text(false, &panel), PatrolText::Quiet);
        assert_eq!(panel.nick, "农场主");
        assert_eq!(shown_uptime(&panel), Some(3661));
        let offline = PanelStatus::from_engine_value(&json!({ "uptime": 3661 }), "account", false);
        assert_eq!(shown_uptime(&offline), None);
        assert_eq!(clock_text(0), (0, "00:00:00".to_owned()));
        assert_eq!(clock_text(90_061), (1, "01:01:01".to_owned()));
        assert_eq!(
            shown_operations(&panel.operations)
                .into_iter()
                .map(|row| (row.key, row.count))
                .collect::<Vec<_>>(),
            vec![("harvest".to_owned(), 2), ("extra".to_owned(), 4)]
        );
        let offline = PanelStatus::from_engine_value(
            &json!({ "nextChecks": { "farmRemainSec": 10, "farmQuiet": true } }),
            "account",
            false,
        );
        assert_eq!(patrol_text(true, &offline), PatrolText::Offline);
    }

    #[test]
    fn inventory_uses_hours_text_then_seconds_and_keeps_the_first_item() {
        let stock = inventory_from(&BagDetail {
            items: vec![
                BagItemView {
                    id: 1011,
                    count: 10,
                    hours_text: "1.5小时".to_owned(),
                    ..BagItemView::default()
                },
                BagItemView { id: 3001, count: 4, ..BagItemView::default() },
            ],
            system_items: vec![
                SystemBagItem { id: 1012, count: 7200, ..SystemBagItem::default() },
                SystemBagItem { id: 3001, count: 99, ..SystemBagItem::default() },
                SystemBagItem { id: 3002, count: 6, ..SystemBagItem::default() },
            ],
            ..BagDetail::default()
        });
        assert_eq!(stock.normal_fertilizer, "1.5h");
        assert_eq!(stock.organic_fertilizer, "2.0h");
        assert_eq!(stock.normal_collection, 4);
        assert_eq!(stock.rare_collection, 6);
    }

    #[test]
    fn progress_rate_and_forecast_follow_the_bot_overview() {
        assert_eq!(progress_copy(30, 120), "30 / 120 · 25%");
        assert_eq!(exp_per_hour(3600, 1800.0), 7200);
        assert_eq!(exp_per_hour(10, 0.0), 0);
        let generated = 1_800_000_000_000_i64;
        let planted = forecast_notes(Some(&json!({
            "generatedAt": generated,
            "scheduledExp": 20_544,
            "plotCount": 24,
            "estimatedAt": generated + 21_614_000_i64,
            "source": "planted",
            "automatic": true
        })));
        assert_eq!(
            planted,
            vec![
                ForecastNote::Later {
                    minutes: 361,
                    planted: true,
                    basis: Some("dashboard.byCrops"),
                },
                ForecastNote::Plots { count: 24, exp: 20_544 },
            ]
        );
        assert_eq!(forecast_notes(None), Vec::<ForecastNote>::new());
        assert_eq!(
            forecast_notes(Some(&json!({ "plotCount": 0, "source": "unavailable" }))),
            vec![ForecastNote::Waiting]
        );
    }

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
                module: format!("module-{index}"),
                event: String::new(),
                message: String::new(),
                time: String::new(),
                warn: false,
                count: 1,
                started: String::new(),
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

    #[test]
    fn repeated_logs_collapse_when_only_the_numbers_change() {
        let line = |message: &str, time: &str| LogLine {
            id: time.to_owned(),
            module: "农场".to_owned(),
            event: "农场巡查".to_owned(),
            message: message.to_owned(),
            time: time.to_owned(),
            warn: false,
            count: 1,
            started: time.to_owned(),
        };
        let groups = group_logs(&[
            line("收获 3 块", "10:03"),
            line("收获 2 块", "10:02"),
            line("收获 1 块", "10:01"),
            line("出售完成", "10:00"),
        ]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].message, "收获 3 块");
        assert_eq!(groups[0].count, 3);
        assert_eq!(groups[0].time, "10:03");
        assert_eq!(groups[0].started, "10:01");
        assert_eq!(groups[1].count, 1);
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
        time: entry.time.clone(),
        warn: entry.is_warn,
        count: 1,
        started: entry.time,
    }
}

fn group_logs(lines: &[LogLine]) -> Vec<LogLine> {
    let mut groups = Vec::new();
    for line in lines {
        let same = groups.last().is_some_and(|last| log_signature(last) == log_signature(line));
        if same {
            let last = groups.last_mut().expect("matched group");
            last.count += line.count.max(1);
            last.started = line.time.clone();
            continue;
        }
        let mut grouped = line.clone();
        grouped.count = line.count.max(1);
        grouped.started = line.time.clone();
        groups.push(grouped);
    }
    groups
}

fn log_signature(line: &LogLine) -> String {
    format!("{}|{}|{}", line.module, line.event, mask_digits(&line.message))
}

fn mask_digits(text: &str) -> String {
    text.chars().map(|ch| if ch.is_ascii_digit() { '#' } else { ch }).collect()
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
    let groups = group_logs(lines);
    if expanded || groups.len() <= LOG_WINDOW {
        return LogWindow { lines: groups, earlier: 0 };
    }
    let earlier = groups.len() - LOG_WINDOW;
    LogWindow { lines: groups[..LOG_WINDOW].to_vec(), earlier }
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

fn status_block(
    page: DashboardPage,
    status: Resource<PanelStatus, String>,
    diamond: Resource<i64, String>,
    inventory: Resource<Inventory, String>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(metric("dashboard.name", move || nick_text(status)))
        .child(metric("dashboard.platform", move || account_platform(page)))
        .child(metric("dashboard.online", move || online_label(status)))
        .child(metric("dashboard.uptime", move || uptime_text(status)))
        .child(metric("dashboard.level", move || number(status, |panel| panel.level)))
        .child(metric("dashboard.gold", move || number(status, |panel| panel.gold)))
        .child(metric("dashboard.coupon", move || nested_number(status, "coupon")))
        .child(metric("dashboard.bean", move || nested_number(status, "goldBean")))
        .child(metric("dashboard.diamond", move || diamond_text(diamond)))
        .child(metric("dashboard.fertNormal", move || inventory_hours(inventory, true)))
        .child(metric("dashboard.fertOrganic", move || inventory_hours(inventory, false)))
        .child(metric("dashboard.collectNormal", move || inventory_count(inventory, true)))
        .child(metric("dashboard.collectRare", move || inventory_count(inventory, false)))
        .child(metric("dashboard.exp", move || number(status, |panel| panel.exp)))
        .child(metric("dashboard.progress", move || progress_text(status)))
        .child(metric("dashboard.expRate", move || rate_text(status)))
        .child(forecast_block(status))
        .child(metric("dashboard.nextFarm", move || patrol_label(status, true)))
        .child(metric("dashboard.nextFriend", move || patrol_label(status, false)))
        .child(operations_block(status))
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

fn progress_text(status: Resource<PanelStatus, String>) -> SharedString {
    let Some(progress) = status.get().and_then(|panel| panel.level_progress.clone()) else {
        return text_key("dashboard.dash");
    };
    SharedString::from(progress_copy(progress.current, progress.needed))
}

fn progress_copy(current: i64, needed: i64) -> String {
    if needed <= 0 {
        return current.to_string();
    }
    let percent = current.max(0).saturating_mul(100) / needed;
    format!("{current} / {needed} · {percent}%")
}

fn rate_text(status: Resource<PanelStatus, String>) -> SharedString {
    let Some(panel) = status.get() else {
        return text_key("dashboard.dash");
    };
    let seconds = panel
        .session_elapsed_seconds
        .filter(|seconds| *seconds > 0.0)
        .unwrap_or_else(|| panel.uptime.unwrap_or(0) as f64);
    let rate = exp_per_hour(panel.session_exp_gained.unwrap_or(0), seconds);
    SharedString::from(text_key("dashboard.perHour").replace("{rate}", &rate.to_string()))
}

fn exp_per_hour(gain: i64, seconds: f64) -> i64 {
    if seconds <= 0.0 {
        return 0;
    }
    let hours = seconds / 3600.0;
    if hours <= 0.0 {
        return 0;
    }
    (gain as f64 / hours).floor() as i64
}

fn forecast_block(status: Resource<PanelStatus, String>) -> View {
    view()
        .gap_1()
        .child(when(
            move || !forecast_of(status).is_empty(),
            move || {
                text(move || text_key("dashboard.forecast"))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
            },
        ))
        .child(each(
            move || forecast_of(status),
            forecast_key,
            |note, _| {
                let line = SharedString::from(forecast_copy(&note.get_untracked()));
                text(line).text_sm().text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
}

fn forecast_of(status: Resource<PanelStatus, String>) -> Vec<ForecastNote> {
    status.get().map(|panel| forecast_notes(panel.level_forecast.as_ref())).unwrap_or_default()
}

fn nested_number(status: Resource<PanelStatus, String>, key: &'static str) -> SharedString {
    let amount = status.get().map_or(0, |panel| {
        panel.status.as_ref().and_then(|value| value.get(key)).and_then(Value::as_i64).unwrap_or(0)
    });
    SharedString::from(amount.to_string())
}

fn inventory_hours(inventory: Resource<Inventory, String>, normal: bool) -> SharedString {
    inventory
        .get()
        .map(|stock| {
            SharedString::from(if normal {
                stock.normal_fertilizer
            } else {
                stock.organic_fertilizer
            })
        })
        .unwrap_or_else(|| text_key("dashboard.dash"))
}

fn inventory_count(inventory: Resource<Inventory, String>, normal: bool) -> SharedString {
    inventory
        .get()
        .map(|stock| {
            SharedString::from(
                (if normal { stock.normal_collection } else { stock.rare_collection }).to_string(),
            )
        })
        .unwrap_or_else(|| text_key("dashboard.dash"))
}

fn diamond_text(diamond: Resource<i64, String>) -> SharedString {
    diamond
        .get()
        .map(|value| SharedString::from(value.to_string()))
        .unwrap_or_else(|| text_key("dashboard.dash"))
}

fn patrol_label(status: Resource<PanelStatus, String>, farm: bool) -> SharedString {
    let Some(panel) = status.get() else {
        return text_key("dashboard.dash");
    };
    SharedString::from(patrol_copy(&patrol_text(farm, &panel)))
}

fn patrol_copy(text: &PatrolText) -> String {
    match text {
        PatrolText::Offline => text_key("dashboard.notLoggedIn").to_string(),
        PatrolText::Quiet => text_key("dashboard.quiet").to_string(),
        PatrolText::Checking => text_key("dashboard.checking").to_string(),
        PatrolText::Clock { days: 0, clock } => clock.clone(),
        PatrolText::Clock { days, clock } => text_key("dashboard.days")
            .replace("{days}", &days.to_string())
            .replace("{clock}", clock),
    }
}

fn operations_block(status: Resource<PanelStatus, String>) -> View {
    view()
        .gap_2()
        .child(
            text(move || text_key("dashboard.today")).text_sm().font_weight(FontWeight::SEMIBOLD),
        )
        .child(when(
            move || {
                status.get().is_some_and(|panel| {
                    panel.online && shown_operations(&panel.operations).is_empty()
                })
            },
            move || {
                text(move || text_key("dashboard.noOps"))
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(when(
            move || status.get().is_some_and(|panel| !panel.online),
            move || {
                text(move || text_key("dashboard.notLoggedIn"))
                    .text_sm()
                    .text_color(move || nsh_base::theme().muted_foreground)
            },
        ))
        .child(each(
            move || {
                status
                    .get()
                    .filter(|panel| panel.online)
                    .map(|panel| shown_operations(&panel.operations))
                    .unwrap_or_default()
            },
            |row: &OpRow| row.key.clone(),
            |row, _| {
                let row = row.get_untracked();
                metric_text(
                    SharedString::from(
                        row.label.map(text_key).unwrap_or_else(|| SharedString::from(row.key)),
                    ),
                    SharedString::from(row.count.to_string()),
                )
            },
        ))
}

fn account_platform(page: DashboardPage) -> SharedString {
    let Some(id) = page.account_id.get() else {
        return text_key("dashboard.dash");
    };
    page.rows
        .get()
        .as_ref()
        .and_then(|rows| selected_account(rows, Some(&id)))
        .filter(|row| !row.platform.is_empty())
        .map(|row| platform_label(&row.platform))
        .unwrap_or_else(|| text_key("dashboard.dash"))
}

fn nick_text(status: Resource<PanelStatus, String>) -> SharedString {
    status
        .get()
        .and_then(|panel| {
            let nick = panel.nick.trim();
            (!nick.is_empty()).then(|| SharedString::from(nick.to_owned()))
        })
        .unwrap_or_else(|| text_key("dashboard.dash"))
}

fn uptime_text(status: Resource<PanelStatus, String>) -> SharedString {
    status
        .get()
        .as_ref()
        .and_then(shown_uptime)
        .map(|seconds| SharedString::from(uptime_copy(seconds)))
        .unwrap_or_else(|| text_key("dashboard.dash"))
}

fn shown_uptime(panel: &PanelStatus) -> Option<i64> {
    panel.online.then_some(i64::try_from(panel.uptime.unwrap_or(0)).unwrap_or(i64::MAX))
}

fn uptime_copy(seconds: i64) -> String {
    let (days, clock) = clock_text(seconds);
    if days > 0 {
        text_key("dashboard.days").replace("{days}", &days.to_string()).replace("{clock}", &clock)
    } else {
        clock
    }
}

fn clock_text(seconds: i64) -> (i64, String) {
    let seconds = seconds.max(0);
    let days = seconds / 86_400;
    let rest = seconds % 86_400;
    let hours = rest / 3600;
    let minutes = rest % 3600 / 60;
    let secs = rest % 60;
    (days, format!("{hours:02}:{minutes:02}:{secs:02}"))
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
    metric_text(text_key(label), value)
}

fn metric_text(
    label: impl IntoReactive<SharedString> + 'static,
    value: impl IntoReactive<SharedString> + 'static,
) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_4()
        .child(text(label).text_sm())
        .child(text(value).text_sm().font_weight(FontWeight::MEDIUM))
}

fn toggle(page: DashboardPage, status: Resource<PanelStatus, String>) -> impl IntoElement {
    each(
        move || current_running(page).into_iter().collect::<Vec<_>>(),
        |active: &bool| active.to_string(),
        move |active, _| command_button(page, status, active.get_untracked()),
    )
}

const NORMAL_FERTILIZER: i64 = 1011;
const ORGANIC_FERTILIZER: i64 = 1012;
const NORMAL_COLLECTION: i64 = 3001;
const RARE_COLLECTION: i64 = 3002;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Inventory {
    normal_fertilizer: String,
    organic_fertilizer: String,
    normal_collection: i64,
    rare_collection: i64,
}

struct BagRow {
    id: i64,
    count: i64,
    hours_text: String,
}

fn inventory_from(bag: &qq_farm_core::services::warehouse::BagDetail) -> Inventory {
    let rows = bag
        .items
        .iter()
        .map(|item| BagRow { id: item.id, count: item.count, hours_text: item.hours_text.clone() })
        .chain(bag.system_items.iter().map(|item| BagRow {
            id: item.id,
            count: item.count,
            hours_text: item.hours_text.clone(),
        }))
        .collect::<Vec<_>>();
    Inventory {
        normal_fertilizer: bucket_time(&rows, NORMAL_FERTILIZER),
        organic_fertilizer: bucket_time(&rows, ORGANIC_FERTILIZER),
        normal_collection: item_count(&rows, NORMAL_COLLECTION),
        rare_collection: item_count(&rows, RARE_COLLECTION),
    }
}

fn bucket_time(rows: &[BagRow], id: i64) -> String {
    let Some(item) = rows.iter().find(|row| row.id == id) else {
        return "0.0h".to_owned();
    };
    if !item.hours_text.is_empty() {
        return item.hours_text.replace("小时", "h");
    }
    format!("{:.1}h", item.count as f64 / 3600.0)
}

fn item_count(rows: &[BagRow], id: i64) -> i64 {
    rows.iter().find(|row| row.id == id).map(|row| row.count).unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ForecastNote {
    Waiting,
    Short,
    Ready { basis: Option<&'static str> },
    Later { minutes: i64, planted: bool, basis: Option<&'static str> },
    Plots { count: i64, exp: i64 },
    Manual,
}

fn forecast_notes(forecast: Option<&Value>) -> Vec<ForecastNote> {
    let Some(forecast) = forecast else {
        return Vec::new();
    };
    let plots = forecast.get("plotCount").and_then(Value::as_i64).unwrap_or(0);
    let automatic = forecast.get("automatic").and_then(Value::as_bool).unwrap_or(false);
    let scheduled = forecast.get("scheduledExp").and_then(Value::as_i64).unwrap_or(0);
    let basis = match forecast.get("source").and_then(Value::as_str) {
        Some("planted") => Some("dashboard.byCrops"),
        Some("ready") => Some("dashboard.readyHarvest"),
        Some("mixed") => Some("dashboard.mixedRate"),
        Some("history") => Some("dashboard.historyRate"),
        _ => None,
    };
    let mut notes = Vec::new();
    match forecast.get("estimatedAt").and_then(Value::as_i64) {
        None if plots > 0 => notes.push(ForecastNote::Short),
        None => notes.push(ForecastNote::Waiting),
        Some(_) if forecast.get("source").and_then(Value::as_str) == Some("ready") => {
            notes.push(ForecastNote::Ready { basis });
        }
        Some(estimated_at) => {
            let generated = forecast.get("generatedAt").and_then(Value::as_i64).unwrap_or(0);
            let minutes = (((estimated_at - generated).max(0) as f64) / 60_000.0).ceil().max(1.0);
            notes.push(ForecastNote::Later {
                minutes: minutes as i64,
                planted: forecast.get("source").and_then(Value::as_str) == Some("planted"),
                basis,
            });
        }
    }
    if plots > 0 {
        notes.push(ForecastNote::Plots { count: plots, exp: scheduled });
        if !automatic {
            notes.push(ForecastNote::Manual);
        }
    }
    notes
}

fn forecast_copy(note: &ForecastNote) -> String {
    match note {
        ForecastNote::Waiting => text_key("dashboard.forecastWait").to_string(),
        ForecastNote::Short => text_key("dashboard.forecastShort").to_string(),
        ForecastNote::Ready { basis } => with_basis(text_key("dashboard.forecastReady"), *basis),
        ForecastNote::Later { minutes, planted, basis } => {
            let key =
                if *planted { "dashboard.forecastHarvest" } else { "dashboard.forecastLater" };
            with_basis(text_key(key).replace("{duration}", &duration_copy(*minutes)), *basis)
        }
        ForecastNote::Plots { count, exp } => text_key("dashboard.forecastPlots")
            .replace("{plots}", &count.to_string())
            .replace("{exp}", &exp.to_string()),
        ForecastNote::Manual => text_key("dashboard.forecastManual").to_string(),
    }
}

fn with_basis(text: impl AsRef<str>, basis: Option<&'static str>) -> String {
    match basis {
        Some(key) => format!("{} · {}", text.as_ref(), text_key(key)),
        None => text.as_ref().to_owned(),
    }
}

fn duration_copy(minutes: i64) -> String {
    if minutes < 60 {
        text_key("dashboard.minutes").replace("{count}", &minutes.to_string())
    } else {
        text_key("dashboard.hours").replace("{count}", &format!("{:.1}", minutes as f64 / 60.0))
    }
}

fn forecast_key(note: &ForecastNote) -> String {
    match note {
        ForecastNote::Waiting => "wait".to_owned(),
        ForecastNote::Short => "short".to_owned(),
        ForecastNote::Ready { .. } => "ready".to_owned(),
        ForecastNote::Later { minutes, .. } => format!("later-{minutes}"),
        ForecastNote::Plots { count, .. } => format!("plots-{count}"),
        ForecastNote::Manual => "manual".to_owned(),
    }
}

fn running_id(page: DashboardPage) -> Option<String> {
    let id = page.account_id.get()?;
    page.rows.get()?.into_iter().find(|row| row.id == id && row.running).map(|row| row.id)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OpRow {
    key: String,
    label: Option<&'static str>,
    count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PatrolText {
    Offline,
    Quiet,
    Checking,
    Clock { days: i64, clock: String },
}

const OPERATIONS: &[(&str, &str)] = &[
    ("harvest", "dashboard.opHarvest"),
    ("farming", "dashboard.opFarming"),
    ("fertilize", "dashboard.opFertilize"),
    ("plant", "dashboard.opPlant"),
    ("steal", "dashboard.opSteal"),
    ("helpFarming", "dashboard.opHelp"),
    ("taskClaim", "dashboard.opTask"),
    ("sell", "dashboard.opSell"),
];

fn patrol_text(farm: bool, panel: &PanelStatus) -> PatrolText {
    let checks = panel.next_checks.as_ref();
    let (quiet, remain) = if farm {
        (check_flag(checks, &["farmQuiet"]), check_remain(checks, &["farmRemainSec"]))
    } else {
        (
            check_flag(checks, &["friendQuiet", "helpQuiet", "stealQuiet"]),
            check_remain(checks, &["friendRemainSec", "helpRemainSec", "stealRemainSec"]),
        )
    };
    if !panel.online {
        PatrolText::Offline
    } else if quiet {
        PatrolText::Quiet
    } else if remain <= 0 {
        PatrolText::Checking
    } else {
        let (days, clock) = clock_text(remain);
        PatrolText::Clock { days, clock }
    }
}

fn check_flag(checks: Option<&Value>, keys: &[&str]) -> bool {
    keys.iter().find_map(|key| checks?.get(*key).and_then(Value::as_bool)).unwrap_or(false)
}

fn check_remain(checks: Option<&Value>, keys: &[&str]) -> i64 {
    keys.iter().find_map(|key| checks?.get(*key).and_then(Value::as_i64)).unwrap_or(0)
}

fn shown_operations(operations: &std::collections::HashMap<String, i64>) -> Vec<OpRow> {
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (key, label) in OPERATIONS {
        seen.insert(*key);
        if let Some(count) = operations.get(*key).copied().filter(|count| *count > 0) {
            rows.push(OpRow { key: (*key).to_owned(), label: Some(label), count });
        }
    }
    let mut extra = operations
        .iter()
        .filter(|(key, count)| {
            **count > 0
                && !seen.contains(key.as_str())
                && key.as_str() != "upgrade"
                && key.as_str() != "levelUp"
        })
        .map(|(key, count)| OpRow { key: key.clone(), label: None, count: *count })
        .collect::<Vec<_>>();
    extra.sort_by(|left, right| left.key.cmp(&right.key));
    rows.extend(extra);
    rows
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
