//! 公共可交互 UI 部件。

pub mod activity;
pub mod admin;
pub mod analytics;
pub mod commerce;
pub mod config;
pub mod dashboard;
pub mod friends;
pub mod personal;
pub mod settings;

use gpui::*;
use serde_json::Value;

use crate::ui::*;

pub fn section_title(text: &str) -> impl IntoElement {
    div()
        .text_xl()
        .font_weight(FontWeight::SEMIBOLD)
        .mb_1()
        .child(text.to_string())
}

pub fn section_subtitle(text: &str, cx: &App) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .mb_2()
        .child(text.to_string())
}

pub fn page_header(title: &str, subtitle: &str, cx: &App) -> impl IntoElement {
    v_flex()
        .gap_1()
        .mb_1()
        .child(section_title(title))
        .child(section_subtitle(subtitle, cx))
}

pub fn stat_chip(label: &str, value: &str, cx: &App) -> impl IntoElement {
    v_flex()
        .min_w(px(108.))
        .flex_1()
        .p_3()
        .rounded_xl()
        .bg(cx.theme().secondary)
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(value.to_string()),
        )
}

pub fn panel_card(cx: &App) -> Div {
    v_flex()
        .p_5()
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .shadow_md()
}

pub fn card(cx: &App) -> Div {
    panel_card(cx).p_4()
}

pub fn empty_hint(text: &str, cx: &App) -> impl IntoElement {
    v_flex()
        .w_full()
        .items_center()
        .justify_center()
        .gap_2()
        .p_10()
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted.opacity(0.45))
        .child(
            Icon::new(IconName::Inbox)
                .text_color(cx.theme().muted_foreground)
                .into_any_element(),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .text_center()
                .child(text.to_string()),
        )
}

pub fn jstr(v: &Value, path: &str) -> String {
    v.pointer(path)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

pub fn ji64(v: &Value, path: &str) -> i64 {
    v.pointer(path)
        .and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)))
        .unwrap_or(0)
}

pub fn jbool(v: &Value, path: &str) -> bool {
    v.pointer(path).and_then(|x| x.as_bool()).unwrap_or(false)
}

pub fn format_secs(secs: i64) -> String {
    if secs <= 0 {
        return "—".into();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}时{m}分")
    } else if m > 0 {
        format!("{m}分{s}秒")
    } else {
        format!("{s}秒")
    }
}

pub fn format_amount(n: i64) -> String {
    if n.abs() >= 100_000_000 {
        format!("{:.1}亿", n as f64 / 100_000_000.0)
    } else if n.abs() >= 10_000 {
        format!("{:.1}万", n as f64 / 10_000.0)
    } else {
        n.to_string()
    }
}

pub fn land_status_label(status: &str) -> String {
    match status {
        "harvestable" => "可收获".into(),
        "growing" => "生长中".into(),
        "empty" => "空地".into(),
        "dead" => "枯死".into(),
        "locked" => "未解锁".into(),
        "stealable" => "可偷".into(),
        "harvested" => "已收".into(),
        _ => status.to_string(),
    }
}

pub fn conn_label(status: &Value) -> (&'static str, bool) {
    let connected = status
        .pointer("/connection/connected")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            status
                .pointer("/connection")
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case("connected") || s == "online")
        })
        .unwrap_or(false);
    if connected {
        ("在线", true)
    } else {
        ("离线", false)
    }
}

/// 去掉门面错误前缀，业务提示不当成系统崩了。
#[must_use]
pub fn humanize_error(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    for prefix in [
        "bad request: ",
        "Bad request: ",
        "internal: ",
        "Internal: ",
        "not found: ",
        "Not found: ",
        "forbidden: ",
        "Forbidden: ",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
            break;
        }
    }
    s
}

/// 可领取为空等业务结果 → 提示，不是致命错误。
#[must_use]
pub fn is_soft_business_message(msg: &str) -> bool {
    let m = msg.to_lowercase();
    msg.contains("没有可领取")
        || msg.contains("暂无可")
        || msg.contains("无需")
        || msg.contains("已领取")
        || m.contains("nothing to")
        || m.contains("no reward")
}
