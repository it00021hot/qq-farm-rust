//! 主壳：侧栏 + 内容区。

mod sidebar;

use gpui::*;
use gpui_component::alert::Alert;
use gpui_component::ThemeMode;

use crate::app_state::{AppState, NavPage};
use crate::ui::*;
use crate::views;

pub struct ShellView {
    state: Entity<AppState>,
}

impl ShellView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        state.update(cx, |s, cx| s.refresh_async(cx));
        let _ = window;
        Self { state }
    }
}

impl Render for ShellView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let page = state.page;
        let err = state.last_error.clone();
        let msg = state.last_message.clone();
        let toast_kind = state.toast_kind;
        let account_id = state.account_id.clone();
        let dark = state.dark_theme;

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .id("toolbar")
                    .h(px(52.))
                    .px_5()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().title_bar)
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size_8()
                                            .rounded_lg()
                                            .bg(cx.theme().primary)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                Icon::new(IconName::LayoutDashboard)
                                                    .text_color(cx.theme().primary_foreground)
                                                    .into_any_element(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BOLD)
                                            .child("QQ Farm"),
                                    ),
                            )
                            .child(Tag::secondary().small().rounded_full().child(page.label())),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("refresh")
                                    .ghost()
                                    .icon(IconName::Loader)
                                    .small()
                                    .tooltip("刷新当前页数据")
                                    .on_click({
                                        let state = self.state.clone();
                                        move |_, _, cx| {
                                            state.update(cx, |s, cx| {
                                                s.refresh_sync();
                                                s.refresh_async(cx);
                                                s.flash_success("已刷新", cx);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("theme")
                                    .ghost()
                                    .small()
                                    .icon(if dark {
                                        IconName::Sun
                                    } else {
                                        IconName::Moon
                                    })
                                    .tooltip(if dark {
                                        "切换浅色"
                                    } else {
                                        "切换深色"
                                    })
                                    .on_click({
                                        let state = self.state.clone();
                                        move |_, window, cx| {
                                            state.update(cx, |s, cx| {
                                                s.dark_theme = !s.dark_theme;
                                                let mode = if s.dark_theme {
                                                    ThemeMode::Dark
                                                } else {
                                                    ThemeMode::Light
                                                };
                                                gpui_component::Theme::change(
                                                    mode,
                                                    Some(window),
                                                    cx,
                                                );
                                                qq_farm_app::settings::set_theme(if s.dark_theme {
                                                    "dark"
                                                } else {
                                                    "light"
                                                });
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .id("body")
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .child(sidebar::render_sidebar(&self.state, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .bg(cx.theme().secondary.opacity(0.55))
                            .p_5()
                            .gap_3()
                            .child(toast_banner(
                                &self.state,
                                err,
                                msg,
                                toast_kind,
                                account_id.is_empty(),
                                cx,
                            ))
                            .child(
                                div()
                                    .id("content")
                                    .flex_1()
                                    .overflow_y_scroll()
                                    .child(match page {
                                        NavPage::Dashboard => views::dashboard::render(
                                            &self.state, cx,
                                        )
                                        .into_any_element(),
                                        NavPage::Personal => {
                                            views::personal::render(&self.state, cx).into_any_element()
                                        }
                                        NavPage::Activity => {
                                            views::activity::render(&self.state, cx).into_any_element()
                                        }
                                        NavPage::GameMall => views::commerce::render_mall(
                                            &self.state, cx,
                                        )
                                        .into_any_element(),
                                        NavPage::MysteryShop => views::commerce::render_mystery(
                                            &self.state, cx,
                                        )
                                        .into_any_element(),
                                        NavPage::Friends => {
                                            views::friends::render(&self.state, cx).into_any_element()
                                        }
                                        NavPage::Analytics => views::analytics::render(
                                            &self.state, cx,
                                        )
                                        .into_any_element(),
                                        NavPage::Settings => {
                                            views::settings::render(&self.state, cx).into_any_element()
                                        }
                                        NavPage::Config => {
                                            views::config::render(&self.state, cx).into_any_element()
                                        }
                                        NavPage::Admin => {
                                            views::admin::render(&self.state, cx).into_any_element()
                                        }
                                    }),
                            ),
                    ),
            )
    }
}

fn toast_banner(
    state: &Entity<AppState>,
    err: Option<String>,
    msg: Option<String>,
    kind: u8,
    no_account: bool,
    cx: &App,
) -> AnyElement {
    let _ = cx;
    if let Some(e) = err {
        let state = state.clone();
        return Alert::error("toast-err", e)
            .banner()
            .on_close(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.clear_toast();
                    cx.notify();
                });
            })
            .into_any_element();
    }
    if let Some(m) = msg {
        let state = state.clone();
        let alert = if kind == 1 {
            Alert::warning("toast-warn", m).banner()
        } else {
            Alert::success("toast-ok", m).banner()
        };
        return alert
            .on_close(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.clear_toast();
                    cx.notify();
                });
            })
            .into_any_element();
    }
    if no_account {
        return Alert::info(
            "toast-empty",
            "还没有可用账号。左侧点「+ 添加」，用微信扫码登录。",
        )
        .banner()
        .into_any_element();
    }
    div().into_any_element()
}
