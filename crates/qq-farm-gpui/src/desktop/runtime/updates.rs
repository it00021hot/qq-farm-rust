use super::*;

fn update_checking_message(language: &str) -> String {
    crate::i18n::text(language, "正在检查发布…", "Checking for updates…").into()
}

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

impl FarmDesktop {
    pub(in crate::desktop) fn check_update(&mut self, cx: &mut Context<Self>) {
        let runtime = Arc::clone(&self.tokio_runtime);
        self.update_message = Some(update_checking_message(&self.preferences.language));
        self.release_url = None;
        cx.notify();
        if self.test_mode {
            // Tests exercise the in-flight state without hitting the release API.
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = runtime.spawn(crate::update::check_latest()).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(release))) => {
                        let version = release.name.as_deref().unwrap_or(&release.tag_name);
                        this.update_message =
                            Some(update_available_message(&this.preferences.language, version));
                        this.release_url = Some(release.html_url);
                    }
                    Ok(Ok(None)) => {
                        this.update_message = Some(no_update_message(&this.preferences.language));
                        this.release_url = None;
                    }
                    Ok(Err(error)) => {
                        this.update_message =
                            Some(error.localized_message(&this.preferences.language));
                        this.release_url = None;
                    }
                    Err(error) => {
                        this.update_message = Some(crate::i18n::error_with_detail(
                            &this.preferences.language,
                            "版本检查任务失败",
                            "Update check failed",
                            error,
                        ));
                        this.release_url = None;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::desktop) fn open_release_page(&mut self) {
        let Some(url) = self.release_url.as_deref() else { return };
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
}

#[cfg(test)]
mod tests {
    use super::{no_update_message, update_available_message, update_checking_message};

    #[test]
    fn update_status_messages_follow_the_selected_language() {
        assert_eq!(update_checking_message("zh-CN"), "正在检查发布…");
        assert_eq!(update_checking_message("en-US"), "Checking for updates…");
        assert_eq!(no_update_message("zh-CN"), "尚无适用于本机的新版本安装包");
        assert_eq!(no_update_message("en-US"), "No new release is available for this device");
        assert_eq!(update_available_message("zh-CN", "v1.2.3"), "发现新版本 v1.2.3");
        assert_eq!(update_available_message("en-US", "v1.2.3"), "Version v1.2.3 is available");
    }
}
