//! 本机运维：卡密 / 用户 / 系统 / 日志。

use gpui::*;

use crate::app_state::AppState;
use crate::ui::*;
use crate::views::{card, empty_hint, ji64, jstr, section_title};

pub fn render(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let tab = state.read(cx).admin_tab;

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .child(section_title("本机运维"))
                .child({
                    let state = state.clone();
                    Button::new("adm-refresh")
                        .small()
                        .primary()
                        .label("刷新")
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                s.admin_cards = qq_farm_app::admin::list_cards();
                                s.admin_users = qq_farm_app::admin::list_users();
                                s.admin_system = qq_farm_app::admin::get_system_config();
                                cx.notify();
                            });
                        })
                }),
        )
        .child(
            h_flex().gap_2().children(
                ["卡密", "用户", "系统", "登录日志"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| {
                        let state = state.clone();
                        Button::new(SharedString::from(format!("atab-{i}")))
                            .small()
                            .selected(tab == i)
                            .label(label)
                            .on_click(move |_, _, cx| {
                                state.update(cx, |s, cx| {
                                    s.admin_tab = i;
                                    cx.notify();
                                });
                            })
                    }),
            ),
        )
        .child(match tab {
            0 => cards_panel(state, cx).into_any_element(),
            1 => users_panel(state, cx).into_any_element(),
            2 => system_panel(state, cx).into_any_element(),
            _ => logs_panel(state, cx).into_any_element(),
        })
}

fn cards_panel(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let cards = state.read(cx).admin_cards.clone();
    let rows = cards.as_array().cloned().unwrap_or_default();
    v_flex()
        .gap_2()
        .child({
            let state = state.clone();
            Button::new("create-card")
                .small()
                .primary()
                .label("创建 7 天卡密")
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        let _ = qq_farm_app::admin::create_card("desktop", 7, Some("time"), Some(1));
                        s.admin_cards = qq_farm_app::admin::list_cards();
                        s.last_message = Some("已创建卡密".into());
                        cx.notify();
                    });
                })
        })
        .child(if rows.is_empty() {
            empty_hint("暂无卡密。", cx).into_any_element()
        } else {
            v_flex()
                .gap_1()
                .children(rows.into_iter().take(50).enumerate().map(|(i, c)| {
                    let code = jstr(&c, "/code");
                    let days = ji64(&c, "/days");
                    let enabled = c
                        .get("enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let desc = jstr(&c, "/description");
                    card(cx)
                        .id(SharedString::from(format!("card-{i}")))
                        .child(
                            h_flex()
                                .items_center()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(if code.is_empty() {
                                                    "卡密".into()
                                                } else {
                                                    code.clone()
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!(
                                                    "{desc} · {days}天 · {}",
                                                    if enabled { "启用" } else { "停用" }
                                                )),
                                        ),
                                )
                                .child({
                                    let state = state.clone();
                                    let code = code.clone();
                                    Button::new(SharedString::from(format!("delc-{i}")))
                                        .small()
                                        .danger()
                                        .label("删除")
                                        .on_click(move |_, _, cx| {
                                            state.update(cx, |s, cx| {
                                                let _ = qq_farm_app::admin::delete_card(&code);
                                                s.admin_cards = qq_farm_app::admin::list_cards();
                                                cx.notify();
                                            });
                                        })
                                }),
                        )
                }))
                .into_any_element()
        })
}

fn users_panel(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let users = state.read(cx).admin_users.clone();
    let rows = users.as_array().cloned().unwrap_or_default();
    if rows.is_empty() {
        return empty_hint("暂无本地用户。", cx).into_any_element();
    }
    v_flex()
        .gap_1()
        .children(rows.into_iter().enumerate().map(|(i, u)| {
            let name = jstr(&u, "/username");
            let role = jstr(&u, "/role");
            card(cx)
                .id(SharedString::from(format!("user-{i}")))
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(if role.is_empty() {
                                    "user".into()
                                } else {
                                    role
                                }),
                        ),
                )
        }))
        .into_any_element()
}

fn system_panel(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let sys = state.read(cx).admin_system.clone();
    let server = jstr(&sys, "/serverUrl")
        .pipe_or(jstr(&sys, "/server_url"))
        .pipe_or(jstr(&sys, "/saved/serverUrl"));
    let platform = jstr(&sys, "/platform").pipe_or(jstr(&sys, "/saved/platform"));
    let version = jstr(&sys, "/clientVersion").pipe_or(jstr(&sys, "/saved/clientVersion"));

    card(cx)
        .gap_2()
        .child(kv("网关", &server, cx))
        .child(kv("平台", &platform, cx))
        .child(kv("客户端版本", &version, cx))
        .child({
            let state = state.clone();
            Button::new("reset-sys")
                .small()
                .danger()
                .label("重置系统配置")
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.admin_system = qq_farm_app::admin::reset_system_config();
                        s.last_message = Some("系统配置已重置".into());
                        cx.notify();
                    });
                })
        })
}

fn logs_panel(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let logs = qq_farm_app::admin::login_logs(50, 0);
    let rows = logs
        .get("logs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    v_flex()
        .gap_2()
        .child({
            let state = state.clone();
            Button::new("clear-ll")
                .small()
                .danger()
                .label("清空登录日志")
                .on_click(move |_, _, cx| {
                    qq_farm_app::admin::clear_login_logs();
                    state.update(cx, |s, cx| {
                        s.last_message = Some("登录日志已清空".into());
                        cx.notify();
                    });
                })
        })
        .child(if rows.is_empty() {
            empty_hint("暂无登录日志。", cx).into_any_element()
        } else {
            v_flex()
                .gap_1()
                .children(rows.into_iter().enumerate().map(|(i, l)| {
                    let user = jstr(&l, "/username");
                    let time = jstr(&l, "/time").pipe_or(jstr(&l, "/createdAt"));
                    let ok = l.get("success").and_then(|v| v.as_bool()).unwrap_or(true);
                    card(cx)
                        .id(SharedString::from(format!("ll-{i}")))
                        .child(
                            h_flex()
                                .justify_between()
                                .child(div().child(user))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if ok {
                                            cx.theme().success
                                        } else {
                                            cx.theme().danger
                                        })
                                        .child(if ok { "成功" } else { "失败" }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(time),
                                ),
                        )
                }))
                .into_any_element()
        })
}

fn kv(k: &str, v: &str, cx: &App) -> impl IntoElement {
    h_flex()
        .gap_2()
        .child(
            div()
                .w(px(100.))
                .text_color(cx.theme().muted_foreground)
                .child(k.to_string()),
        )
        .child(div().child(if v.is_empty() { "—".into() } else { v.to_string() }))
}

trait PipeOr {
    fn pipe_or(self, other: String) -> String;
}
impl PipeOr for String {
    fn pipe_or(self, other: String) -> String {
        if self.is_empty() {
            other
        } else {
            self
        }
    }
}
