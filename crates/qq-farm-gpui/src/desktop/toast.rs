//! 全局操作反馈 toast：右上角浮层，默认 2.5s 自动消失，悬停时暂停计时。

use std::time::Duration;

use super::*;

/// 未悬停时的展示时长（用户反馈 5s 过长）。
const TOAST_DURATION_MS: u64 = 2_500;
/// 剩余时间按固定步长递减；悬停期间不递减。
const TOAST_TICK_MS: u64 = 100;

impl FarmDesktop {
    /// 弹出操作结果 toast（success=false 时为错误样式）。
    pub(crate) fn show_toast(
        &mut self,
        message: impl Into<String>,
        success: bool,
        cx: &mut Context<Self>,
    ) {
        self.toast_generation = self.toast_generation.wrapping_add(1);
        let generation = self.toast_generation;
        self.toast = Some(ToastState {
            message: message.into(),
            success,
            remaining_ms: TOAST_DURATION_MS,
            hovered: false,
        });
        cx.notify();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(TOAST_TICK_MS)).await;
            let active = this.update(cx, |this, cx| {
                if generation != this.toast_generation {
                    return false;
                }
                let Some(toast) = this.toast.as_mut() else { return false };
                if toast.hovered {
                    return true;
                }
                if toast.remaining_ms <= TOAST_TICK_MS {
                    this.toast = None;
                    cx.notify();
                    return false;
                }
                toast.remaining_ms -= TOAST_TICK_MS;
                true
            });
            if !matches!(active, Ok(true)) {
                break;
            }
        })
        .detach();
    }

    pub(crate) fn render_toast(&self, cx: &mut Context<Self>) -> Div {
        div().when_some(self.toast.as_ref(), |layer, toast| {
            layer.child(
                div()
                    .id("app-toast")
                    .test_support()
                    .absolute()
                    .top(px(16.))
                    .right(px(16.))
                    .max_w(px(360.))
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().group_box)
                    .px_4()
                    .py_3()
                    .text_sm()
                    .text_color(if toast.success { cx.theme().success } else { cx.theme().danger })
                    .child(SharedString::from(toast.message.clone()))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if let Some(toast) = this.toast.as_mut() {
                            toast.hovered = *hovered;
                            cx.notify();
                        }
                    })),
            )
        })
    }
}
