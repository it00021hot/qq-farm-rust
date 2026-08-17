//! 侧栏导航 + 账号切换。

use gpui::*;

use crate::app_state::{AppState, NavPage};
use crate::ui::*;

pub fn render_sidebar(state: &Entity<AppState>, cx: &mut Context<impl Render>) -> impl IntoElement {
    let current = state.read(cx).page;
    let accounts = state.read(cx).account_labels();
    let selected = state.read(cx).account_id.clone();

    v_flex()
        .id("sidebar")
        .w(px(256.))
        .h_full()
        .border_r_1()
        .border_color(cx.theme().sidebar_border)
        .bg(cx.theme().sidebar)
        .p_3()
        .gap_4()
        .child(
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .px_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child("账号"),
                        )
                        .child({
                            let state = state.clone();
                            Button::new("add-account-side")
                                .xsmall()
                                .ghost()
                                .icon(IconName::Plus)
                                .label("添加")
                                .on_click(move |_, window, cx| {
                                    state.update(cx, |s, cx| {
                                        s.settings_tab = 0;
                                        s.set_page(NavPage::Settings, cx);
                                        s.open_add_account(None, window, cx);
                                    });
                                })
                        }),
                )
                .child(if accounts.is_empty() {
                    v_flex()
                        .gap_2()
                        .p_3()
                        .rounded_xl()
                        .border_1()
                        .border_color(cx.theme().sidebar_border)
                        .bg(cx.theme().sidebar_accent)
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("还没有账号"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("点「添加」用微信扫码登录"),
                        )
                        .into_any_element()
                } else {
                    v_flex()
                        .gap_1()
                        .children(accounts.into_iter().map(|(id, name, running)| {
                            let is_selected = selected == id;
                            let state = state.clone();
                            let id_click = id.clone();
                            let avatar_name = name.clone();
                            div()
                                .id(SharedString::from(format!("acc-card-{id}")))
                                .px_2()
                                .py_2()
                                .rounded_xl()
                                .cursor_pointer()
                                .border_1()
                                .border_color(if is_selected {
                                    cx.theme().primary.opacity(0.35)
                                } else {
                                    gpui::transparent_black()
                                })
                                .bg(if is_selected {
                                    cx.theme().sidebar_accent
                                } else {
                                    gpui::transparent_black()
                                })
                                .hover(|s| s.bg(cx.theme().sidebar_accent))
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |s, cx| s.set_account(id_click.clone(), cx));
                                })
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Avatar::new().name(avatar_name).xsmall())
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .overflow_hidden()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(name),
                                                )
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_1()
                                                        .child(
                                                            div()
                                                                .size_2()
                                                                .rounded_full()
                                                                .bg(if running {
                                                                    cx.theme().success
                                                                } else {
                                                                    cx.theme().muted_foreground
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                )
                                                                .child(if running {
                                                                    "运行中"
                                                                } else {
                                                                    "已停止"
                                                                }),
                                                        ),
                                                ),
                                        ),
                                )
                        }))
                        .into_any_element()
                }),
        )
        .child(div().h(px(1.)).bg(cx.theme().sidebar_border).mx_1())
        .child(
            v_flex()
                .gap_1()
                .flex_1()
                .child(
                    div()
                        .px_2()
                        .pb_1()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("导航"),
                )
                .children(NavPage::ALL.iter().map(|(page, label)| {
                    let page = *page;
                    let is_selected = current == page;
                    let state = state.clone();
                    let icon = nav_icon(page);
                    h_flex()
                        .id(SharedString::from(format!("nav-{label}")))
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded_xl()
                        .cursor_pointer()
                        .bg(if is_selected {
                            cx.theme().sidebar_accent
                        } else {
                            gpui::transparent_black()
                        })
                        .hover(|s| s.bg(cx.theme().sidebar_accent))
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| s.set_page(page, cx));
                        })
                        .when(is_selected, |el| {
                            el.border_l_2()
                                .border_color(cx.theme().primary)
                                .pl(px(10.))
                        })
                        .child(
                            Icon::new(icon)
                                .text_color(if is_selected {
                                    cx.theme().primary
                                } else {
                                    cx.theme().muted_foreground
                                })
                                .into_any_element(),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(if is_selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(*label),
                        )
                })),
        )
}

fn nav_icon(page: NavPage) -> IconName {
    match page {
        NavPage::Dashboard => IconName::LayoutDashboard,
        NavPage::Personal => IconName::User,
        NavPage::Activity => IconName::Star,
        NavPage::GameMall => IconName::Inbox,
        NavPage::MysteryShop => IconName::Palette,
        NavPage::Friends => IconName::Heart,
        NavPage::Analytics => IconName::ChartPie,
        NavPage::Settings => IconName::Settings,
        NavPage::Config => IconName::Settings2,
        NavPage::Admin => IconName::SquareTerminal,
    }
}
