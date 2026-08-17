//! 商城 / 神秘商人：商品列表 + 购买。

use gpui::*;
use serde_json::Value;

use crate::app_state::AppState;
use crate::bridge;
use crate::ui::*;
use crate::views::{card, empty_hint, ji64, jstr, section_title};

pub fn render_mall(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let mall = state.read(cx).mall_json.clone();
    let products = extract_products(&mall);

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .child(section_title("游戏商城"))
                .child({
                    let state = state.clone();
                    Button::new("mall-refresh")
                        .small()
                        .primary()
                        .label("刷新")
                        .on_click(move |_, _, cx| {
                            bridge::run_async(
                                &state,
                                cx,
                                |app, id| async move {
                                    qq_farm_app::commerce::mall_catalog(&app, &id, None, None)
                                        .await
                                        .map_err(|e| e.to_string())
                                },
                                |s, v, _| {
                                    s.mall_json = v;
                                    s.last_message = Some("商城已刷新".into());
                                },
                            );
                        })
                }),
        )
        .child(if products.is_empty() {
            empty_hint("暂无商品。账号在线后点刷新。", cx).into_any_element()
        } else {
            v_flex()
                .gap_1()
                .children(products.into_iter().enumerate().map(|(i, p)| {
                    let name = product_name(&p);
                    let goods_id = ji64(&p, "/goodsId")
                        .max(ji64(&p, "/id"))
                        .max(ji64(&p, "/productId"));
                    let price = ji64(&p, "/price").max(ji64(&p, "/cost"));
                    card(cx)
                        .id(SharedString::from(format!("mall-{i}-{goods_id}")))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(name),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("ID {goods_id} · 价格 {price}")),
                                        ),
                                )
                                .child({
                                    let state = state.clone();
                                    Button::new(SharedString::from(format!("buy-{goods_id}")))
                                        .small()
                                        .primary()
                                        .label("购买")
                                        .disabled(goods_id <= 0)
                                        .on_click(move |_, _, cx| {
                                            bridge::run_async(
                                                &state,
                                                cx,
                                                move |app, id| async move {
                                                    qq_farm_app::commerce::purchase_mall(
                                                        &app, &id, goods_id as i32, 1,
                                                    )
                                                    .await
                                                    .map_err(|e| e.to_string())
                                                },
                                                |s, _, cx| {
                                                    s.last_message = Some("购买请求已发送".into());
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

pub fn render_mystery(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let mystery = state.read(cx).mystery_json.clone();
    let offers = extract_products(&mystery);

    v_flex()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .child(section_title("神秘商人"))
                .child({
                    let state = state.clone();
                    Button::new("mystery-refresh")
                        .small()
                        .primary()
                        .label("刷新")
                        .on_click(move |_, _, cx| {
                            bridge::run_async(
                                &state,
                                cx,
                                |app, id| async move {
                                    qq_farm_app::commerce::mystery_shop(&app, &id)
                                        .await
                                        .map_err(|e| e.to_string())
                                },
                                |s, v, _| {
                                    s.mystery_json = v;
                                    s.last_message = Some("神秘商人已刷新".into());
                                },
                            );
                        })
                }),
        )
        .child(if offers.is_empty() {
            empty_hint("当前没有神秘商人商品。", cx).into_any_element()
        } else {
            v_flex()
                .gap_1()
                .children(offers.into_iter().enumerate().map(|(i, p)| {
                    let name = product_name(&p);
                    let offer = jstr(&p, "/offerId")
                        .pipe_or(jstr(&p, "/npcId"))
                        .pipe_or(ji64(&p, "/npcId").to_string());
                    card(cx)
                        .id(SharedString::from(format!("mys-{i}")))
                        .child(
                            h_flex()
                                .items_center()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(name),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("报价 {offer}")),
                                        ),
                                )
                                .child({
                                    let state = state.clone();
                                    let offer = offer.clone();
                                    Button::new(SharedString::from(format!("mbuy-{i}")))
                                        .small()
                                        .primary()
                                        .label("购买")
                                        .on_click(move |_, _, cx| {
                                            let offer = offer.clone();
                                            bridge::run_async(
                                                &state,
                                                cx,
                                                move |app, id| async move {
                                                    qq_farm_app::commerce::purchase_mystery(
                                                        &app, &id, &offer,
                                                    )
                                                    .await
                                                    .map_err(|e| e.to_string())
                                                },
                                                |s, _, cx| {
                                                    s.last_message = Some("已购买神秘商品".into());
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

fn extract_products(root: &Value) -> Vec<Value> {
    for key in ["products", "items", "goods", "offers", "list"] {
        if let Some(arr) = root.get(key).and_then(|v| v.as_array()) {
            return arr.clone();
        }
    }
    root.as_array().cloned().unwrap_or_default()
}

fn product_name(p: &Value) -> String {
    for path in ["/name", "/goodsName", "/title", "/itemName"] {
        let s = jstr(p, path);
        if !s.is_empty() {
            return s;
        }
    }
    format!("商品#{}", ji64(p, "/goodsId").max(ji64(p, "/id")))
}

trait PipeOr {
    fn pipe_or(self, other: String) -> String;
}
impl PipeOr for String {
    fn pipe_or(self, other: String) -> String {
        if self.is_empty() || self == "0" {
            other
        } else {
            self
        }
    }
}
