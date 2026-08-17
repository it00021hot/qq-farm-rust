//! 概览：对齐 web Dashboard 信息架构与交互密度。

use gpui::*;
use gpui_component::progress::Progress;
use serde_json::Value;

use crate::app_state::AppState;
use crate::bridge;
use crate::ui::*;
use crate::views::{
    conn_label, empty_hint, format_amount, ji64, jstr, panel_card, section_subtitle,
};

pub fn render(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let s = state.read(cx);
    let status = s.status_json.clone();
    let logs = s.logs_json.clone();
    let bag = s.bag_json.clone();
    let account_id = s.account_id.clone();
    let accounts = s.accounts_json.clone();
    let log_mod = s.log_filter_module;

    let (display_name, remark) = account_display(&status, &accounts, &account_id);
    let level = ji64(&status, "/status/level");
    let gold = ji64(&status, "/status/gold");
    let gold_bean = ji64(&status, "/status/goldBean");
    let coupon = ji64(&status, "/status/coupon");
    let diamond = ji64(&status, "/status/diamond")
        .max(ji64(&status, "/status/diamonds"))
        .max(ji64(&status, "/diamondBalance"));
    let exp_cur = ji64(&status, "/levelProgress/current");
    let exp_need = ji64(&status, "/levelProgress/needed");
    let uptime = ji64(&status, "/uptime");
    let session_exp = ji64(&status, "/sessionExpGained");
    let session_gold = ji64(&status, "/sessionGoldGained");
    let session_coupon = ji64(&status, "/sessionCouponGained");
    let (conn_text, online) = conn_label(&status);
    let _running = account_running(&accounts, &account_id);

    let exp_ratio = if exp_need > 0 {
        (exp_cur as f32 / exp_need as f32 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let exp_rate = exp_rate_text(session_exp, uptime);
    let time_to_level = time_to_level_text(session_exp, uptime, exp_cur, exp_need);

    let fert_n = bag_item_hours(&bag, 1011);
    let fert_o = bag_item_hours(&bag, 1012);
    let col_n = bag_item_count(&bag, 3001);
    let col_r = bag_item_count(&bag, 3002);

    let farm_cd = format_remain(ji64(&status, "/nextChecks/farmRemainSec"), online);
    let help_cd = format_remain(ji64(&status, "/nextChecks/helpRemainSec"), online);
    let steal_cd = format_remain(ji64(&status, "/nextChecks/stealRemainSec"), online);

    let ops = ordered_ops(&status);
    let log_rows = filter_logs(&logs, log_mod);

    v_flex()
        .gap_4()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(
                    v_flex()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("农场概览"),
                        )
                        .child(section_subtitle(
                            "状态、资产、巡查倒计时与运行日志",
                            cx,
                        )),
                )
                .when(!online && !account_id.is_empty(), |el| {
                    el.child({
                        let state = state.clone();
                        Button::new("dash-relogin")
                            .primary()
                            .label("扫码重新登录")
                            .on_click(move |_, window, cx| {
                                state.update(cx, |s, cx| {
                                    let remark = s
                                        .accounts_json
                                        .get("accounts")
                                        .and_then(|a| a.as_array())
                                        .into_iter()
                                        .flatten()
                                        .find(|a| {
                                            a.get("id").and_then(|v| v.as_str())
                                                == Some(s.account_id.as_str())
                                        })
                                        .and_then(|a| {
                                            a.get("name")
                                                .and_then(|v| v.as_str())
                                                .map(|n| n.to_string())
                                        });
                                    s.settings_tab = 0;
                                    s.set_page(crate::app_state::NavPage::Settings, cx);
                                    s.open_add_account(remark, window, cx);
                                });
                            })
                    })
                }),
        )
        // —— 顶栏三卡：等宽等高 ——
        .child(
            h_flex()
                .w_full()
                .gap_3()
                .child(
                    account_card(
                        &display_name,
                        remark.as_deref(),
                        level,
                        exp_cur,
                        exp_need,
                        exp_ratio,
                        &exp_rate,
                        &time_to_level,
                        session_exp,
                        cx,
                    )
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(208.)),
                )
                .child(
                    assets_card(
                        gold,
                        coupon,
                        gold_bean,
                        diamond,
                        session_gold,
                        session_coupon,
                        online,
                        conn_text,
                        uptime,
                        cx,
                    )
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(208.)),
                )
                .child(
                    resources_card(&fert_n, &fert_o, col_n, col_r, cx)
                        .flex_1()
                        .min_w(px(0.))
                        .min_h(px(208.)),
                ),
        )
        // —— 下方：日志与右侧栏底边对齐 ——
        .child(
            h_flex()
                .w_full()
                .gap_3()
                .min_h(px(480.))
                .child(
                    panel_card(cx)
                        .flex_1()
                        .min_w(px(0.))
                        .h_full()
                        .gap_3()
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .flex_shrink_0()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("运行日志"),
                                )
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .children(
                                            [
                                                ("全部", 0usize),
                                                ("农场", 1),
                                                ("好友", 2),
                                                ("系统", 3),
                                            ]
                                            .into_iter()
                                            .map(|(label, i)| {
                                                let state = state.clone();
                                                Button::new(SharedString::from(format!(
                                                    "logmod-{i}"
                                                )))
                                                .xsmall()
                                                .selected(log_mod == i)
                                                .label(label)
                                                .on_click(move |_, _, cx| {
                                                    state.update(cx, |s, cx| {
                                                        s.log_filter_module = i;
                                                        cx.notify();
                                                    });
                                                })
                                            }),
                                        )
                                        .child(
                                            Button::new("clear-logs")
                                                .xsmall()
                                                .ghost()
                                                .label("清空")
                                                .on_click({
                                                    let state = state.clone();
                                                    move |_, _, cx| {
                                                        state.update(cx, |s, cx| {
                                                            let id = s.account_id.clone();
                                                            qq_farm_app::farm::clear_global_logs(
                                                                &s.app,
                                                                if id.is_empty() {
                                                                    None
                                                                } else {
                                                                    Some(id.as_str())
                                                                },
                                                            );
                                                            s.refresh_sync();
                                                            s.flash_success("日志已清空", cx);
                                                        });
                                                    }
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .id("dashboard-logs")
                                .flex_1()
                                .min_h(px(0.))
                                .w_full()
                                .overflow_y_scroll()
                                .p_3()
                                .rounded_lg()
                                .bg(cx.theme().muted)
                                .child(if log_rows.is_empty() {
                                    div()
                                        .size_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(if online {
                                            "暂无日志".to_string()
                                        } else {
                                            "账号未运行。点上方「扫码重新登录」后，日志会出现在这里。"
                                                .to_string()
                                        })
                                        .into_any_element()
                                } else {
                                    v_flex()
                                        .gap_0p5()
                                        .children(log_rows.into_iter().enumerate().map(
                                            |(i, (time, tag, msg))| {
                                                h_flex()
                                                    .id(SharedString::from(format!("log-{i}")))
                                                    .gap_2()
                                                    .px_1()
                                                    .py_0p5()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(format!(
                                                                "[{}]",
                                                                short_time(&time)
                                                            )),
                                                    )
                                                    .child(log_tag_badge(&tag, cx))
                                                    .child(div().flex_1().text_sm().child(msg))
                                            },
                                        ))
                                        .into_any_element()
                                }),
                        ),
                )
                .child(
                    v_flex()
                        .w(px(300.))
                        .flex_shrink_0()
                        .h_full()
                        .gap_3()
                        .child(
                            panel_card(cx)
                                .gap_3()
                                .flex_shrink_0()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("下次巡查倒计时"),
                                )
                                .child(countdown_row("下次农场巡查", &farm_cd, cx))
                                .child(countdown_row("下次帮助", &help_cd, cx))
                                .child(countdown_row("下次偷菜", &steal_cd, cx)),
                        )
                        .child(
                            panel_card(cx)
                                .flex_1()
                                .min_h(px(0.))
                                .gap_2()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("今日统计"),
                                )
                                .child(if !online {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap_1()
                                        .text_center()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("账号未登录"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("请先扫码登录账号"),
                                        )
                                        .into_any_element()
                                } else if ops.is_empty() {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("暂无操作统计")
                                        .into_any_element()
                                } else {
                                    v_flex()
                                        .flex_1()
                                        .gap_1()
                                        .children(ops.into_iter().map(|(k, n)| {
                                            h_flex()
                                                .justify_between()
                                                .items_center()
                                                .px_2()
                                                .py_1p5()
                                                .rounded_lg()
                                                .bg(cx.theme().secondary)
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(op_label(&k)),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(n.to_string()),
                                                )
                                        }))
                                        .into_any_element()
                                }),
                        ),
                ),
        )
}

fn account_card(
    name: &str,
    remark: Option<&str>,
    level: i64,
    exp_cur: i64,
    exp_need: i64,
    exp_ratio: f32,
    exp_rate: &str,
    time_to_level: &str,
    session_exp: i64,
    cx: &App,
) -> Div {
    panel_card(cx)
        .h_full()
        .justify_between()
        .gap_2()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("账号"),
                )
                .child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .bg(cx.theme().accent)
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().accent_foreground)
                        .child(format!("Lv.{level}")),
                ),
        )
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .child(name.to_string()),
                )
                .when_some(remark.map(|s| s.to_string()), |el, r| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("备注：{r}")),
                    )
                }),
        )
        .child(
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("EXP"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .child(format!("{exp_cur} / {exp_need}")),
                        ),
                )
                .child(Progress::new().value(exp_ratio))
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("效率: {exp_rate}")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(time_to_level.to_string()),
                        ),
                )
                .when(session_exp != 0, |el| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().success)
                            .child(format!("今日经验 +{session_exp}")),
                    )
                }),
        )
}

fn assets_card(
    gold: i64,
    coupon: i64,
    gold_bean: i64,
    diamond: i64,
    session_gold: i64,
    session_coupon: i64,
    online: bool,
    conn_text: &str,
    uptime: i64,
    cx: &App,
) -> Div {
    panel_card(cx)
        .h_full()
        .justify_between()
        .gap_2()
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap_3()
                .child(asset_cell("金币", &format_amount(gold), session_gold, cx.theme().warning, cx))
                .child(asset_cell(
                    "点券",
                    &format_amount(coupon),
                    session_coupon,
                    cx.theme().success,
                    cx,
                ))
                .child(asset_cell(
                    "金豆豆",
                    &format_amount(gold_bean),
                    0,
                    cx.theme().warning,
                    cx,
                ))
                .child(asset_cell(
                    "钻石",
                    &format_amount(diamond),
                    0,
                    cx.theme().info,
                    cx,
                )),
        )
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .pt_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div().size_2().rounded_full().bg(if online {
                                cx.theme().success
                            } else {
                                cx.theme().danger
                            }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(conn_text.to_string()),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format_uptime(uptime)),
                ),
        )
}

fn resources_card(fert_n: &str, fert_o: &str, col_n: i64, col_r: i64, cx: &App) -> Div {
    panel_card(cx)
        .h_full()
        .justify_between()
        .gap_2()
        .child(
            v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("化肥容器"),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .child(kv("普通", fert_n, cx))
                        .child(kv("有机", fert_o, cx)),
                ),
        )
        .child(
            v_flex()
                .gap_2()
                .pt_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("收藏点"),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .child(kv("普通", &col_n.to_string(), cx))
                        .child(kv("典藏", &col_r.to_string(), cx)),
                ),
        )
}

fn asset_cell(
    label: &str,
    value: &str,
    delta: i64,
    color: Hsla,
    cx: &App,
) -> impl IntoElement {
    v_flex()
        .gap_0p5()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(value.to_string()),
        )
        .when(delta != 0, |el| {
            el.child(
                div()
                    .text_xs()
                    .text_color(if delta > 0 {
                        cx.theme().success
                    } else {
                        cx.theme().danger
                    })
                    .child(format!(
                        "{}{delta}",
                        if delta > 0 { "+" } else { "" }
                    )),
            )
        })
}

fn kv(label: &str, value: &str, cx: &App) -> impl IntoElement {
    v_flex()
        .gap_0p5()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(value.to_string()),
        )
}

fn countdown_row(label: &str, value: &str, cx: &App) -> impl IntoElement {
    h_flex()
        .justify_between()
        .items_center()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .font_family("ui-monospace")
                .child(value.to_string()),
        )
}

fn log_tag_badge(tag: &str, cx: &App) -> impl IntoElement {
    let (bg, fg) = match tag {
        "错误" | "Error" | "error" => (cx.theme().danger, cx.theme().danger_foreground),
        "农场" | "farm" => (cx.theme().success, cx.theme().success_foreground),
        "好友" | "friend" => (cx.theme().info, cx.theme().info_foreground),
        _ => (cx.theme().accent, cx.theme().accent_foreground),
    };
    div()
        .px_1p5()
        .py_0p5()
        .rounded_full()
        .bg(bg)
        .text_color(fg)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .child(if tag.is_empty() {
            "系统".to_string()
        } else {
            tag.to_string()
        })
}

fn account_display(status: &Value, accounts: &Value, account_id: &str) -> (String, Option<String>) {
    let game_name = jstr(status, "/status/name");
    let (remark, nick) = accounts
        .get("accounts")
        .and_then(|a| a.as_array())
        .into_iter()
        .flatten()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(account_id))
        .map(|a| {
            (
                a.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                a.get("nick")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            )
        })
        .unwrap_or_default();

    if !game_name.is_empty() {
        if !remark.is_empty() && remark != game_name {
            return (format!("{game_name}（{remark}）"), Some(remark));
        }
        return (game_name, None);
    }
    if !nick.is_empty() {
        if !remark.is_empty() && remark != nick {
            return (format!("{nick}（{remark}）"), Some(remark));
        }
        return (nick, None);
    }
    if !remark.is_empty() {
        return (remark, None);
    }
    if account_id.is_empty() {
        ("未选择账号".into(), None)
    } else {
        ("未登录".into(), None)
    }
}

fn account_running(accounts: &Value, account_id: &str) -> bool {
    accounts
        .get("accounts")
        .and_then(|a| a.as_array())
        .into_iter()
        .flatten()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(account_id))
        .and_then(|a| a.get("running").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

fn bag_items(bag: &Value) -> Vec<&Value> {
    bag.get("items")
        .or_else(|| bag.get("bag"))
        .or_else(|| bag.pointer("/data/items"))
        .and_then(|v| v.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

fn bag_item_by_id<'a>(bag: &'a Value, id: i64) -> Option<&'a Value> {
    bag_items(bag).into_iter().find(|it| {
        ji64(it, "/id") == id || ji64(it, "/itemId") == id || ji64(it, "/seed_id") == id
    })
}

fn bag_item_hours(bag: &Value, id: i64) -> String {
    let Some(it) = bag_item_by_id(bag, id) else {
        return "0.0h".into();
    };
    let hours = jstr(it, "/hoursText");
    if !hours.is_empty() {
        return hours.replace("小时", "h");
    }
    let count = ji64(it, "/count").max(ji64(it, "/num"));
    format!("{:.1}h", count as f64 / 3600.0)
}

fn bag_item_count(bag: &Value, id: i64) -> i64 {
    bag_item_by_id(bag, id)
        .map(|it| ji64(it, "/count").max(ji64(it, "/num")))
        .unwrap_or(0)
}

fn format_remain(secs: i64, online: bool) -> String {
    if !online {
        return "账号未登录".into();
    }
    if secs <= 0 {
        return "巡查中…".into();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

fn format_uptime(secs: i64) -> String {
    if secs <= 0 {
        return "0分".into();
    }
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{d}天 {h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}")
    }
}

fn exp_rate_text(gain: i64, uptime: i64) -> String {
    if uptime <= 0 {
        return "0/时".into();
    }
    let hours = uptime as f64 / 3600.0;
    let rate = if hours > 0.0 {
        gain as f64 / hours
    } else {
        0.0
    };
    format!("{}/时", rate.floor() as i64)
}

fn time_to_level_text(gain: i64, uptime: i64, current: i64, needed: i64) -> String {
    if needed <= 0 || uptime <= 0 || gain <= 0 {
        return String::new();
    }
    let hours = uptime as f64 / 3600.0;
    let rate = if hours > 0.0 {
        gain as f64 / hours
    } else {
        0.0
    };
    if rate <= 0.0 {
        return String::new();
    }
    let remain = (needed - current).max(0) as f64;
    let mins = remain / (rate / 60.0);
    if mins < 60.0 {
        format!("约 {} 分钟后升级", mins.ceil() as i64)
    } else {
        format!("约 {:.1} 小时后升级", mins / 60.0)
    }
}

fn ordered_ops(status: &Value) -> Vec<(String, i64)> {
    const ORDER: &[&str] = &[
        "farming",
        "harvest",
        "plant",
        "steal",
        "fertilize",
        "helpFarming",
        "sell",
        "taskClaim",
        "upgrade",
        "levelUp",
    ];
    let Some(obj) = status.get("operations").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for k in ORDER {
        if let Some(n) = obj.get(*k).and_then(|v| v.as_i64()) {
            if n != 0 || true {
                out.push(((*k).to_string(), n));
            }
        }
    }
    for (k, v) in obj {
        if ORDER.contains(&k.as_str()) {
            continue;
        }
        if let Some(n) = v.as_i64() {
            out.push((k.clone(), n));
        }
    }
    out
}

fn filter_logs(logs: &Value, module: usize) -> Vec<(String, String, String)> {
    logs.as_array()
        .into_iter()
        .flatten()
        .rev()
        .filter(|l| {
            let tag = jstr(l, "/tag").to_lowercase();
            let msg = jstr(l, "/msg").to_lowercase();
            match module {
                1 => tag.contains("农场") || tag.contains("farm") || msg.contains("农场"),
                2 => tag.contains("好友") || tag.contains("friend") || msg.contains("好友"),
                3 => {
                    tag.contains("系统")
                        || tag.contains("system")
                        || tag.contains("错误")
                        || tag.is_empty()
                }
                _ => true,
            }
        })
        .take(80)
        .map(|l| (jstr(l, "/time"), jstr(l, "/tag"), jstr(l, "/msg")))
        .collect()
}

fn short_time(time: &str) -> String {
    if time.len() >= 8 {
        time[time.len().saturating_sub(8)..].to_string()
    } else {
        time.to_string()
    }
}

fn op_label(key: &str) -> String {
    match key {
        "harvest" => "收获".into(),
        "farming" | "clear" => "一键务农".into(),
        "fertilize" => "施肥".into(),
        "plant" => "种植".into(),
        "steal" => "偷菜".into(),
        "helpFarming" | "help" => "帮忙".into(),
        "taskClaim" | "tasks" => "任务".into(),
        "sell" => "出售".into(),
        "upgrade" => "升级地".into(),
        "levelUp" => "升级".into(),
        other => other.to_string(),
    }
}
