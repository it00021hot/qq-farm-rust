//! 节令日历页（bot 0533264 SolarCalendar.vue 的 GPUI 移植）。

use std::time::Duration;

use gpui_kit::component::WindowExt;

use super::super::*;

fn unix_time_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

fn js_i64(value: Option<&serde_json::Value>) -> i64 {
    match value {
        Some(serde_json::Value::Number(number)) => number.as_i64().unwrap_or_default(),
        Some(serde_json::Value::String(text)) => text.trim().parse().unwrap_or_default(),
        _ => 0,
    }
}

fn solar_events(raw: &serde_json::Value) -> Vec<serde_json::Value> {
    raw.get("events").and_then(serde_json::Value::as_array).cloned().unwrap_or_default()
}

fn solar_seasons(raw: &serde_json::Value) -> Vec<serde_json::Value> {
    raw.get("seasons").and_then(serde_json::Value::as_array).cloned().unwrap_or_default()
}

fn solar_event_by_id(raw: &serde_json::Value, id: i64) -> Option<&serde_json::Value> {
    raw.get("events")
        .and_then(serde_json::Value::as_array)
        .and_then(|events| events.iter().find(|event| js_i64(event.get("id")) == id))
}

/// 下一个自动刷新边界：事件开始 / 结束 + 1 / 答题重置（次日 0 点，北京时间）。
fn solar_next_boundary(raw: &serde_json::Value) -> i64 {
    let snapshot_time = js_i64(raw.get("serverTime"));
    if snapshot_time <= 0 {
        return 0;
    }
    let mut times: Vec<i64> = Vec::new();
    let mut has_wrong_today = false;
    for event in solar_events(raw) {
        times.push(js_i64(event.get("startTime")));
        times.push(js_i64(event.get("endTime")) + 1);
        if event.get("quizWrongToday").and_then(serde_json::Value::as_bool) == Some(true) {
            has_wrong_today = true;
        }
    }
    if has_wrong_today {
        times.push((((snapshot_time + 28_800) / 86_400) + 1) * 86_400 - 28_800);
    }
    times.into_iter().filter(|time| *time > snapshot_time).min().unwrap_or(0)
}

/// 撕页日历：首行是农历日期，其余是节令诗句。
fn solar_lunar_of(event: &serde_json::Value) -> String {
    let description = event.get("description").and_then(serde_json::Value::as_str).unwrap_or("");
    let first = description.split('\n').next().unwrap_or("");
    if first.starts_with("农历") {
        first.to_string()
    } else {
        String::new()
    }
}

fn solar_poem_of(event: &serde_json::Value) -> String {
    let description = event.get("description").and_then(serde_json::Value::as_str).unwrap_or("");
    let lines: Vec<&str> = description.split('\n').filter(|line| !line.is_empty()).collect();
    let lines =
        if !solar_lunar_of(event).is_empty() { &lines[1.min(lines.len())..] } else { &lines[..] };
    lines.join("\n")
}

/// "A.敬老节" 拆成字母标和选项文字（对齐 bot optionParts）。
fn solar_option_parts(option: &str, index: usize) -> (String, String) {
    let mut chars = option.chars();
    if let (Some(letter), Some(separator)) = (chars.next(), chars.next()) {
        if letter.is_ascii_uppercase() && matches!(separator, '.' | '．' | '、') {
            return (letter.to_string(), chars.as_str().trim().to_string());
        }
    }
    (String::from_utf8_lossy(&[b'A' + index as u8]).to_string(), option.to_string())
}

fn solar_distance_text(seconds: i64, language: &str) -> String {
    if seconds >= 86_400 {
        let days = (seconds + 86_399) / 86_400;
        if language == "en-US" {
            format!("{days}d")
        } else {
            format!("{days}天")
        }
    } else if seconds >= 3_600 {
        let hours = (seconds + 3_599) / 3_600;
        if language == "en-US" {
            format!("{hours}h")
        } else {
            format!("{hours}小时")
        }
    } else {
        let minutes = (seconds / 60).max(1);
        if language == "en-US" {
            format!("{minutes}m")
        } else {
            format!("{minutes}分钟")
        }
    }
}

/// 阶段（对齐 bot phase）：done / expired / upcoming / open。
fn solar_phase(event: &serde_json::Value, now: i64) -> &'static str {
    let direct_claimed =
        event.get("directClaimed").and_then(serde_json::Value::as_bool) == Some(true);
    let quiz_claimed = event.get("quizClaimed").and_then(serde_json::Value::as_bool) == Some(true);
    let has_question = event
        .get("question")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|question| !question.is_empty());
    if direct_claimed && (!has_question || quiz_claimed) {
        return "done";
    }
    let status = js_i64(event.get("status"));
    if now > js_i64(event.get("endTime")) || status == 3 {
        return "expired";
    }
    if now < js_i64(event.get("startTime")) {
        return "upcoming";
    }
    "open"
}

fn solar_phase_text(event: &serde_json::Value, now: i64, language: &str) -> String {
    match solar_phase(event, now) {
        "done" => crate::i18n::text(language, "已完成", "Completed").to_string(),
        "expired" => crate::i18n::text(language, "已结束", "Ended").to_string(),
        "upcoming" => {
            let distance = solar_distance_text(js_i64(event.get("startTime")) - now, language);
            crate::i18n::text(
                language,
                &format!("{distance}后开始"),
                &format!("Starts in {distance}"),
            )
            .to_string()
        }
        _ => {
            let distance = solar_distance_text(js_i64(event.get("endTime")) - now, language);
            crate::i18n::text(
                language,
                &format!("进行中 · 剩 {distance}"),
                &format!("Open · {distance} left"),
            )
            .to_string()
        }
    }
}

/// 领取 / 答题按钮文案（对齐 bot stateText）。
fn solar_state_text(event: &serde_json::Value, now: i64, quiz: bool, language: &str) -> String {
    let claimed = if quiz {
        event.get("quizClaimed").and_then(serde_json::Value::as_bool) == Some(true)
    } else {
        event.get("directClaimed").and_then(serde_json::Value::as_bool) == Some(true)
    };
    if claimed {
        return crate::i18n::text(language, "已领取", "Claimed").to_string();
    }
    if now > js_i64(event.get("endTime")) || js_i64(event.get("status")) == 3 {
        return crate::i18n::text(language, "已过期", "Expired").to_string();
    }
    let seconds = js_i64(event.get("startTime")) - now;
    if seconds > 0 {
        let distance = solar_distance_text(seconds, language);
        return if quiz {
            crate::i18n::text(
                language,
                &format!("{distance}后开启"),
                &format!("Opens in {distance}"),
            )
            .to_string()
        } else {
            crate::i18n::text(
                language,
                &format!("{distance}后可领取"),
                &format!("Claimable in {distance}"),
            )
            .to_string()
        };
    }
    if quiz {
        if event.get("quizWrongToday").and_then(serde_json::Value::as_bool) == Some(true) {
            return crate::i18n::text(language, "明日再来", "Try again tomorrow").to_string();
        }
        return crate::i18n::text(language, "答题", "Answer").to_string();
    }
    crate::i18n::text(language, "领取", "Claim").to_string()
}

fn solar_reward_text(value: &serde_json::Value, action: Option<&str>, language: &str) -> String {
    let rewards = value
        .get("rewards")
        .and_then(serde_json::Value::as_array)
        .map(|rewards| {
            rewards
                .iter()
                .map(|reward| {
                    let name = reward.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
                    let count = js_i64(reward.get("count"));
                    format!("{name} ×{count}")
                })
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    if !rewards.is_empty() {
        return rewards;
    }
    if action == Some("answer")
        && value.get("correct").and_then(serde_json::Value::as_bool) == Some(false)
    {
        return crate::i18n::text(language, "明日再来", "Try again tomorrow").to_string();
    }
    crate::i18n::text(language, "领取成功", "Claimed").to_string()
}

impl FarmDesktop {
    /// 进入页面时启动 1s 时钟（本地推演 serverTime，跨边界自动刷新）。
    pub(in crate::desktop) fn start_solar_calendar_clock(&mut self, cx: &mut Context<Self>) {
        if self.test_mode || self.page != Page::SolarCalendar {
            return;
        }
        self.solar_calendar.tick_generation = self.solar_calendar.tick_generation.wrapping_add(1);
        let generation = self.solar_calendar.tick_generation;
        let now = unix_time_seconds();
        self.solar_calendar.base_server_time = js_i64(self.page_raw.get("serverTime"));
        self.solar_calendar.base_at_sec = now;
        self.solar_calendar.now_sec = self.solar_calendar.base_server_time;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let now = unix_time_seconds();
            let active = this.update(cx, |this, cx| {
                if generation != this.solar_calendar.tick_generation
                    || this.page != Page::SolarCalendar
                {
                    return false;
                }
                let elapsed = now.saturating_sub(this.solar_calendar.base_at_sec);
                this.solar_calendar.now_sec =
                    this.solar_calendar.base_server_time.saturating_add(elapsed);
                let boundary = solar_next_boundary(&this.page_raw);
                if boundary > 0
                    && this.solar_calendar.now_sec >= boundary
                    && this.solar_calendar.refreshed_boundary != boundary
                    && !this.solar_calendar.pending
                {
                    this.solar_calendar.refreshed_boundary = boundary;
                    this.refresh(cx);
                }
                cx.notify();
                true
            });
            if !matches!(active, Ok(true)) {
                break;
            }
        })
        .detach();
    }

    pub(in crate::desktop) fn invalidate_solar_calendar(&mut self) {
        self.solar_calendar.tick_generation = self.solar_calendar.tick_generation.wrapping_add(1);
        self.solar_calendar.action_generation =
            self.solar_calendar.action_generation.wrapping_add(1);
        self.solar_calendar.pending = false;
        self.solar_calendar.question_id = 0;
        self.solar_calendar.quiz_answer = None;
        self.solar_calendar.error = None;
    }

    /// 载入 / 操作后用回包日历重建时钟基点并归一赛季选中（对齐 bot watch 逻辑）。
    pub(in crate::desktop) fn apply_solar_calendar(&mut self, calendar: serde_json::Value) {
        let now = unix_time_seconds();
        self.page_raw = calendar;
        self.solar_calendar.error = None;
        self.solar_calendar.base_server_time = js_i64(self.page_raw.get("serverTime"));
        self.solar_calendar.base_at_sec = now;
        self.solar_calendar.now_sec = self.solar_calendar.base_server_time;
        self.reconcile_solar_calendar_selection();
    }

    fn reconcile_solar_calendar_selection(&mut self) {
        let seasons = solar_seasons(&self.page_raw);
        if seasons.iter().any(|season| js_i64(season.get("id")) == self.solar_calendar.season_id) {
            return;
        }
        let now = self.solar_calendar.now_sec;
        let target = solar_events(&self.page_raw)
            .into_iter()
            .find(|event| {
                event.get("directClaimed").and_then(serde_json::Value::as_bool) != Some(true)
                    && js_i64(event.get("endTime")) >= now
            })
            .map(|event| js_i64(event.get("id")))
            .unwrap_or_else(|| js_i64(self.page_raw.get("defaultEventId")));
        self.solar_calendar.season_id = seasons
            .iter()
            .find(|season| {
                season
                    .get("eventIds")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|ids| ids.iter().any(|id| js_i64(Some(id)) == target))
            })
            .map(|season| js_i64(season.get("id")))
            .or_else(|| seasons.last().map(|season| js_i64(season.get("id"))))
            .unwrap_or(0);
    }

    /// 触发一次日历请求（`action=None` 拉快照）。
    pub(in crate::desktop) fn solar_calendar_request(
        &mut self,
        action: Option<&'static str>,
        input: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if self.solar_calendar.pending {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else {
            self.error = Some(
                crate::i18n::text(
                    &self.preferences.language,
                    "请先添加账号",
                    "Add an account first",
                )
                .into(),
            );
            cx.notify();
            return;
        };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let account_id_guard = account_id.clone();
        self.solar_calendar.action_generation =
            self.solar_calendar.action_generation.wrapping_add(1);
        let generation = self.solar_calendar.action_generation;
        self.solar_calendar.pending = true;
        self.solar_calendar.error = None;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    match action {
                        Some(action) => {
                            qq_farm_app::solar_calendar::operate(&app, &account_id, action, &input)
                                .await
                        }
                        None => qq_farm_app::solar_calendar::snapshot(&app, &account_id).await,
                    }
                    .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.solar_calendar.action_generation != generation {
                    return;
                }
                this.solar_calendar.pending = false;
                if this.selected_account.as_deref() != Some(account_id_guard.as_str())
                    || this.page != Page::SolarCalendar
                {
                    cx.notify();
                    return;
                }
                let language = this.preferences.language.clone();
                match result {
                    Ok(Ok(value)) => {
                        if action.is_some() {
                            this.solar_calendar.question_id = 0;
                            this.solar_calendar.quiz_answer = None;
                            let text = solar_reward_text(&value, action, &language);
                            let refresh_required =
                                value.get("refreshRequired").and_then(serde_json::Value::as_bool)
                                    == Some(true);
                            this.show_toast(text, !refresh_required, cx);
                        }
                        let calendar =
                            value.get("calendar").cloned().unwrap_or(serde_json::Value::Null);
                        if calendar.is_null() {
                            if action.is_some() {
                                this.refresh(cx);
                            } else {
                                this.solar_calendar.error = Some(
                                    crate::i18n::text(
                                        &language,
                                        "暂未获取到节令日历",
                                        "No solar calendar data",
                                    )
                                    .into(),
                                );
                            }
                        } else {
                            this.apply_solar_calendar(calendar);
                        }
                    }
                    Ok(Err(error)) => {
                        if action.is_some() {
                            this.show_toast(error, false, cx);
                        } else {
                            this.solar_calendar.error = Some(error);
                        }
                    }
                    Err(error) => {
                        let message = crate::i18n::background_task_error(&language, error);
                        if action.is_some() {
                            this.show_toast(message, false, cx);
                        } else {
                            this.solar_calendar.error = Some(message);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 打开节令问答弹窗（对齐 bot question modal）。
    pub(in crate::desktop) fn open_solar_quiz(
        &mut self,
        event_id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(event) = solar_event_by_id(&self.page_raw, event_id) else {
            return;
        };
        if event.get("canAnswer").and_then(serde_json::Value::as_bool) != Some(true) {
            return;
        }
        self.solar_calendar.question_id = event_id;
        self.solar_calendar.quiz_answer = None;
        let language = self.preferences.language.clone();
        let radius = cx.theme().radius;
        let group_box = cx.theme().group_box;
        let event = event.clone();
        let desktop = cx.entity();
        window.open_dialog(cx, move |dialog, _, app| {
            let state = &desktop.read(app).solar_calendar;
            let pending = state.pending;
            let selected = state.quiz_answer;
            let name =
                event.get("name").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
            let question =
                event.get("question").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
            let options: Vec<String> = event
                .get("options")
                .and_then(serde_json::Value::as_array)
                .map(|options| {
                    options
                        .iter()
                        .filter_map(|option| option.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let wrong: Vec<i64> = event
                .get("wrongOptions")
                .and_then(serde_json::Value::as_array)
                .map(|values| values.iter().filter_map(serde_json::Value::as_i64).collect())
                .unwrap_or_default();
            let mut options_column = div().v_flex().gap_2();
            for (index, option) in options.iter().enumerate() {
                let (letter, text) = solar_option_parts(option, index);
                let is_wrong = wrong.contains(&(index as i64));
                let select_entity = desktop.clone();
                let selectable = !is_wrong && !pending;
                let row = h_flex()
                    .id(("solar-option", index))
                    .aria_label(format!("{letter}. {text}"))
                    .gap_2()
                    .rounded(radius)
                    .px_3()
                    .py_2()
                    .border_1()
                    .when(selected == Some(index), |row| row.bg(group_box))
                    .when(is_wrong, |row| row.opacity(0.5))
                    .when(selectable, |row| {
                        row.cursor_pointer().on_click(move |_, _, cx| {
                            select_entity.update(cx, |this, cx| {
                                this.solar_calendar.quiz_answer = Some(index);
                                cx.notify();
                            });
                        })
                    })
                    .child(div().font_semibold().child(format!("{letter}.")))
                    .child(div().child(text))
                    .when(is_wrong, |row| {
                        row.child(div().ml_auto().text_xs().child(crate::i18n::text(
                            &language,
                            "已答错",
                            "Wrong answer",
                        )))
                    });
                // 测试窗口只跟踪带 debug selector 的元素（与 pet-log-row 等一致）
                #[cfg(test)]
                let row = row.test_support();
                options_column = options_column.child(row);
            }
            let submit_entity = desktop.clone();
            let can_submit = selected.is_some() && !pending;
            let title = if language == "en-US" {
                format!("{name} · Solar term quiz")
            } else {
                format!("{name} · 节令问答")
            };
            dialog
                .title(title)
                .w(px(460.))
                .child(
                    div()
                        .v_flex()
                        .gap_3()
                        .child(div().text_sm().child(question))
                        .child(options_column),
                )
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("solar-quiz-cancel")
                                .label(crate::i18n::text(&language, "取消", "Cancel"))
                                .ghost()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("solar-quiz-submit")
                                .label(crate::i18n::text(&language, "提交答案", "Submit answer"))
                                .primary()
                                .loading(pending)
                                .disabled(!can_submit)
                                .on_click(move |_, window, cx| {
                                    submit_entity.update(cx, |this, cx| {
                                        this.submit_solar_answer(cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
        cx.notify();
    }

    pub(in crate::desktop) fn submit_solar_answer(&mut self, cx: &mut Context<Self>) {
        let event_id = self.solar_calendar.question_id;
        let Some(answer) = self.solar_calendar.quiz_answer else {
            return;
        };
        let Some(event) = solar_event_by_id(&self.page_raw, event_id) else {
            return;
        };
        let question =
            event.get("question").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        self.solar_calendar_request(
            Some("answer"),
            serde_json::json!({
                "id": event_id,
                "optionIndex": answer,
                "expectedQuestion": question,
            }),
            cx,
        );
    }

    /// 页面渲染。
    pub(in crate::desktop) fn render_solar_calendar(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let now = self.solar_calendar.now_sec;
        let mut panel = div().v_flex().flex_1().min_h_0().gap_3();
        panel = panel.child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_lg().font_semibold().child(crate::i18n::text(
                    language,
                    "节令日历",
                    "Solar Calendar",
                )))
                .child(
                    Button::new("solar-refresh")
                        .label(crate::i18n::text(language, "刷新", "Refresh"))
                        .outline()
                        .loading(self.solar_calendar.pending || self.loading)
                        .disabled(self.selected_account.is_none() || self.solar_calendar.pending)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.solar_calendar_request(None, serde_json::json!({}), cx)
                        })),
                ),
        );
        if self.selected_account.is_none() {
            return panel.child(solar_notice(
                cx,
                crate::i18n::text(language, "请先选择账号", "Select an account first"),
            ));
        }
        if let Some(error) = &self.solar_calendar.error {
            return panel.child(solar_notice(cx, error));
        }
        if self.page_raw.is_null() {
            return panel.child(solar_notice(
                cx,
                crate::i18n::text(language, "暂未获取到节令日历", "No solar calendar data"),
            ));
        }

        let seasons = solar_seasons(&self.page_raw);
        panel = panel.child(h_flex().flex_wrap().gap_2().children(seasons.iter().enumerate().map(
            |(index, season)| {
                let id = js_i64(season.get("id"));
                let name = season.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
                let lit = js_i64(season.get("litCount"));
                let total = js_i64(season.get("totalCount"));
                Button::new(format!("solar-season-{index}"))
                    .label(format!("{name}（{lit}/{total}）"))
                    .when(self.solar_calendar.season_id == id, |button| button.primary())
                    .when(self.solar_calendar.season_id != id, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.solar_calendar.season_id = id;
                        cx.notify();
                    }))
            },
        )));

        let season = seasons
            .into_iter()
            .find(|season| js_i64(season.get("id")) == self.solar_calendar.season_id);
        if let Some(season) = season {
            let lit = js_i64(season.get("litCount"));
            let total = js_i64(season.get("totalCount"));
            let name =
                season.get("name").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
            let rewards = season
                .get("rewards")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let can_claim =
                season.get("canClaim").and_then(serde_json::Value::as_bool) == Some(true);
            let claimed = season.get("claimed").and_then(serde_json::Value::as_bool) == Some(true);
            let season_id = js_i64(season.get("id"));
            let mut bar = div()
                .v_flex()
                .gap_2()
                .rounded(cx.theme().radius)
                .bg(cx.theme().group_box)
                .p_3()
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(div().font_semibold().child(format!(
                            "{name}{}{lit}/{total}",
                            crate::i18n::text(language, "已点亮 ", " lit ")
                        )))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{lit}/{total}")),
                        ),
                );
            if !rewards.is_empty() {
                let label = crate::i18n::text(language, "集齐奖励", "Season rewards");
                let mut chips =
                    h_flex().flex_wrap().gap_2().items_center().child(
                        div().text_sm().text_color(cx.theme().muted_foreground).child(label),
                    );
                for reward in &rewards {
                    chips = chips.child(self.solar_reward_chip(reward, cx));
                }
                let button_label = if claimed {
                    crate::i18n::text(language, "已领取", "Claimed")
                } else if can_claim {
                    crate::i18n::text(language, "领取", "Claim")
                } else {
                    crate::i18n::text(language, "集齐后可领", "Claim when complete")
                };
                chips = chips.child(
                    Button::new("solar-season-claim")
                        .label(button_label)
                        .when(can_claim, |button| button.primary())
                        .when(!can_claim, |button| button.ghost())
                        .loading(self.solar_calendar.pending)
                        .disabled(!can_claim || self.solar_calendar.pending)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.solar_calendar_request(
                                Some("season"),
                                serde_json::json!({ "id": season_id }),
                                cx,
                            )
                        })),
                );
                bar = bar.child(chips);
            }
            panel = panel.child(bar);
        }

        let events: Vec<serde_json::Value> = solar_events(&self.page_raw)
            .into_iter()
            .filter(|event| {
                !selected_season_exists(&self.page_raw, self.solar_calendar.season_id)
                    || seasons_event_in_season(&self.page_raw, event, self.solar_calendar.season_id)
            })
            .collect();
        panel = panel.child(
            div().v_flex().gap_2().children(
                events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| self.solar_event_card(event, index, now, language, cx)),
            ),
        );
        panel
    }

    fn solar_reward_chip(&self, reward: &serde_json::Value, cx: &mut Context<Self>) -> Div {
        let name = reward.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
        let count = js_i64(reward.get("count"));
        let image = reward.get("image").and_then(serde_json::Value::as_str).unwrap_or("");
        let mut chip = h_flex().gap_1().items_center().text_sm();
        if !image.is_empty() {
            chip = chip.child(render_asset_image(image, 18., cx));
        }
        chip.child(div().child(format!("{name} ×{count}")))
    }

    fn solar_event_card(
        &self,
        event: &serde_json::Value,
        index: usize,
        now: i64,
        language: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let event_id = js_i64(event.get("id"));
        let name = event.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
        let lunar = solar_lunar_of(event);
        let poem = solar_poem_of(event);
        let phase_text = solar_phase_text(event, now, language);
        let mut card = div()
            .id(("solar-event", index))
            .v_flex()
            .gap_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .p_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().font_semibold().child(name.to_string()))
                    .when(!lunar.is_empty(), |row| {
                        row.child(
                            div().text_xs().text_color(cx.theme().muted_foreground).child(lunar),
                        )
                    })
                    .child(
                        div()
                            .ml_auto()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(phase_text),
                    ),
            );
        if !poem.is_empty() {
            card = card.child(div().text_xs().text_color(cx.theme().muted_foreground).child(poem));
        }
        let rewards =
            event.get("rewards").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        let can_claim = event.get("canClaim").and_then(serde_json::Value::as_bool) == Some(true);
        let claim_label = solar_state_text(event, now, false, language);
        let pending = self.solar_calendar.pending;
        let mut reward_row = h_flex().flex_wrap().items_center().gap_2().child(
            div().text_xs().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                language,
                "节令奖励",
                "Term rewards",
            )),
        );
        for reward in &rewards {
            reward_row = reward_row.child(self.solar_reward_chip(reward, cx));
        }
        reward_row = reward_row.child(
            Button::new(("solar-claim", index))
                .label(claim_label)
                .when(can_claim, |button| button.primary())
                .when(!can_claim, |button| button.ghost())
                .loading(pending)
                .disabled(!can_claim || pending)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.solar_calendar_request(
                        Some("claim"),
                        serde_json::json!({ "id": event_id }),
                        cx,
                    )
                })),
        );
        card = card.child(reward_row);
        let has_question = event
            .get("question")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|question| !question.is_empty());
        if has_question {
            let question_rewards = event
                .get("questionRewards")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let can_answer =
                event.get("canAnswer").and_then(serde_json::Value::as_bool) == Some(true);
            let quiz_label = solar_state_text(event, now, true, language);
            let mut quiz_row = h_flex().flex_wrap().items_center().gap_2().child(
                div().text_xs().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                    language,
                    "答题奖励",
                    "Quiz rewards",
                )),
            );
            for reward in &question_rewards {
                quiz_row = quiz_row.child(self.solar_reward_chip(reward, cx));
            }
            quiz_row = quiz_row.child(
                Button::new(("solar-quiz", index))
                    .label(quiz_label)
                    .outline()
                    .when(can_answer, |button| button.primary())
                    .disabled(!can_answer || pending)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_solar_quiz(event_id, window, cx)
                    })),
            );
            card = card.child(quiz_row);
            let wrong_today =
                event.get("quizWrongToday").and_then(serde_json::Value::as_bool) == Some(true);
            let error_tips =
                event.get("errorTips").and_then(serde_json::Value::as_str).unwrap_or("");
            if wrong_today && !error_tips.is_empty() {
                card = card.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(error_tips.to_string()),
                );
            }
        }
        card.into_any_element()
    }
}

/// 事件是否属于当前季度（对齐 bot `season.eventIds.includes`）。
fn selected_season_exists(raw: &serde_json::Value, season_id: i64) -> bool {
    solar_seasons(raw).iter().any(|season| js_i64(season.get("id")) == season_id)
}

fn seasons_event_in_season(
    raw: &serde_json::Value,
    event: &serde_json::Value,
    season_id: i64,
) -> bool {
    let event_id = js_i64(event.get("id"));
    solar_seasons(raw)
        .iter()
        .find(|season| js_i64(season.get("id")) == season_id)
        .and_then(|season| season.get("eventIds").and_then(serde_json::Value::as_array).cloned())
        .is_some_and(|ids| ids.iter().any(|id| js_i64(Some(id)) == event_id))
}

fn solar_notice(cx: &Context<FarmDesktop>, message: &str) -> Div {
    div().py_8().text_center().text_color(cx.theme().muted_foreground).child(message.to_string())
}

#[cfg(test)]
mod tests {
    fn open_event() -> serde_json::Value {
        serde_json::json!({
            "id": 401, "name": "重阳", "startTime": 100, "endTime": 200, "status": 2,
            "directClaimed": false, "quizClaimed": false, "quizWrongToday": false,
            "question": "重阳节又被称为什么节？",
            "options": ["A.敬老节", "儿童节"],
            "canClaim": true, "canAnswer": true
        })
    }

    #[test]
    fn option_parts_split_letters_like_bot() {
        assert_eq!(
            super::solar_option_parts("A.敬老节", 0),
            ("A".to_string(), "敬老节".to_string())
        );
        assert_eq!(super::solar_option_parts("儿童节", 1), ("B".to_string(), "儿童节".to_string()));
        assert_eq!(
            super::solar_option_parts("C、母亲节", 2),
            ("C".to_string(), "母亲节".to_string())
        );
    }

    #[test]
    fn phase_and_state_follow_server_window() {
        let mut event = open_event();
        assert_eq!(super::solar_phase(&event, 150), "open");
        assert_eq!(super::solar_state_text(&event, 150, false, "zh-CN"), "领取");
        assert_eq!(super::solar_state_text(&event, 150, true, "zh-CN"), "答题");
        assert_eq!(super::solar_phase(&event, 50), "upcoming");
        assert_eq!(super::solar_state_text(&event, 50, false, "zh-CN"), "1分钟后可领取");
        assert_eq!(super::solar_phase(&event, 250), "expired");
        event["quizWrongToday"] = serde_json::json!(true);
        assert_eq!(super::solar_state_text(&event, 150, true, "zh-CN"), "明日再来");
        event["quizWrongToday"] = serde_json::json!(false);
        event["directClaimed"] = serde_json::json!(true);
        assert_eq!(super::solar_state_text(&event, 150, false, "zh-CN"), "已领取");
        // 有题未答时阶段仍视为进行中（对齐 bot：direct 已领、题未答 → open）
        assert_eq!(super::solar_phase(&event, 150), "open");
        event["quizClaimed"] = serde_json::json!(true);
        assert_eq!(super::solar_phase(&event, 150), "done");
    }

    #[test]
    fn next_boundary_picks_earliest_future_transition() {
        let raw = serde_json::json!({
            "serverTime": 100,
            "events": [
                {"startTime": 50, "endTime": 300, "quizWrongToday": false},
                {"startTime": 400, "endTime": 500, "quizWrongToday": true}
            ]
        });
        assert_eq!(super::solar_next_boundary(&raw), 301);
        let empty = serde_json::json!({"serverTime": 0, "events": []});
        assert_eq!(super::solar_next_boundary(&empty), 0);
    }

    #[test]
    fn reward_text_prefers_rewards_and_falls_back_to_outcome() {
        let value = serde_json::json!({
            "rewards": [{"name": "点券", "count": 200}],
        });
        assert_eq!(super::solar_reward_text(&value, Some("claim"), "zh-CN"), "点券 ×200");
        let wrong = serde_json::json!({ "rewards": [], "correct": false });
        assert_eq!(super::solar_reward_text(&wrong, Some("answer"), "zh-CN"), "明日再来");
        let plain = serde_json::json!({ "rewards": [] });
        assert_eq!(super::solar_reward_text(&plain, Some("season"), "zh-CN"), "领取成功");
    }
}
