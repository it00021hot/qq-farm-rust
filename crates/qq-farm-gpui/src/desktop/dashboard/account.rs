use super::super::*;
use super::format::{
    account_avatar_url, account_display_name, int_at, level_forecast_lines, string_at,
};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::StatefulInteractiveElement;

impl FarmDesktop {
    pub(super) fn render_account_card(&self, cx: &mut Context<Self>) -> Div {
        let status = self.page_raw.get("status").unwrap_or(&serde_json::Value::Null);
        let language = self.preferences.language.as_str();
        let level = int_at(status, &["level"]);
        let nickname = string_at(status, &["nick", "status.name"]);
        let account_name = self
            .selected_account
            .as_ref()
            .and_then(|id| self.accounts.records.iter().find(|(account_id, _, _)| account_id == id))
            .map(|(_, name, _)| name.as_str())
            .unwrap_or("");
        let saved_avatar = self
            .selected_account
            .as_ref()
            .and_then(|id| self.accounts.avatars.get(id))
            .map(String::as_str);
        let display_name = account_display_name(&nickname, account_name, language);
        let progress_current = int_at(status, &["levelProgress.current"]);
        let progress_needed = int_at(status, &["levelProgress.needed"]);
        let progress = if progress_needed > 0 {
            (progress_current as f32 / progress_needed as f32).clamp(0., 1.)
        } else {
            0.
        };
        let uptime = int_at(status, &["uptime"]).max(0);
        let session_exp = int_at(status, &["sessionExpGained"]);
        // 会话效率分母优先用 sessionElapsedSeconds（bot 62e1880：登录起算）
        let session_elapsed = int_at(status, &["sessionElapsedSeconds"]);
        let rate_base = if session_elapsed > 0 { session_elapsed } else { uptime };
        let exp_rate = if rate_base > 0 {
            ((session_exp.max(0) as f64 * 3600. / rate_base as f64).floor() as i64).max(0)
        } else {
            0
        };
        // 等级升级时间预测（bot 62e1880：服务端按当前作物排期推算，替代本地历史估算）
        let forecast_lines = level_forecast_lines(status.get("levelForecast"), language);
        let avatar = account_avatar_url(&string_at(status, &["avatar"]), saved_avatar);
        let avatar_initial = display_name.chars().next().unwrap_or('?');
        let avatar_background = cx.theme().background;
        let avatar_fallback = move || {
            div()
                .id("dashboard-account-avatar")
                .aria_label(format!("Avatar {avatar_initial}"))
                .size(px(40.))
                .rounded_full()
                .bg(avatar_background)
                .flex()
                .items_center()
                .justify_center()
                .font_semibold()
                .child(avatar_initial.to_string())
                .test_support()
                .into_any_element()
        };
        let avatar_element = match avatar.as_deref() {
            Some(url) => img(url.to_string())
                .size(px(40.))
                .rounded_full()
                .object_fit(ObjectFit::Cover)
                .with_fallback(avatar_fallback)
                .into_any_element(),
            None => avatar_fallback(),
        };
        let level_card = div()
            .v_flex()
            .flex_1()
            .min_w(px(230.))
            .gap_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .p_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::i18n::text(language, "账号", "Account")),
                    )
                    .child(
                        div()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().primary.opacity(0.12))
                            .px_2()
                            .py_1()
                            .text_xs()
                            .text_color(cx.theme().primary)
                            .child(format!("Lv.{level}")),
                    ),
            )
            .child(
                h_flex().items_center().gap_3().child(avatar_element).child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_medium()
                        .id("dashboard-account-name")
                        .child(display_name.clone())
                        .tooltip(move |window, cx| {
                            Tooltip::new(display_name.clone()).build(window, cx)
                        }),
                ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("EXP")
                    .child(format!(
                        "{progress_current} / {}",
                        if progress_needed > 0 { progress_needed.to_string() } else { "?".into() }
                    )),
            )
            .child(
                div().h(px(5.)).w_full().rounded_full().bg(cx.theme().background).child(
                    div().h_full().w(relative(progress)).rounded_full().bg(cx.theme().primary),
                ),
            )
            .child(
                h_flex().justify_between().text_xs().text_color(cx.theme().muted_foreground).child(
                    if language == "en-US" {
                        format!("Session rate: {exp_rate}/h")
                    } else {
                        format!("会话效率：{exp_rate}/时")
                    },
                ),
            )
            .children(
                forecast_lines.into_iter().map(|line| {
                    div().text_xs().text_color(cx.theme().muted_foreground).child(line)
                }),
            )
            .when(session_exp != 0, |card| {
                card.child(div().text_xs().text_color(cx.theme().green).child(
                    if language == "en-US" {
                        format!("EXP gained today: {session_exp:+}")
                    } else {
                        format!("今日已获得经验：{session_exp:+}")
                    },
                ))
            });

        level_card
    }
}
