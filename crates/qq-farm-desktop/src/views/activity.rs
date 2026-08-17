//! 活动中心：分 Tab 操作（对齐 web ActivityCenter 的分区，而非一排裸按钮）。

use gpui::*;

use crate::app_state::AppState;
use crate::bridge;
use crate::ui::*;
use crate::views::{empty_hint, ji64, jstr, panel_card, section_subtitle, section_title};

pub fn render(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let tab = state.read(cx).activity_tab;
    let activity = state.read(cx).activity_json.clone();
    let season_title = jstr(&activity, "/season/title")
        .pipe_nonempty()
        .or_else(|| jstr(&activity, "/season/name").pipe_nonempty())
        .or_else(|| jstr(&activity, "/title").pipe_nonempty())
        .unwrap_or_else(|| "当前活动".into());
    let star = ji64(&activity, "/starSand")
        .max(ji64(&activity, "/currency"))
        .max(ji64(&activity, "/season/starSand"));

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .items_end()
                .child(
                    v_flex()
                        .child(section_title("活动中心"))
                        .child(section_subtitle(&format!("赛季：{season_title}"), cx)),
                )
                .child({
                    let state = state.clone();
                    Button::new("act-refresh")
                        .small()
                        .primary()
                        .label("刷新")
                        .on_click(move |_, _, cx| {
                            bridge::run_async(
                                &state,
                                cx,
                                |app, id| async move {
                                    qq_farm_app::activity::snapshot(&app, &id)
                                        .await
                                        .map_err(|e| e.to_string())
                                },
                                |s, v, cx| {
                                    s.activity_json = v;
                                    s.flash_success("活动已刷新", cx);
                                },
                            );
                        })
                }),
        )
        .child(
            h_flex().gap_2().children(
                [
                    ("游记战令", 0usize),
                    ("星座", 1),
                    ("青梅酿造", 2),
                    ("星砂商店", 3),
                ]
                .into_iter()
                .map(|(label, i)| {
                    let state = state.clone();
                    Button::new(SharedString::from(format!("act-tab-{i}")))
                        .small()
                        .selected(tab == i)
                        .label(label)
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                s.activity_tab = i;
                                cx.notify();
                            });
                        })
                }),
            ),
        )
        .child(
            panel_card(cx)
                .gap_3()
                .child(match tab {
                    0 => travel_tab(state, cx).into_any_element(),
                    1 => constellation_tab(state, cx).into_any_element(),
                    2 => qingmei_tab(state, cx).into_any_element(),
                    _ => shop_tab(state, star, cx).into_any_element(),
                }),
        )
        .child(if activity.is_null() {
            empty_hint("暂无活动快照。请确认账号已启动，再点刷新。", cx).into_any_element()
        } else {
            div().into_any_element()
        })
}

fn travel_tab(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let _ = cx;
    v_flex()
        .gap_2()
        .child(div().font_weight(FontWeight::SEMIBOLD).child("游记战令"))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("一键领取当前可领的游记/战令奖励。若提示无可领取，属于正常状态。"),
        )
        .child(action_btn(
            state,
            "claim-pass",
            "领取战令奖励",
            Op::ClaimPass,
        ))
}

fn constellation_tab(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(div().font_weight(FontWeight::SEMIBOLD).child("点亮星座"))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("按当前进度点亮星座节点。"),
        )
        .child(action_btn(
            state,
            "light-star",
            "点亮星座",
            Op::Light,
        ))
}

fn qingmei_tab(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(div().font_weight(FontWeight::SEMIBOLD).child("青梅酿造"))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("领取每日种子，继续酿造或结算。"),
        )
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(action_btn(state, "qm-seed", "领取青梅种子", Op::QingmeiSeed))
                .child(action_btn(state, "qm-c", "酿造继续", Op::BrewContinue))
                .child(action_btn(state, "qm-s", "酿造结算", Op::BrewSettle)),
        )
}

fn shop_tab(state: &Entity<AppState>, star: i64, cx: &App) -> impl IntoElement {
    let _ = state;
    v_flex()
        .gap_2()
        .child(div().font_weight(FontWeight::SEMIBOLD).child("星砂商店"))
        .child(
            div()
                .text_sm()
                .child(format!("当前星砂 / 相关货币：{star}")),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("商店兑换列表会在后续补齐商品卡片；当前可先刷新活动快照看余额。"),
        )
}

fn action_btn(
    state: &Entity<AppState>,
    id: &'static str,
    label: &'static str,
    op: Op,
) -> impl IntoElement {
    let state = state.clone();
    Button::new(SharedString::from(format!("actbtn-{id}")))
        .primary()
        .label(label)
        .on_click(move |_, _, cx| {
            let label = label.to_string();
            bridge::run_async(
                &state,
                cx,
                move |app, id| async move {
                    match op {
                        Op::ClaimPass => {
                            qq_farm_app::activity::claim_battle_pass(&app, &id).await
                        }
                        Op::Light => {
                            qq_farm_app::activity::light_constellation(&app, &id).await
                        }
                        Op::QingmeiSeed => {
                            qq_farm_app::activity::claim_qingmei_seed(&app, &id).await
                        }
                        Op::BrewContinue => {
                            qq_farm_app::activity::continue_qingmei_brew(&app, &id).await
                        }
                        Op::BrewSettle => {
                            qq_farm_app::activity::settle_qingmei_brew(&app, &id).await
                        }
                    }
                    .map_err(|e| e.to_string())
                },
                move |s, _, cx| {
                    s.flash_success(format!("已执行：{label}"), cx);
                    s.refresh_async(cx);
                },
            );
        })
}

#[derive(Clone, Copy)]
enum Op {
    ClaimPass,
    Light,
    QingmeiSeed,
    BrewContinue,
    BrewSettle,
}

trait PipeNonEmpty {
    fn pipe_nonempty(self) -> Option<String>;
}
impl PipeNonEmpty for String {
    fn pipe_nonempty(self) -> Option<String> {
        if self.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}
