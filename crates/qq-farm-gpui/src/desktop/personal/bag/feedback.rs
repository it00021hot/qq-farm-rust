use std::time::Duration;

use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn show_personal_bag_notice(
        &mut self,
        message: impl Into<String>,
        success: bool,
        cx: &mut Context<Self>,
    ) {
        self.farm.bag.notice_generation = self.farm.bag.notice_generation.wrapping_add(1);
        let generation = self.farm.bag.notice_generation;
        self.farm.bag.notice = Some((message.into(), success));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.bag.notice_generation == generation {
                    this.farm.bag.notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

pub(super) fn operation_summary(
    result: &serde_json::Value,
    language: &str,
    chinese_fallback: &str,
    english_fallback: &str,
) -> String {
    result
        .get("summary")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .unwrap_or(crate::i18n::text(language, chinese_fallback, english_fallback))
        .to_string()
}
