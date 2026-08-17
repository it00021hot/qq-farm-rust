//! 好友：列表 / 黑名单 / 操作。

use gpui::*;
use serde_json::Value;

use crate::app_state::AppState;
use crate::bridge;
use crate::ui::*;
use crate::views::{card, empty_hint, ji64, jstr, section_title};

pub fn render(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let tab = state.read(cx).friends_tab;
    let account_id = state.read(cx).account_id.clone();
    let friends = state.read(cx).friends_json.clone();

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(section_title("好友"))
                .child(
                    h_flex()
                        .gap_2()
                        .child({
                            let state = state.clone();
                            Button::new("friends-sync")
                                .small()
                                .primary()
                                .label("同步好友")
                                .on_click(move |_, _, cx| {
                                    bridge::run_async(
                                        &state,
                                        cx,
                                        |app, id| async move {
                                            qq_farm_app::friend::list_friends(&app, &id, true)
                                                .await
                                                .map_err(|e| e.to_string())
                                        },
                                        |s, v, _| {
                                            s.friends_json = v;
                                            s.last_message = Some("好友已同步".into());
                                        },
                                    );
                                })
                        })
                        .child({
                            let state = state.clone();
                            Button::new("friends-clear")
                                .small()
                                .ghost()
                                .label("清缓存")
                                .on_click(move |_, _, cx| {
                                    bridge::run_sync(&state, cx, |app, id, s, cx| {
                                        qq_farm_app::friend::clear_friends_cache(app, id)
                                            .map(|_| {
                                                s.flash_success("缓存已清空", cx);
                                            })
                                            .map_err(|e| e.to_string())
                                    });
                                })
                        }),
                ),
        )
        .child(
            h_flex().gap_2().children(["好友列表", "黑名单", "已知GID"].into_iter().enumerate().map(
                |(i, label)| {
                    let state = state.clone();
                    Button::new(SharedString::from(format!("ftab-{i}")))
                        .small()
                        .selected(tab == i)
                        .label(label)
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                s.friends_tab = i;
                                cx.notify();
                            });
                        })
                },
            )),
        )
        .child(match tab {
            0 => friends_list(state, &friends, cx).into_any_element(),
            1 => blacklist_panel(state, &account_id, cx).into_any_element(),
            _ => known_gids_panel(&account_id, cx).into_any_element(),
        })
}

fn friend_name(f: &Value) -> String {
    for path in ["/name", "/nick", "/nickname", "/remark"] {
        let s = jstr(f, path);
        if !s.is_empty() {
            return s;
        }
    }
    "好友".into()
}

fn friends_list(
    state: &Entity<AppState>,
    friends: &Value,
    cx: &mut Context<impl Render>,
) -> impl IntoElement {
    let rows: Vec<Value> = if let Some(arr) = friends.as_array() {
        arr.clone()
    } else if let Some(arr) = friends.get("friends").and_then(|v| v.as_array()) {
        arr.clone()
    } else if let Some(arr) = friends.get("list").and_then(|v| v.as_array()) {
        arr.clone()
    } else {
        vec![]
    };

    if rows.is_empty() {
        return empty_hint("暂无好友。点「同步好友」拉取。", cx).into_any_element();
    }

    v_flex()
        .gap_1()
        .children(rows.into_iter().enumerate().map(|(i, f)| {
            let name = friend_name(&f);
            let gid = ji64(&f, "/gid").max(ji64(&f, "/id"));
            card(cx)
                .id(SharedString::from(format!("friend-{i}-{gid}")))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            v_flex()
                                .flex_1()
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("GID {gid}")),
                                ),
                        )
                        .children(
                            [("help", "帮忙"), ("steal", "偷菜"), ("bad", "捣乱")]
                                .into_iter()
                                .map(|(op, label)| {
                                    let state = state.clone();
                                    let op = op.to_string();
                                    Button::new(SharedString::from(format!("{op}-{gid}")))
                                        .small()
                                        .label(label)
                                        .on_click(move |_, _, cx| {
                                            let op = op.clone();
                                            let label = label.to_string();
                                            bridge::run_async(
                                                &state,
                                                cx,
                                                move |app, id| async move {
                                                    qq_farm_app::friend::friend_op(
                                                        &app, &id, gid, &op,
                                                    )
                                                    .await
                                                    .map_err(|e| e.to_string())
                                                },
                                                move |s, _, _| {
                                                    s.last_message =
                                                        Some(format!("已对 {gid} 执行{label}"));
                                                },
                                            );
                                        })
                                }),
                        )
                        .child({
                            let state = state.clone();
                            Button::new(SharedString::from(format!("bl-{gid}")))
                                .small()
                                .ghost()
                                .label("拉黑")
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| {
                                        let _ = qq_farm_app::friend::toggle_friend_blacklist(
                                            &s.account_id,
                                            gid,
                                        );
                                        s.last_message = Some(format!("已切换黑名单 {gid}"));
                                        cx.notify();
                                    });
                                })
                        }),
                )
        }))
        .into_any_element()
}

fn blacklist_panel(
    state: &Entity<AppState>,
    account_id: &str,
    cx: &mut Context<impl Render>,
) -> impl IntoElement {
    let list = if account_id.is_empty() {
        vec![]
    } else {
        qq_farm_app::friend::friend_blacklist(account_id)
            .as_array()
            .cloned()
            .unwrap_or_default()
    };
    if list.is_empty() {
        return empty_hint("黑名单为空。", cx).into_any_element();
    }
    v_flex()
        .gap_1()
        .children(list.into_iter().enumerate().map(|(i, g)| {
            let gid = g.as_i64().unwrap_or(0);
            card(cx)
                .id(SharedString::from(format!("blrow-{i}")))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(div().child(format!("GID {gid}")))
                        .child({
                            let state = state.clone();
                            Button::new(SharedString::from(format!("unbl-{gid}")))
                                .small()
                                .label("移除")
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| {
                                        let _ = qq_farm_app::friend::toggle_friend_blacklist(
                                            &s.account_id,
                                            gid,
                                        );
                                        s.last_message = Some("已更新黑名单".into());
                                        cx.notify();
                                    });
                                })
                        }),
                )
        }))
        .into_any_element()
}

fn known_gids_panel(account_id: &str, cx: &App) -> impl IntoElement {
    let settings = if account_id.is_empty() {
        Value::Null
    } else {
        qq_farm_app::friend::known_gid_settings(account_id)
    };
    let gids = settings
        .get("knownFriendGids")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!("已知好友 GID 共 {} 个", gids.len())),
        )
        .child(if gids.is_empty() {
            empty_hint("暂无 known GID。", cx).into_any_element()
        } else {
            h_flex()
                .gap_1()
                .flex_wrap()
                .children(gids.into_iter().take(100).enumerate().map(|(i, g)| {
                    div()
                        .id(SharedString::from(format!("kg-{i}")))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_xs()
                        .child(g.as_i64().unwrap_or(0).to_string())
                }))
                .into_any_element()
        })
}
