//! 设置：账号管理 / 策略 / 自动控制 / 离线提醒。

use gpui::*;
use gpui_component::alert::Alert;
use gpui_component::input::Input;
use gpui_component::switch::Switch;
use gpui_component::tab::{Tab, TabBar};
use qq_farm_app::accounts::UpsertAccountRequest;
use serde_json::{json, Value};

use crate::app_state::AppState;
use crate::bridge;
use crate::ui::*;
use crate::views::{card, empty_hint, jbool, jstr, page_header};

pub fn render(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let tab = state.read(cx).settings_tab;
    let state_tabs = state.clone();
    v_flex()
        .gap_4()
        .child(page_header(
            "设置",
            "账号生命周期、策略与自动控制",
            cx,
        ))
        .child(
            TabBar::new("settings-tabs")
                .pill()
                .selected_index(tab)
                .on_click(move |ix, _, cx| {
                    state_tabs.update(cx, |s, cx| {
                        s.settings_tab = *ix;
                        cx.notify();
                    });
                })
                .child(Tab::new().label("账号管理"))
                .child(Tab::new().label("策略"))
                .child(Tab::new().label("自动控制"))
                .child(Tab::new().label("用户")),
        )
        .child(match tab {
            0 => accounts_tab(state, cx).into_any_element(),
            1 => strategy_tab(state, cx).into_any_element(),
            2 => automation_tab(state, cx).into_any_element(),
            _ => user_tab(state, cx).into_any_element(),
        })
}

fn accounts_tab(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let show_add = state.read(cx).show_add_account;
    let accounts = state
        .read(cx)
        .accounts_json
        .get("accounts")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let selected = state.read(cx).account_id.clone();
    let name_input = state.read(cx).add_name_input.clone();
    let code_input = state.read(cx).add_code_input.clone();
    let login_tab = state.read(cx).add_login_tab;
    let platform = state.read(cx).add_platform.clone();
    let wx_status = state.read(cx).wx_status_text.clone();
    let wx_error = state.read(cx).wx_error.clone();
    let wx_loading = state.read(cx).wx_loading;
    let wx_qr = state.read(cx).wx_qr_image.clone();
    let relogin = state.read(cx).relogin_name.is_some();
    let total = accounts.len();

    v_flex()
        .gap_3()
        .child(
            card(cx)
                .gap_4()
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("账号管理"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("共 {total} 个账号")),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child({
                                    let state = state.clone();
                                    Button::new("acc-add")
                                        .primary()
                                        .icon(IconName::Plus)
                                        .label("新增")
                                        .on_click(move |_, window, cx| {
                                            state.update(cx, |s, cx| {
                                                s.open_add_account(None, window, cx);
                                            });
                                        })
                                })
                                .child({
                                    let state = state.clone();
                                    Button::new("acc-refresh")
                                        .outline()
                                        .icon(IconName::Loader)
                                        .label("刷新")
                                        .on_click(move |_, _, cx| {
                                            state.update(cx, |s, cx| {
                                                s.refresh_sync();
                                                s.refresh_async(cx);
                                                s.flash_success("已刷新账号列表", cx);
                                                cx.notify();
                                            });
                                        })
                                }),
                        ),
                )
                .child(
                    Alert::info(
                        "acc-code-tip",
                        "登录 code 一次性有效：停止后请扫码重新登录，不能复用旧 code 直接启动。",
                    )
                    .into_any_element(),
                )
                .when(show_add, |el| {
                    el.child(add_account_panel(
                        state,
                        &name_input,
                        &code_input,
                        login_tab,
                        &platform,
                        &wx_status,
                        wx_error.clone(),
                        wx_loading,
                        wx_qr.clone(),
                        relogin,
                        cx,
                    ))
                })
                .child(if accounts.is_empty() {
                    empty_hint(
                        "还没有账号。点右上角「新增」，推荐微信扫码登录。",
                        cx,
                    )
                    .into_any_element()
                } else {
                    account_table(state, &accounts, &selected, cx).into_any_element()
                }),
        )
}

fn add_account_panel(
    state: &Entity<AppState>,
    name_input: &Entity<gpui_component::input::InputState>,
    code_input: &Entity<gpui_component::input::InputState>,
    login_tab: usize,
    platform: &str,
    wx_status: &str,
    wx_error: Option<String>,
    wx_loading: bool,
    wx_qr: Option<std::sync::Arc<gpui::Image>>,
    relogin: bool,
    cx: &mut Context<impl Render>,
) -> impl IntoElement {
    let title = if relogin {
        "重新登录（同备注会更新并启动该账号）"
    } else {
        "新增账号"
    };
    v_flex()
        .gap_3()
        .p_4()
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().primary.opacity(0.25))
        .bg(cx.theme().accent.opacity(0.08))
        .shadow_sm()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(Tag::primary().small().rounded_full().child(if relogin {
                            "重新登录"
                        } else {
                            "新增"
                        }))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        ),
                )
                .child({
                    let state = state.clone();
                    Button::new("acc-add-close")
                        .ghost()
                        .small()
                        .icon(IconName::Close)
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| s.close_add_account(cx));
                        })
                }),
        )
        .child(Input::new(name_input))
        .child(
            TabBar::new("login-tabs")
                .segmented()
                .small()
                .selected_index(login_tab)
                .on_click({
                    let state = state.clone();
                    move |ix, _, cx| {
                        let i = *ix;
                        state.update(cx, |s, cx| {
                            s.add_login_tab = i;
                            if i == 1 {
                                s.add_platform = "wx".into();
                                if s.wx_qr_image.is_none() && !s.wx_loading {
                                    s.start_wx_login(cx);
                                }
                            } else {
                                s.reset_wx_login();
                            }
                            cx.notify();
                        });
                    }
                })
                .child(Tab::new().label("输入 code"))
                .child(Tab::new().label("微信扫码")),
        )
        .child(if login_tab == 0 {
            v_flex()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("粘贴登录 code，也可直接粘贴带 ?code= 的整段链接"),
                )
                .child(Input::new(code_input))
                .child(
                    h_flex().gap_2().children(
                        [("qq", "QQ"), ("wx", "微信小程序")].into_iter().map(|(key, label)| {
                            let state = state.clone();
                            let key = key.to_string();
                            Button::new(SharedString::from(format!("plat-{key}")))
                                .small()
                                .selected(platform == key)
                                .label(label)
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| {
                                        s.add_platform = key.clone();
                                        cx.notify();
                                    });
                                })
                        }),
                    ),
                )
                .child(
                    Button::new("add-account-code")
                        .primary()
                        .label(if relogin {
                            "更新并启动"
                        } else {
                            "添加并启动"
                        })
                        .on_click({
                            let state = state.clone();
                            move |_, _, cx| {
                                let name =
                                    state.read(cx).add_name_input.read(cx).value().to_string();
                                let mut code = state
                                    .read(cx)
                                    .add_code_input
                                    .read(cx)
                                    .value()
                                    .to_string();
                                if let Some(extracted) = extract_code(&code) {
                                    code = extracted;
                                }
                                let platform = state.read(cx).add_platform.clone();
                                if code.trim().is_empty() {
                                    state.update(cx, |s, cx| {
                                        s.flash_error("请输入 Code", cx);
                                        cx.notify();
                                    });
                                    return;
                                }
                                state.update(cx, |s, cx| {
                                    let req = UpsertAccountRequest {
                                        name: Some(if name.trim().is_empty() {
                                            format!(
                                                "账号{}",
                                                chrono::Local::now().format("%H%M%S")
                                            )
                                        } else {
                                            name.trim().to_string()
                                        }),
                                        code: Some(code.trim().to_string()),
                                        platform: Some(platform),
                                        username: Some("local".into()),
                                        ..Default::default()
                                    };
                                    match qq_farm_app::accounts::upsert_account(
                                        &s.app, &s.policy, req,
                                    ) {
                                        Ok(v) => {
                                            s.accounts_json = v.clone();
                                            if let Some(id) = v
                                                .get("accounts")
                                                .and_then(|a| a.as_array())
                                                .and_then(|arr| arr.last())
                                                .and_then(|a| a.get("id"))
                                                .and_then(|x| x.as_str())
                                            {
                                                s.account_id = id.to_string();
                                            }
                                            s.show_add_account = false;
                                            s.relogin_name = None;
                                            s.flash_success("账号已登录并启动", cx);
                                            s.refresh_async(cx);
                                        }
                                        Err(e) => s.flash_error(e.to_string(), cx),
                                    }
                                });
                            }
                        }),
                )
                .into_any_element()
        } else {
            v_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("用微信扫一扫下方二维码，手机确认后自动写入账号并启动"),
                )
                .child(
                    div()
                        .w(px(220.))
                        .h(px(220.))
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(if let Some(img_data) = wx_qr {
                            img(img_data)
                                .id("wx-qr")
                                .w(px(208.))
                                .h(px(208.))
                                .object_fit(ObjectFit::Contain)
                                .into_any_element()
                        } else if wx_loading {
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("正在获取二维码…")
                                .into_any_element()
                        } else {
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("点击下方刷新获取二维码")
                                .into_any_element()
                        }),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(if wx_status.is_empty() {
                            "等待开始扫码".to_string()
                        } else {
                            wx_status.to_string()
                        }),
                )
                .when_some(wx_error, |el, err| {
                    el.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().danger)
                            .child(err),
                    )
                })
                .child(
                    h_flex()
                        .gap_2()
                        .child({
                            let state = state.clone();
                            Button::new("wx-refresh")
                                .primary()
                                .label("刷新二维码")
                                .disabled(wx_loading)
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| {
                                        s.start_wx_login(cx);
                                    });
                                })
                        })
                        .child({
                            let state = state.clone();
                            Button::new("wx-cancel")
                                .ghost()
                                .label("取消")
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| s.close_add_account(cx));
                                })
                        }),
                )
                .into_any_element()
        })
}

fn account_table(
    state: &Entity<AppState>,
    accounts: &[Value],
    selected: &str,
    cx: &mut Context<impl Render>,
) -> impl IntoElement {
    let header = |label: &str, width: Option<Pixels>| {
        let mut d = div()
            .px_3()
            .py_2()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().muted_foreground)
            .child(label.to_string());
        if let Some(w) = width {
            d = d.w(w);
        } else {
            d = d.flex_1().min_w(px(0.));
        }
        d
    };

    v_flex()
        .w_full()
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .bg(cx.theme().background)
        .child(
            h_flex()
                .w_full()
                .items_center()
                .bg(cx.theme().muted.opacity(0.5))
                .border_b_1()
                .border_color(cx.theme().border)
                .child(header("序号", Some(px(56.))))
                .child(header("账号备注", None))
                .child(header("平台", Some(px(100.))))
                .child(header("运行状态", Some(px(110.))))
                .child(header("最近更新", Some(px(150.))))
                .child(header("操作", Some(px(300.)))),
        )
        .children(accounts.iter().enumerate().map(|(idx, acc)| {
            let id = jstr(acc, "/id");
            let name = {
                let n = jstr(acc, "/name");
                if n.is_empty() {
                    "未命名".into()
                } else {
                    n
                }
            };
            let platform_key = jstr(acc, "/platform");
            let running = jbool(acc, "/running");
            let updated = format_account_time(acc);
            let is_selected = selected == id;
            let state = state.clone();
            let id_row = id.clone();
            let name_row = name.clone();
            let avatar_name = name.clone();

            h_flex()
                .id(SharedString::from(format!("acc-tr-{id}")))
                .w_full()
                .items_center()
                .border_b_1()
                .border_color(cx.theme().border)
                .bg(if is_selected {
                    cx.theme().accent.opacity(0.12)
                } else if idx % 2 == 1 {
                    cx.theme().muted.opacity(0.25)
                } else {
                    gpui::transparent_black()
                })
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().secondary))
                .on_click({
                    let state = state.clone();
                    let id = id_row.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| s.set_account(id.clone(), cx));
                    }
                })
                .child(
                    div()
                        .w(px(56.))
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{}", idx + 1)),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_w(px(0.))
                        .px_3()
                        .py_3()
                        .items_center()
                        .gap_2()
                        .child(Avatar::new().name(avatar_name).small())
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_ellipsis()
                                .overflow_hidden()
                                .child(name),
                        ),
                )
                .child(
                    div()
                        .w(px(100.))
                        .px_3()
                        .py_3()
                        .child(platform_tag(&platform_key)),
                )
                .child(
                    div()
                        .w(px(110.))
                        .px_3()
                        .py_3()
                        .child(status_tag(running)),
                )
                .child(
                    div()
                        .w(px(150.))
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(updated),
                )
                .child(
                    h_flex()
                        .w(px(300.))
                        .px_2()
                        .py_2()
                        .gap_1()
                        .child({
                            let state = state.clone();
                            let name = name_row.clone();
                            Button::new(SharedString::from(format!("edit-{id_row}")))
                                .xsmall()
                                .outline()
                                .icon(IconName::Replace)
                                .label("编辑")
                                .on_click(move |_, window, cx| {
                                    let name = name.clone();
                                    state.update(cx, |s, cx| {
                                        s.open_add_account(Some(name), window, cx);
                                    });
                                })
                        })
                        .child(if running {
                            let state = state.clone();
                            let id = id_row.clone();
                            Button::new(SharedString::from(format!("stop-{id}")))
                                .xsmall()
                                .warning()
                                .outline()
                                .icon(IconName::CircleX)
                                .label("停止")
                                .on_click(move |_, _, cx| {
                                    let state = state.clone();
                                    let id = id.clone();
                                    bridge::run_sync(&state, cx, move |app, _, s, cx| {
                                        qq_farm_app::accounts::stop_account(app, &s.policy, &id)
                                            .map(|_| {
                                                s.flash_success("已停止", cx);
                                            })
                                            .map_err(|e| e.to_string())
                                    });
                                })
                                .into_any_element()
                        } else {
                            let state = state.clone();
                            let name = name_row.clone();
                            Button::new(SharedString::from(format!("relogin-{id_row}")))
                                .xsmall()
                                .success()
                                .outline()
                                .icon(IconName::Redo)
                                .label("重新登录")
                                .on_click(move |_, window, cx| {
                                    let name = name.clone();
                                    state.update(cx, |s, cx| {
                                        s.open_add_account(Some(name), window, cx);
                                    });
                                })
                                .into_any_element()
                        })
                        .child({
                            let state = state.clone();
                            let id = id_row.clone();
                            Button::new(SharedString::from(format!("del-{id}")))
                                .xsmall()
                                .danger()
                                .outline()
                                .icon(IconName::Delete)
                                .label("删除")
                                .on_click(move |_, _, cx| {
                                    let state = state.clone();
                                    let id = id.clone();
                                    bridge::run_sync(&state, cx, move |app, _, s, cx| {
                                        qq_farm_app::accounts::delete_account(app, &s.policy, &id)
                                            .map(|_| {
                                                s.flash_success("已删除", cx);
                                                if s.account_id == id {
                                                    s.account_id.clear();
                                                }
                                            })
                                            .map_err(|e| e.to_string())
                                    });
                                })
                        }),
                )
        }))
}

fn status_tag(running: bool) -> impl IntoElement {
    if running {
        Tag::success()
            .small()
            .rounded_full()
            .child("运行中")
            .into_any_element()
    } else {
        Tag::secondary()
            .small()
            .rounded_full()
            .child("已停止")
            .into_any_element()
    }
}

fn platform_tag(platform: &str) -> impl IntoElement {
    match platform {
        "wx" | "wechat" => Tag::info()
            .small()
            .outline()
            .rounded_full()
            .child("微信")
            .into_any_element(),
        "qq" => Tag::warning()
            .small()
            .outline()
            .rounded_full()
            .child("QQ")
            .into_any_element(),
        other if other.is_empty() => Tag::secondary()
            .small()
            .rounded_full()
            .child("—")
            .into_any_element(),
        other => Tag::secondary()
            .small()
            .rounded_full()
            .child(other.to_string())
            .into_any_element(),
    }
}

fn format_account_time(acc: &Value) -> String {
    let ts = acc
        .get("updated_at")
        .or_else(|| acc.get("updatedAt"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if ts <= 0 {
        return "—".into();
    }
    let secs = if ts > 1_000_000_000_000 {
        ts / 1000
    } else {
        ts
    };
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}

fn strategy_tab(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let settings = state.read(cx).settings_json.clone();
    let current = jstr(&settings, "/strategy");
    let preferred = settings
        .get("preferredSeed")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    v_flex()
        .gap_3()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "当前策略：{} · 指定种子 ID：{preferred}",
                    strategy_label(&current)
                )),
        )
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(
                    [
                        ("max_exp", "最大经验"),
                        ("max_profit", "最大利润"),
                        ("max_fert_exp", "化肥经验"),
                        ("max_fert_profit", "化肥利润"),
                        ("preferred", "指定种子"),
                        ("bag_priority", "背包优先"),
                        ("level", "按等级"),
                    ]
                    .into_iter()
                    .map(|(key, label)| {
                        let state = state.clone();
                        let key = key.to_string();
                        let selected = current == key;
                        Button::new(SharedString::from(format!("strat-{key}")))
                            .small()
                            .selected(selected)
                            .label(label)
                            .on_click(move |_, _, cx| {
                                let key = key.clone();
                                state.update(cx, |s, cx| {
                                    let snap = json!({ "strategy": key });
                                    match qq_farm_app::settings::save_settings(
                                        &s.app,
                                        &s.account_id,
                                        snap,
                                    ) {
                                        Ok(v) => {
                                            s.last_message =
                                                Some(format!("策略已设为 {}", strategy_label(&key)));
                                            if let Some(data) = v.get("data") {
                                                s.settings_json = data.clone();
                                            }
                                        }
                                        Err(e) => s.last_error = Some(e.to_string()),
                                    }
                                    cx.notify();
                                });
                            })
                    }),
                ),
        )
        .child(
            Button::new("save-settings-full")
                .primary()
                .label("保存全部设置")
                .on_click({
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            let snap = s.settings_json.clone();
                            match qq_farm_app::settings::save_settings(
                                &s.app,
                                &s.account_id,
                                snap,
                            ) {
                                Ok(v) => {
                                    s.last_message = Some("设置已保存".into());
                                    if let Some(data) = v.get("data") {
                                        s.settings_json = data.clone();
                                    }
                                }
                                Err(e) => s.last_error = Some(e.to_string()),
                            }
                            cx.notify();
                        });
                    }
                }),
        )
}

fn automation_tab(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let settings = state.read(cx).settings_json.clone();
    let auto = settings.get("automation").cloned().unwrap_or(json!({}));

    let switches = [
        ("farm", "自动农场"),
        ("friend", "自动好友"),
        ("task", "自动任务"),
        ("sell", "自动出售"),
        ("fertilizer", "自动买化肥"),
        ("steal", "偷菜"),
        ("help", "帮忙"),
        ("bad", "捣乱"),
    ];

    v_flex()
        .gap_2()
        .children(switches.into_iter().map(|(key, label)| {
            let checked = auto
                .get(key)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let state = state.clone();
            let key = key.to_string();
            let label_owned = label.to_string();
            card(cx)
                .id(SharedString::from(format!("auto-{key}")))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(div().child(label_owned.clone()))
                        .child(
                            Switch::new(SharedString::from(format!("sw-{key}")))
                                .checked(checked)
                                .on_click(move |checked, _, cx| {
                                    let key = key.clone();
                                    let label_owned = label_owned.clone();
                                    let checked = *checked;
                                    state.update(cx, |s, cx| {
                                        match qq_farm_app::farm::set_automation(
                                            &s.app,
                                            &s.account_id,
                                            &key,
                                            json!(checked),
                                            json!({}),
                                        ) {
                                            Ok(_) => {
                                                if let Some(obj) =
                                                    s.settings_json.get_mut("automation")
                                                {
                                                    if let Some(map) = obj.as_object_mut() {
                                                        map.insert(key.clone(), json!(checked));
                                                    }
                                                }
                                                s.last_message = Some(format!(
                                                    "{} 已{}",
                                                    label_owned,
                                                    if checked { "开启" } else { "关闭" }
                                                ));
                                            }
                                            Err(e) => s.last_error = Some(e.to_string()),
                                        }
                                        cx.notify();
                                    });
                                }),
                        ),
                )
        }))
}

fn user_tab(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let settings = state.read(cx).settings_json.clone();
    let offline = settings.get("offlineReminder").cloned().unwrap_or(Value::Null);
    let channel = jstr(&offline, "/channel");
    let endpoint = jstr(&offline, "/endpoint");

    v_flex()
        .gap_3()
        .child(
            card(cx)
                .gap_2()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("离线提醒"),
                )
                .child(
                    div()
                        .text_sm()
                        .child(format!(
                            "渠道：{}{}",
                            if channel.is_empty() { "未配置" } else { &channel },
                            if endpoint.is_empty() {
                                String::new()
                            } else {
                                format!(" · {endpoint}")
                            }
                        )),
                )
                .child(
                    Button::new("test-push")
                        .small()
                        .label("发送测试")
                        .on_click({
                            let state = state.clone();
                            move |_, _, cx| {
                                bridge::run_async(
                                    &state,
                                    cx,
                                    |_app, _id| async move {
                                        qq_farm_app::settings::test_offline_reminder(
                                            Some("local"),
                                            json!({}),
                                        )
                                        .await
                                        .map_err(|e| e.to_string())
                                    },
                                    |s, v, _| {
                                        let ok = jbool(&v, "/ok");
                                        s.last_message = Some(if ok {
                                            "测试推送已发送".into()
                                        } else {
                                            format!(
                                                "推送失败：{}",
                                                jstr(&v, "/msg")
                                            )
                                        });
                                    },
                                );
                            }
                        }),
                ),
        )
}

fn strategy_label(key: &str) -> &str {
    match key {
        "max_exp" => "最大经验",
        "max_profit" => "最大利润",
        "max_fert_exp" => "化肥经验",
        "max_fert_profit" => "化肥利润",
        "preferred" => "指定种子",
        "bag_priority" => "背包优先",
        "level" => "按等级",
        "" => "未设置",
        other => other,
    }
}

fn extract_code(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(idx) = raw.find("code=") {
        let rest = &raw[idx + 5..];
        let end = rest.find('&').unwrap_or(rest.len());
        let code = rest[..end].to_string();
        if !code.is_empty() {
            return Some(code);
        }
    }
    Some(raw.to_string())
}
