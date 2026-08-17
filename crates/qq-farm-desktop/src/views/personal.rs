//! 个人：农场地块 / 背包列表 / 任务进度。

use gpui::*;
use serde_json::Value;

use crate::app_state::AppState;
use crate::bridge;
use crate::ui::*;
use crate::views::{
    card, empty_hint, format_secs, ji64, jbool, jstr, land_status_label, section_title,
};

pub fn render(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let tab = state.read(cx).personal_tab;
    v_flex()
        .gap_3()
        .child(section_title("个人"))
        .child(
            h_flex().gap_2().children(["农场", "背包", "任务"].into_iter().enumerate().map(
                |(i, label)| {
                    let state = state.clone();
                    Button::new(SharedString::from(format!("ptab-{i}")))
                        .small()
                        .selected(tab == i)
                        .label(label)
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                s.personal_tab = i;
                                cx.notify();
                            });
                        })
                },
            )),
        )
        .child(match tab {
            0 => farm_panel(state, cx).into_any_element(),
            1 => bag_panel(state, cx).into_any_element(),
            _ => tasks_panel(state, cx).into_any_element(),
        })
}

fn farm_panel(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let lands_root = state.read(cx).lands_json.clone();
    let lands = lands_root
        .get("lands")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let summary = lands_root.get("summary").cloned().unwrap_or(Value::Null);

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(stat_mini("可收", ji64(&summary, "/harvestable"), cx))
                .child(stat_mini("生长", ji64(&summary, "/growing"), cx))
                .child(stat_mini("空地", ji64(&summary, "/empty"), cx))
                .child(stat_mini("枯死", ji64(&summary, "/dead"), cx))
                .child(stat_mini("缺水", ji64(&summary, "/needWater"), cx))
                .child(stat_mini("杂草", ji64(&summary, "/needWeed"), cx))
                .child(stat_mini("虫", ji64(&summary, "/needBug"), cx)),
        )
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(
                    [
                        ("harvest", "收获"),
                        ("clear", "一键务农"),
                        ("plant", "种植"),
                        ("upgrade", "升级土地"),
                        ("all", "一键全收"),
                    ]
                    .into_iter()
                    .map(|(op, label)| {
                        let state = state.clone();
                        let op = op.to_string();
                        Button::new(SharedString::from(format!("farm-{op}")))
                            .small()
                            .primary()
                            .label(label)
                            .on_click(move |_, _, cx| {
                                let op = op.clone();
                                let label = label.to_string();
                                bridge::run_async(
                                    &state,
                                    cx,
                                    move |app, id| async move {
                                        qq_farm_app::farm::operate(&app, &id, &op)
                                            .await
                                            .map_err(|e| e.to_string())
                                    },
                                    move |s, _v, cx| {
                                        s.last_message = Some(format!("已执行：{label}"));
                                        s.refresh_async(cx);
                                    },
                                );
                            })
                    }),
                ),
        )
        .child(if lands.is_empty() {
            empty_hint("暂无地块数据。请确认账号已启动且在线。", cx).into_any_element()
        } else {
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(lands.into_iter().enumerate().map(|(i, land)| {
                    land_card(i, &land, cx)
                }))
                .into_any_element()
        })
}

fn land_card(i: usize, land: &Value, cx: &App) -> impl IntoElement {
    let id = ji64(land, "/id");
    let status = jstr(land, "/status");
    let plant = jstr(land, "/plantName");
    let phase = jstr(land, "/phaseName");
    let level = ji64(land, "/level");
    let mature = ji64(land, "/matureInSec");
    let need_water = jbool(land, "/needWater");
    let need_weed = jbool(land, "/needWeed");
    let need_bug = jbool(land, "/needBug");
    let unlocked = jbool(land, "/unlocked");

    let title = if !plant.is_empty() {
        plant
    } else if !unlocked || status == "locked" {
        "未解锁".into()
    } else if status == "empty" {
        "空地".into()
    } else {
        land_status_label(&status).to_string()
    };

    let mut badges = Vec::new();
    if need_water {
        badges.push("水");
    }
    if need_weed {
        badges.push("草");
    }
    if need_bug {
        badges.push("虫");
    }

    card(cx)
        .id(SharedString::from(format!("land-{i}-{id}")))
        .w(px(148.))
        .min_h(px(110.))
        .gap_1()
        .child(
            h_flex()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("#{id} · Lv{level}")),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(match status.as_str() {
                            "harvestable" => cx.theme().success,
                            "dead" => cx.theme().danger,
                            "locked" => cx.theme().muted_foreground,
                            _ => cx.theme().foreground,
                        })
                        .child(land_status_label(&status).to_string()),
                ),
        )
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_sm()
                .child(title),
        )
        .when(!phase.is_empty(), |el| {
            el.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(phase),
            )
        })
        .child(
            div()
                .text_xs()
                .child(format!("成熟 {}", format_secs(mature))),
        )
        .when(!badges.is_empty(), |el| {
            el.child(
                h_flex()
                    .gap_1()
                    .children(badges.into_iter().map(|b| {
                        div()
                            .px_1()
                            .rounded_sm()
                            .bg(cx.theme().warning)
                            .text_xs()
                            .child(b.to_string())
                    })),
            )
        })
}

fn bag_panel(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let bag = state.read(cx).bag_json.clone();
    let cat = state.read(cx).bag_category;
    let items = bag
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let filtered: Vec<Value> = items
        .into_iter()
        .filter(|it| match cat {
            1 => ji64(it, "/itemType") == 6 || ji64(it, "/itemType") == 17,
            2 => ji64(it, "/itemType") == 5,
            3 => ji64(it, "/itemType") == 11,
            _ => true,
        })
        .collect();

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .gap_2()
                .children(
                    [("全部", 0), ("果实", 1), ("种子", 2), ("道具", 3)]
                        .into_iter()
                        .map(|(label, i)| {
                            let state = state.clone();
                            Button::new(SharedString::from(format!("bagcat-{i}")))
                                .small()
                                .selected(cat == i)
                                .label(label)
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| {
                                        s.bag_category = i;
                                        cx.notify();
                                    });
                                })
                        }),
                ),
        )
        .child(if filtered.is_empty() {
            empty_hint("背包为空，或当前分类没有物品。", cx).into_any_element()
        } else {
            v_flex()
                .gap_1()
                .children(filtered.into_iter().enumerate().map(|(i, item)| {
                    let name = jstr(&item, "/name");
                    let count = ji64(&item, "/count");
                    let sellable = jbool(&item, "/sellable");
                    let price = ji64(&item, "/price");
                    let unit = jstr(&item, "/priceUnit");
                    let item_id = ji64(&item, "/id");
                    let uid = ji64(&item, "/uid");
                    let interaction = jstr(&item, "/interactionType");

                    card(cx)
                        .id(SharedString::from(format!("bag-{i}-{item_id}-{uid}")))
                        .child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(if name.is_empty() {
                                                    format!("物品#{item_id}")
                                                } else {
                                                    name
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!(
                                                    "数量 {count}{}",
                                                    if sellable && price > 0 {
                                                        format!(" · 单价 {price}{unit}")
                                                    } else {
                                                        String::new()
                                                    }
                                                )),
                                        ),
                                )
                                .child({
                                    let state = state.clone();
                                    Button::new(SharedString::from(format!("use-{i}")))
                                        .small()
                                        .label("使用")
                                        .disabled(interaction.is_empty() || interaction == "none")
                                        .on_click(move |_, _, cx| {
                                            bridge::run_async(
                                                &state,
                                                cx,
                                                move |app, id| async move {
                                                    qq_farm_app::farm::bag_use(
                                                        &app, &id, item_id, 1, uid,
                                                    )
                                                    .await
                                                    .map_err(|e| e.to_string())
                                                },
                                                |s, _, cx| {
                                                    s.last_message = Some("已使用".into());
                                                    s.refresh_async(cx);
                                                },
                                            );
                                        })
                                })
                                .child({
                                    let state = state.clone();
                                    Button::new(SharedString::from(format!("sell-{i}")))
                                        .small()
                                        .danger()
                                        .label("出售")
                                        .disabled(!sellable)
                                        .on_click(move |_, _, cx| {
                                            bridge::run_async(
                                                &state,
                                                cx,
                                                move |app, id| async move {
                                                    qq_farm_app::farm::bag_sell(
                                                        &app,
                                                        &id,
                                                        &[(item_id, count.max(1), uid)],
                                                    )
                                                    .await
                                                    .map_err(|e| e.to_string())
                                                },
                                                |s, _, cx| {
                                                    s.last_message = Some("已出售".into());
                                                    s.refresh_async(cx);
                                                },
                                            );
                                        })
                                }),
                        )
                }))
                .into_any_element()
        })
}

fn tasks_panel(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let gifts = state.read(cx).gifts_json.clone();
    let gift_list = gifts
        .get("gifts")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let growth = gifts.get("growth").cloned().unwrap_or(Value::Null);
    let growth_done = jbool(&growth, "/doneToday");
    let growth_label = jstr(&growth, "/label");
    let completed = ji64(&growth, "/completedCount");
    let total = ji64(&growth, "/totalCount");

    v_flex()
        .gap_3()
        .child(
            card(cx)
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            v_flex()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(if growth_label.is_empty() {
                                            "成长任务".into()
                                        } else {
                                            growth_label
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("进度 {completed}/{total}")),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if growth_done {
                                    cx.theme().success
                                } else {
                                    cx.theme().warning
                                })
                                .child(if growth_done { "今日已完成" } else { "进行中" }),
                        ),
                ),
        )
        .child(
            v_flex()
                .gap_1()
                .children(gift_list.into_iter().enumerate().map(|(i, g)| {
                    let label = jstr(&g, "/label");
                    let done = jbool(&g, "/doneToday");
                    let c = ji64(&g, "/completedCount");
                    let t = ji64(&g, "/totalCount");
                    card(cx)
                        .id(SharedString::from(format!("gift-{i}")))
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    v_flex()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(label),
                                        )
                                        .when(t > 0, |el| {
                                            el.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(format!("{c}/{t}")),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(if done {
                                            cx.theme().success
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .child(if done { "已领" } else { "未领" }),
                                ),
                        )
                })),
        )
}

fn stat_mini(label: &str, n: i64, cx: &App) -> impl IntoElement {
    h_flex()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(cx.theme().secondary)
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(n.to_string()),
        )
}
