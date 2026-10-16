//! 全局操作反馈通知（gpui-kit Notification toast，右上角、5s 自动消失）。
//!
//! 页面内嵌通知保留（可查历史）；关键操作的即时反馈走 toast，
//! 对齐 Tauri 版的 message 式全局提示。

use super::*;

use gpui_kit::component::notification::{Notification, NotificationType};
use gpui_kit::component::WindowExt;

impl FarmDesktop {
    /// 弹出操作结果 toast（success=false 时为错误样式）。
    pub(crate) fn show_toast(
        &self,
        message: impl Into<SharedString>,
        success: bool,
        cx: &mut Context<Self>,
    ) {
        let message = message.into();
        if let Some(handle) = self.window_handle.as_ref() {
            let _ = handle.update(cx, |_, window, cx| {
                window.push_notification(
                    Notification::new().message(message).with_type(if success {
                        NotificationType::Success
                    } else {
                        NotificationType::Error
                    }),
                    cx,
                );
            });
        }
    }
}
