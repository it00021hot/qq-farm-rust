//! 分析：作物排名表 + 黑名单。

use gpui::*;

use crate::app_state::AppState;
use crate::ui::*;
use crate::views::{card, empty_hint, ji64, jstr, section_title};

pub fn render(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let analytics = state.read(cx).analytics_json.clone();
    let account_id = state.read(cx).account_id.clone();
    let blacklist = if account_id.is_empty() {
        vec![]
    } else {
        qq_farm_app::farm::plant_blacklist(&account_id)
            .as_array()
            .cloned()
            .unwrap_or_default()
    };

    let rows = analytics.as_array().cloned().unwrap_or_default();

    v_flex()
        .gap_3()
        .child(section_title("分析"))
        .child(
            h_flex()
                .gap_2()
                .children(
                    [("exp", "按经验"), ("profit", "按利润"), ("fert", "按化肥")]
                        .into_iter()
                        .map(|(key, label)| {
                            let state = state.clone();
                            let key = key.to_string();
                            Button::new(SharedString::from(format!("an-{key}")))
                                .small()
                                .label(label)
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| {
                                        s.analytics_json =
                                            qq_farm_app::farm::analytics(Some(&key));
                                        cx.notify();
                                    });
                                })
                        }),
                ),
        )
        .child(if rows.is_empty() {
            empty_hint("暂无排名数据。", cx).into_any_element()
        } else {
            v_flex()
                .gap_1()
                .children(rows.into_iter().take(30).enumerate().map(|(i, row)| {
                    let name = jstr(&row, "/name")
                        .pipe_nonempty()
                        .or_else(|| Some(jstr(&row, "/plantName")))
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("作物#{}", ji64(&row, "/seedId")));
                    let exp = ji64(&row, "/exp").max(ji64(&row, "/expPerHour"));
                    let profit = ji64(&row, "/profit").max(ji64(&row, "/goldPerHour"));
                    card(cx)
                        .id(SharedString::from(format!("rank-{i}")))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .w(px(28.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{}", i + 1)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .child(format!("经验 {exp}")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .child(format!("利润 {profit}")),
                                ),
                        )
                }))
                .into_any_element()
        })
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("偷菜黑名单（{}）", blacklist.len())),
                )
                .child({
                    let state = state.clone();
                    Button::new("clear-pbl")
                        .small()
                        .danger()
                        .label("清空")
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                let _ =
                                    qq_farm_app::farm::set_plant_blacklist(&s.account_id, vec![]);
                                s.last_message = Some("黑名单已清空".into());
                                cx.notify();
                            });
                        })
                }),
        )
        .child(if blacklist.is_empty() {
            empty_hint("黑名单为空。", cx).into_any_element()
        } else {
            h_flex()
                .gap_1()
                .flex_wrap()
                .children(blacklist.into_iter().enumerate().map(|(i, sid)| {
                    let seed_id = sid.as_i64().unwrap_or(0);
                    let state = state.clone();
                    Button::new(SharedString::from(format!("pbl-{i}")))
                        .small()
                        .label(format!("{seed_id} ×"))
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                let _ = qq_farm_app::farm::remove_plant_blacklist(
                                    &s.account_id,
                                    seed_id,
                                );
                                cx.notify();
                            });
                        })
                }))
                .into_any_element()
        })
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
