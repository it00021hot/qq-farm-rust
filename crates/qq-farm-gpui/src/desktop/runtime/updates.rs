use super::*;
use gpui_kit::component::WindowExt;

fn update_available_message(language: &str, version: &str) -> String {
    if language == "en-US" {
        format!("Version {version} is available")
    } else {
        format!("发现新版本 {version}")
    }
}

fn no_update_message(language: &str) -> String {
    crate::i18n::text(
        language,
        "尚无适用于本机的新版本安装包",
        "No new release is available for this device",
    )
    .into()
}

fn open_release_url(url: &str) {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    if let Err(error) = command.arg(url).spawn() {
        tracing::warn!(%error, "could not open the GPUI release page");
    }
}

impl FarmDesktop {
    /// 检查更新：结果只走全局 toast；有可用版本时弹窗提供发布页入口。
    pub(in crate::desktop) fn check_update(&mut self, cx: &mut Context<Self>) {
        if self.update_checking {
            return;
        }
        self.update_checking = true;
        cx.notify();
        if self.test_mode {
            // Tests exercise the in-flight state without hitting the release API.
            return;
        }
        let runtime = Arc::clone(&self.tokio_runtime);
        cx.spawn(async move |this, cx| {
            let result = runtime.spawn(crate::update::check_latest()).await;
            let _ = this.update(cx, |this, cx| {
                this.update_checking = false;
                match result {
                    Ok(Ok(Some(release))) => {
                        let version =
                            release.name.clone().unwrap_or_else(|| release.tag_name.clone());
                        this.show_update_dialog(&version, &release.html_url, cx);
                    }
                    Ok(Ok(None)) => {
                        this.show_toast(no_update_message(&this.preferences.language), true, cx);
                    }
                    Ok(Err(error)) => {
                        this.show_toast(
                            error.localized_message(&this.preferences.language),
                            false,
                            cx,
                        );
                    }
                    Err(error) => {
                        this.show_toast(
                            crate::i18n::error_with_detail(
                                &this.preferences.language,
                                "版本检查任务失败",
                                "Update check failed",
                                error,
                            ),
                            false,
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn show_update_dialog(
        &mut self,
        version: &str,
        html_url: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = self.window_handle else { return };
        let language = self.preferences.language.clone();
        let message = update_available_message(&language, version);
        let url = html_url.to_string();
        let _ = handle.update(cx, move |_this, window, cx| {
            let language = language.clone();
            let message = message.clone();
            let url = url.clone();
            window.open_dialog(cx, move |dialog, _, _| {
                let language = language.clone();
                let message = message.clone();
                let url = url.clone();
                dialog
                    .title(crate::i18n::text(&language, "发现新版本", "Update available"))
                    .w(px(420.))
                    .child(div().text_sm().child(message))
                    .footer(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("update-later")
                                    .label(crate::i18n::text(&language, "稍后", "Later"))
                                    .ghost()
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child(
                                Button::new("update-open-release")
                                    .label(crate::i18n::text(
                                        &language,
                                        "打开发布页下载",
                                        "Download from release page",
                                    ))
                                    .primary()
                                    .on_click(move |_, window, cx| {
                                        open_release_url(&url);
                                        window.close_dialog(cx);
                                    }),
                            ),
                    )
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{no_update_message, update_available_message};

    #[test]
    fn update_status_messages_follow_the_selected_language() {
        assert_eq!(no_update_message("zh-CN"), "尚无适用于本机的新版本安装包");
        assert_eq!(no_update_message("en-US"), "No new release is available for this device");
        assert_eq!(update_available_message("zh-CN", "v1.2.3"), "发现新版本 v1.2.3");
        assert_eq!(update_available_message("en-US", "v1.2.3"), "Version v1.2.3 is available");
    }
}
