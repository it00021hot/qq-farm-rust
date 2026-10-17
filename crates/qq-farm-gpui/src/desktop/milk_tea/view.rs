//! 奶茶活动页渲染与操作（bot `MilkTeaPage.vue` 的 GPUI 紧凑移植）。

use gpui_kit::component::input::Input;
use gpui_kit::component::WindowExt;

use super::super::*;

fn js_i64(value: Option<&serde_json::Value>) -> i64 {
    match value {
        Some(serde_json::Value::Number(number)) => number.as_i64().unwrap_or_default(),
        Some(serde_json::Value::String(text)) => text.trim().parse().unwrap_or_default(),
        _ => 0,
    }
}

fn reward_text(reward: &serde_json::Value) -> String {
    let name = reward.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
    let count = js_i64(reward.get("count"));
    format!("{name} ×{count}")
}

fn rewards_text(rewards: Option<&serde_json::Value>) -> String {
    rewards
        .and_then(serde_json::Value::as_array)
        .map(|rewards| rewards.iter().map(reward_text).collect::<Vec<_>>().join("、"))
        .unwrap_or_default()
}

fn duration_text(seconds: i64, language: &str) -> String {
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if hours > 0 {
        if language == "en-US" {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}小时{minutes}分钟")
        }
    } else if language == "en-US" {
        format!("{minutes}m")
    } else {
        format!("{minutes}分钟")
    }
}

impl FarmDesktop {
    /// 奶茶活动页渲染（数据来自 `page_raw`，分段由 `page_section` 决定）。
    pub(in crate::desktop) fn render_milk_tea(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let mut panel = div().v_flex().flex_1().min_h_0().gap_3();
        panel = panel.child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_lg().font_semibold().child(crate::i18n::text(
                    language,
                    "奶茶活动",
                    "Milk Tea",
                )))
                .child(
                    Button::new("milk-tea-refresh")
                        .label(crate::i18n::text(language, "刷新", "Refresh"))
                        .outline()
                        .loading(self.milk_tea.pending)
                        .disabled(self.selected_account.is_none() || self.milk_tea.pending)
                        .on_click(cx.listener(|this, _, _, cx| this.fetch_milk_tea_page(cx))),
                ),
        );
        if self.selected_account.is_none() {
            return panel.child(milk_tea_notice(
                cx,
                crate::i18n::text(language, "请先选择账号", "Select an account first"),
            ));
        }
        if let Some(error) = &self.milk_tea.error {
            return panel.child(milk_tea_notice(cx, error));
        }
        let raw = &self.page_raw;
        if raw.is_null() {
            return panel.child(milk_tea_notice(
                cx,
                crate::i18n::text(language, "正在读取奶茶活动", "Loading milk tea event"),
            ));
        }
        let settled_open =
            raw.pointer("/summary/shopOpenAt").and_then(serde_json::Value::as_i64).unwrap_or(0);
        if settled_open == 0 {
            return panel.child(self.milk_tea_create_card(cx));
        }
        panel.child(match self.page_section.as_str() {
            "workshop" => self.milk_tea_workshop(cx),
            "hireList" => self.milk_tea_hire(cx),
            "work" => self.milk_tea_work(cx),
            "exchange" => self.milk_tea_exchange(cx),
            "fund" => self.milk_tea_fund(cx),
            "rank" => self.milk_tea_rank(cx),
            "logs" => self.milk_tea_logs(cx),
            _ => self.milk_tea_shop(cx),
        })
    }

    fn milk_tea_create_card(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let recommended: Vec<String> = self
            .page_raw
            .get("recommendedNames")
            .and_then(serde_json::Value::as_array)
            .map(|names| {
                names.iter().filter_map(|name| name.as_str().map(str::to_string)).collect()
            })
            .unwrap_or_default();
        let mut quick = h_flex().flex_wrap().gap_2();
        for (index, name) in recommended.iter().enumerate() {
            let name = name.clone();
            quick = quick.child(
                Button::new(("milk-tea-welcome", index))
                    .label(name.clone())
                    .secondary()
                    .disabled(self.milk_tea.pending)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.milk_tea_request(
                            Some("create"),
                            serde_json::json!({ "name": name }),
                            cx,
                        )
                    })),
            );
        }
        div()
            .v_flex()
            .gap_3()
            .rounded(cx.theme().radius)
            .bg(cx.theme().group_box)
            .p_4()
            .child(div().font_semibold().child(crate::i18n::text(
                language,
                "开店",
                "Open the shop",
            )))
            .child(
                h_flex().gap_2().child(Input::new(&self.milk_tea.create_name)).child(
                    Button::new("milk-tea-create")
                        .label(crate::i18n::text(language, "开店营业", "Open shop"))
                        .primary()
                        .loading(self.milk_tea.pending)
                        .disabled(self.milk_tea.pending)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let name =
                                this.milk_tea.create_name.read(cx).value().trim().to_string();
                            if name.is_empty() {
                                return;
                            }
                            this.milk_tea_request(
                                Some("create"),
                                serde_json::json!({ "name": name }),
                                cx,
                            );
                        })),
                ),
            )
            .child(
                div().text_xs().text_color(cx.theme().muted_foreground).child(crate::i18n::text(
                    language,
                    "推荐店名",
                    "Suggested names",
                )),
            )
            .child(quick)
    }

    fn milk_tea_shop(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let raw = &self.page_raw;
        let summary = raw.get("summary").cloned().unwrap_or(serde_json::Value::Null);
        let shop = raw.get("shop").cloned().unwrap_or(serde_json::Value::Null);
        let settling = raw.get("settling").and_then(serde_json::Value::as_bool) == Some(true);
        let next_revenue = js_i64(shop.get("nextLevelRevenue"));
        let total_revenue = js_i64(summary.get("totalRevenue"));
        let hire_disabled =
            summary.get("hireDisabled").and_then(serde_json::Value::as_bool) == Some(true);
        let can_upgrade =
            summary.get("canUpgrade").and_then(serde_json::Value::as_bool) == Some(true);
        let shop_name =
            summary.get("shopName").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        let level = js_i64(summary.get("level"));
        let mut slots = h_flex().flex_wrap().gap_2();
        for (index, slot) in shop
            .get("slots")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let name = slot
                .pointer("/employee/name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            let talent = slot.get("talentName").and_then(serde_json::Value::as_str).unwrap_or("");
            slots = slots.child(
                div()
                    .text_xs()
                    .rounded_full()
                    .bg(cx.theme().muted)
                    .px_2()
                    .py_1()
                    .child(format!("#{index} {name} {talent}")),
            );
        }
        let mut recipes = div().grid().grid_cols(4).gap_2();
        for (index, recipe) in raw
            .get("recipes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let unlocked =
                recipe.get("unlocked").and_then(serde_json::Value::as_bool) == Some(true);
            let name = recipe.get("recipeName").and_then(serde_json::Value::as_str).unwrap_or("");
            let stock = js_i64(recipe.get("stockCups"));
            let image = recipe.get("image").and_then(serde_json::Value::as_str).unwrap_or("");
            recipes = recipes.child(
                div()
                    .id(("milk-tea-recipe", index))
                    .v_flex()
                    .items_center()
                    .gap_1()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .when(!unlocked, |card| card.opacity(0.5))
                    .when(!image.is_empty(), |card| card.child(render_asset_image(image, 32., cx)))
                    .child(div().text_xs().child(name.to_string()))
                    .child(
                        div().text_xs().text_color(cx.theme().muted_foreground).child(format!(
                            "{stock}{}",
                            crate::i18n::text(language, "杯", " cups")
                        )),
                    ),
            );
        }
        let boost_preview = rewards_text(shop.get("boostPreviewRewards"));
        let sales_item_id = js_i64(shop.get("salesItemId"));
        let pending_rewards = shop
            .get("pendingRewards")
            .and_then(serde_json::Value::as_array)
            .map(|rewards| rewards.iter().map(reward_text).collect::<Vec<_>>().join("、"))
            .unwrap_or_default();
        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().font_semibold().child(shop_name))
                            .child(div().text_xs().child(format!("Lv.{level}")))
                            .when(hire_disabled, |row| {
                                row.child(div().text_xs().child(crate::i18n::text(
                                    language,
                                    "已关闭雇佣",
                                    "Hiring disabled",
                                )))
                            })
                            .child(
                                div().ml_auto().text_xs().text_color(cx.theme().muted_foreground).child(format!(
                                    "{}：{total_revenue} / {next_revenue}",
                                    crate::i18n::text(language, "营业额", "Revenue"),
                                )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("milk-tea-rename")
                                    .label(crate::i18n::text(language, "修改店名", "Rename"))
                                    .secondary()
                                    .disabled(self.milk_tea.pending)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_milk_tea_rename_dialog(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("milk-tea-upgrade")
                                    .label(crate::i18n::text(language, "升级店铺", "Upgrade shop"))
                                    .primary()
                                    .disabled(!can_upgrade || self.milk_tea.pending)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.milk_tea_request(
                                            Some("upgrade"),
                                            serde_json::json!({ "expectedLevel": level }),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("milk-tea-hire-setting")
                                    .label(if hire_disabled {
                                        crate::i18n::text(language, "开启雇佣", "Enable hiring")
                                    } else {
                                        crate::i18n::text(language, "关闭雇佣", "Disable hiring")
                                    })
                                    .secondary()
                                    .disabled(self.milk_tea.pending)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.milk_tea_request(
                                            Some("hireSetting"),
                                            serde_json::json!({ "disabled": !hire_disabled }),
                                            cx,
                                        )
                                    })),
                            ),
                    ),
            )
            .child(
                div()
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
                            .child(div().child(crate::i18n::text(language, "收银台", "Cashier")))
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                                format!(
                                    "{}s / {}s",
                                    js_i64(shop.get("cashierElapsedSec")),
                                    js_i64(shop.get("cashierCapacitySec"))
                                ),
                            ))
                            .when(!pending_rewards.is_empty(), |row| {
                                row.child(div().text_xs().child(pending_rewards.clone()))
                            })
                            .child(
                                Button::new("milk-tea-claim")
                                    .label(crate::i18n::text(language, "领取收益", "Claim earnings"))
                                    .primary()
                                    .loading(self.milk_tea.pending)
                                    .disabled(self.milk_tea.pending)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.milk_tea_request(Some("claim"), serde_json::json!({}), cx)
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(div().child(crate::i18n::text(language, "加速营业", "Boost")))
                            .when(!boost_preview.is_empty(), |row| {
                                row.child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                                    boost_preview.clone(),
                                ))
                            })
                            .child(Input::new(&self.milk_tea.boost_count).w(px(96.)))
                            .child(
                                Button::new("milk-tea-boost")
                                    .label(crate::i18n::text(language, "使用加速", "Use boost"))
                                    .secondary()
                                    .loading(self.milk_tea.pending)
                                    .disabled(self.milk_tea.pending || settling)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let count = this
                                            .milk_tea
                                            .boost_count
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .parse::<i64>()
                                            .unwrap_or(1)
                                            .clamp(1, 9_999);
                                        this.milk_tea_request(
                                            Some("boost"),
                                            serde_json::json!({
                                                "count": count,
                                                "expectedCosts": [{ "id": sales_item_id, "count": 1 }],
                                            }),
                                            cx,
                                        )
                                    })),
                            ),
                    )
                    .when(
                        shop.get("slots").and_then(serde_json::Value::as_array).is_some_and(|slots| !slots.is_empty()),
                        |card| card.child(slots),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .p_3()
                    .child(div().text_sm().font_medium().child(crate::i18n::text(
                        language,
                        "奶茶配方",
                        "Recipes",
                    )))
                    .child(recipes),
            )
    }

    fn milk_tea_workshop(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let fruits = self
            .page_raw
            .get("fruits")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut rows = div().v_flex().gap_2();
        if fruits.is_empty() {
            rows = rows.child(milk_tea_notice(
                cx,
                crate::i18n::text(language, "暂无可选果实", "No fruits available"),
            ));
        }
        for (index, fruit) in fruits.iter().enumerate() {
            let uid = fruit
                .get("uid")
                .map(|value| match value {
                    serde_json::Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default();
            let selected = self.milk_tea.craft_selected.contains(&uid);
            let locked = fruit.get("locked").and_then(serde_json::Value::as_bool) == Some(true);
            let name = fruit.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
            let count = js_i64(fruit.get("count"));
            let image = fruit.get("image").and_then(serde_json::Value::as_str).unwrap_or("");
            let toggle_uid = uid.clone();
            rows = rows.child(
                h_flex()
                    .id(("milk-tea-fruit", index))
                    .items_center()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(if selected { cx.theme().primary } else { cx.theme().border })
                    .when(locked, |row| row.opacity(0.5))
                    .p_2()
                    .when(!locked && !self.milk_tea.pending, |row| {
                        row.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                            if !this.milk_tea.craft_selected.insert(toggle_uid.clone()) {
                                this.milk_tea.craft_selected.remove(&toggle_uid);
                            }
                            cx.notify();
                        }))
                    })
                    .when(!image.is_empty(), |row| row.child(render_asset_image(image, 20., cx)))
                    .child(div().text_sm().child(name.to_string()))
                    .child(div().ml_auto().text_xs().child(format!("×{count}"))),
            );
        }
        div()
            .v_flex()
            .gap_3()
            .child(div().text_sm().font_medium().child(crate::i18n::text(
                language,
                "制作奶茶（勾选果实整堆投入）",
                "Craft drinks (select fruit stacks)",
            )))
            .child(rows)
            .child(
                Button::new("milk-tea-craft")
                    .label(crate::i18n::text(language, "开始制作", "Craft"))
                    .primary()
                    .loading(self.milk_tea.pending)
                    .disabled(self.milk_tea.pending || self.milk_tea.craft_selected.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| {
                        let fruits = this
                            .page_raw
                            .get("fruits")
                            .and_then(serde_json::Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        let selection: Vec<serde_json::Value> = fruits
                            .iter()
                            .filter(|fruit| {
                                let uid = fruit
                                    .get("uid")
                                    .map(|value| match value {
                                        serde_json::Value::String(text) => text.clone(),
                                        other => other.to_string(),
                                    })
                                    .unwrap_or_default();
                                this.milk_tea.craft_selected.contains(&uid)
                            })
                            .map(|fruit| {
                                serde_json::json!({
                                    "uid": fruit.get("uid").cloned().unwrap_or_default(),
                                    "count": js_i64(fruit.get("count")),
                                })
                            })
                            .collect();
                        if selection.is_empty() {
                            return;
                        }
                        this.milk_tea_request(
                            Some("craft"),
                            serde_json::json!({ "fruits": selection }),
                            cx,
                        );
                        this.milk_tea.craft_selected.clear();
                    })),
            )
    }

    fn milk_tea_hire(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let raw = &self.page_raw;
        let hire = raw.get("hire").cloned().unwrap_or(serde_json::Value::Null);
        let mut durations = h_flex().flex_wrap().gap_2();
        for (index, duration) in raw
            .get("durations")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let id = js_i64(duration.get("durationId"));
            let label = format!(
                "{} · {}{}",
                duration_text(js_i64(duration.get("durationSec")), language),
                js_i64(duration.get("cost")),
                crate::i18n::text(language, "奶茶券", " tokens")
            );
            durations = durations.child(
                Button::new(("milk-tea-duration", index))
                    .label(label)
                    .when(self.milk_tea.hire_duration_id == id, |button| button.primary())
                    .when(self.milk_tea.hire_duration_id != id, |button| button.secondary())
                    .disabled(self.milk_tea.pending)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.milk_tea.hire_duration_id = id;
                        this.fetch_milk_tea_page(cx);
                    })),
            );
        }
        let mut candidates = div().v_flex().gap_2();
        let wage_total = js_i64(hire.get("wageTotal"));
        let entries: Vec<serde_json::Value> = hire
            .get("friends")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .chain(
                hire.get("robots")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            )
            .collect();
        for (index, candidate) in entries.iter().enumerate() {
            let gid = js_i64(candidate.pointer("/friend/gid"));
            let name = candidate
                .pointer("/friend/name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            let level = js_i64(candidate.pointer("/friend/level"));
            let state = js_i64(candidate.get("state"));
            let rehire_at = js_i64(candidate.get("rehireAvailableAt"));
            let now = js_i64(raw.get("serverTime"));
            let available = state == 0 && rehire_at <= now;
            candidates = candidates.child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .p_2()
                    .child(div().text_sm().child(format!("{name} Lv.{level}")))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                        if available {
                            crate::i18n::text(language, "可雇佣", "Available")
                        } else {
                            crate::i18n::text(language, "暂不可用", "Unavailable")
                        },
                    ))
                    .child(
                        Button::new(("milk-tea-hire", index))
                            .label(crate::i18n::text(language, "雇佣", "Hire"))
                            .primary()
                            .disabled(!available || self.milk_tea.pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.milk_tea_request(
                                    Some("hire"),
                                    serde_json::json!({
                                        "targetGid": gid.to_string(),
                                        "expectedCosts": [{ "id": 1041, "count": wage_total }],
                                    }),
                                    cx,
                                )
                            }))
                            .into_any_element(),
                    ),
            );
        }
        if entries.is_empty() {
            candidates = candidates.child(milk_tea_notice(
                cx,
                crate::i18n::text(language, "暂无可雇佣名单", "No candidates"),
            ));
        }
        div()
            .v_flex()
            .gap_3()
            .child(durations)
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!(
                "{}：{wage_total}{}",
                crate::i18n::text(language, "工资", "Wage"),
                crate::i18n::text(language, "奶茶券", " tokens")
            )))
            .child(candidates)
    }

    fn milk_tea_work(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let raw = &self.page_raw;
        let work = raw.get("work").cloned().unwrap_or(serde_json::Value::Null);
        let shift = work.get("myShift").cloned().unwrap_or(serde_json::Value::Null);
        let now = js_i64(raw.get("serverTime"));
        let mut interactions = div().v_flex().gap_2();
        for (index, interaction) in raw
            .get("interactions")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let kind = js_i64(interaction.get("interactionId"));
            let name = interaction.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
            let description =
                interaction.get("description").and_then(serde_json::Value::as_str).unwrap_or("");
            let ready = interaction.get("ready").and_then(serde_json::Value::as_bool) == Some(true);
            let interact_done =
                work.get("interactDone").and_then(serde_json::Value::as_bool) == Some(true);
            let shift_active = !shift.is_null() && js_i64(shift.get("endAt")) > now;
            interactions = interactions.child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .p_2()
                    .child(div().text_sm().font_medium().child(name.to_string()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(description.to_string()),
                    )
                    .child(
                        Button::new(("milk-tea-interact", index))
                            .label(crate::i18n::text(language, "互动", "Interact"))
                            .secondary()
                            .disabled(
                                !ready || !shift_active || interact_done || self.milk_tea.pending,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let shift = this
                                    .page_raw
                                    .pointer("/work/myShift")
                                    .cloned()
                                    .unwrap_or_default();
                                this.milk_tea_request(
                                    Some("interact"),
                                    serde_json::json!({
                                        "interactionType": kind,
                                        "expectedShiftId": shift
                                            .get("shiftId")
                                            .map(|value| match value {
                                                serde_json::Value::String(text) => text.clone(),
                                                other => other.to_string(),
                                            })
                                            .unwrap_or_default(),
                                    }),
                                    cx,
                                )
                            })),
                    ),
            );
        }
        let mut slips = div().v_flex().gap_1();
        for (index, slip) in work
            .get("slips")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let shop_name = slip.get("shopName").and_then(serde_json::Value::as_str).unwrap_or("");
            let wage = js_i64(slip.pointer("/wages/finalWage"));
            let status = js_i64(slip.get("wageStatus"));
            slips = slips.child(div().id(("milk-tea-slip", index)).text_xs().child(format!(
                "{shop_name} · {wage}{} · {}",
                crate::i18n::text(language, "奶茶券", " tokens"),
                if status == 1 {
                    crate::i18n::text(language, "待领取", "Claimable")
                } else {
                    crate::i18n::text(language, "已结算", "Settled")
                }
            )));
        }
        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().group_box)
                    .p_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div().font_semibold().child(
                                    work.get("talentName")
                                        .and_then(serde_json::Value::as_str)
                                        .unwrap_or("—")
                                        .to_string(),
                                ),
                            )
                            .child(
                                div().text_xs().text_color(cx.theme().muted_foreground).child(
                                    work.get("talentDescription")
                                        .and_then(serde_json::Value::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                ),
                            )
                            .child(
                                Button::new("milk-tea-wages")
                                    .label(crate::i18n::text(language, "领取工资", "Claim wages"))
                                    .primary()
                                    .loading(self.milk_tea.pending)
                                    .disabled(self.milk_tea.pending)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.milk_tea_request(
                                            Some("wages"),
                                            serde_json::json!({}),
                                            cx,
                                        )
                                    })),
                            ),
                    )
                    .when(!shift.is_null(), |card| {
                        card.child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                            format!(
                                "{} {} · {} · {}/{}",
                                crate::i18n::text(language, "在", "at"),
                                shift
                                    .get("shopName")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or(""),
                                duration_text((js_i64(shift.get("endAt")) - now).max(0), language),
                                js_i64(shift.get("interactCount")),
                                js_i64(shift.get("interactLimit")),
                            ),
                        ))
                    }),
            )
            .child(interactions)
            .child(slips)
    }

    fn milk_tea_exchange(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let goods = self
            .page_raw
            .get("goods")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut grid = div().grid().grid_cols(3).gap_2();
        for (index, entry) in goods.iter().enumerate() {
            let available =
                entry.get("available").and_then(serde_json::Value::as_bool) == Some(true);
            let name = entry.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
            let rewards = rewards_text(entry.get("rewards"));
            let costs = rewards_text(entry.get("costs"));
            let remaining = entry.get("remaining").and_then(serde_json::Value::as_i64);
            grid =
                grid.child(
                    div()
                        .id(("milk-tea-goods", index))
                        .v_flex()
                        .gap_1()
                        .p_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .when(!available, |card| card.opacity(0.5))
                        .child(div().text_sm().font_medium().child(name.to_string()))
                        .child(div().text_xs().child(rewards))
                        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                            format!("{}：{costs}", crate::i18n::text(language, "费用", "Cost"),),
                        ))
                        .when(remaining.is_some(), |card| {
                            card.child(div().text_xs().child(format!(
                                "{}：{}",
                                crate::i18n::text(language, "剩余", "Remaining"),
                                remaining.unwrap_or(0),
                            )))
                        })
                        .child(
                            Button::new(("milk-tea-exchange", index))
                                .label(crate::i18n::text(language, "兑换", "Exchange"))
                                .primary()
                                .disabled(!available || self.milk_tea.pending)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let goods = this
                                        .page_raw
                                        .get("goods")
                                        .and_then(serde_json::Value::as_array)
                                        .and_then(|goods| goods.get(index))
                                        .cloned()
                                        .unwrap_or_default();
                                    let count = this
                                        .milk_tea
                                        .exchange_count
                                        .read(cx)
                                        .value()
                                        .trim()
                                        .parse::<i64>()
                                        .unwrap_or(1)
                                        .clamp(1, 9_999);
                                    let expected: Vec<serde_json::Value> = goods
                                        .get("costs")
                                        .and_then(serde_json::Value::as_array)
                                        .map(|costs| {
                                            costs
                                                .iter()
                                                .map(|cost| {
                                                    serde_json::json!({
                                                        "id": js_i64(cost.get("id")),
                                                        "count": js_i64(cost.get("count")),
                                                    })
                                                })
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    this.milk_tea_request(
                                        Some("exchange"),
                                        serde_json::json!({
                                            "goodsId": goods.get("id").cloned().unwrap_or_default(),
                                            "count": count,
                                            "expectedCosts": expected,
                                        }),
                                        cx,
                                    )
                                })),
                        ),
                );
        }
        div()
            .v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().child(crate::i18n::text(
                        language,
                        "兑换数量",
                        "Exchange count",
                    )))
                    .child(Input::new(&self.milk_tea.exchange_count).w(px(96.))),
            )
            .child(grid)
    }

    fn milk_tea_fund(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let fund = self.page_raw.get("fund").cloned().unwrap_or(serde_json::Value::Null);
        let unlocked = fund.get("unlocked").and_then(serde_json::Value::as_bool) == Some(true);
        let costs = rewards_text(fund.get("costs"));
        let expected: Vec<serde_json::Value> = fund
            .get("costs")
            .and_then(serde_json::Value::as_array)
            .map(|costs| {
                costs
                    .iter()
                    .map(|cost| {
                        serde_json::json!({
                            "id": js_i64(cost.get("id")),
                            "count": js_i64(cost.get("count")),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut tiers = div().v_flex().gap_2();
        for (index, tier) in fund
            .get("tiers")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let state = js_i64(tier.get("state"));
            let rewards = rewards_text(tier.get("rewards"));
            let conditions = tier
                .get("conditions")
                .and_then(serde_json::Value::as_array)
                .map(|conditions| {
                    conditions
                        .iter()
                        .map(|condition| {
                            format!(
                                "{} {}/{}",
                                condition
                                    .get("type")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or(""),
                                js_i64(condition.get("current")),
                                js_i64(condition.get("target")),
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" · ")
                })
                .unwrap_or_default();
            tiers = tiers.child(
                div()
                    .id(("milk-tea-fund-tier", index))
                    .v_flex()
                    .gap_1()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(if state == 2 { cx.theme().primary } else { cx.theme().border })
                    .p_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .child(format!("#{}", js_i64(tier.get("id")))),
                            )
                            .child(div().text_xs().child(rewards))
                            .child(div().ml_auto().text_xs().child(format!("state {state}"))),
                    )
                    .child(
                        div().text_xs().text_color(cx.theme().muted_foreground).child(conditions),
                    ),
            );
        }
        div()
            .v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().child(if unlocked {
                        crate::i18n::text(language, "基金已解锁", "Fund unlocked")
                    } else {
                        crate::i18n::text(language, "基金未解锁", "Fund locked")
                    }))
                    .when(!costs.is_empty(), |row| {
                        row.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(costs.clone()),
                        )
                    })
                    .child(
                        Button::new("milk-tea-unlock")
                            .label(crate::i18n::text(language, "解锁基金", "Unlock fund"))
                            .primary()
                            .loading(self.milk_tea.pending)
                            .disabled(unlocked || self.milk_tea.pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.milk_tea_request(
                                    Some("unlock"),
                                    serde_json::json!({ "expectedCosts": expected.clone() }),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("milk-tea-fund-claim")
                            .label(crate::i18n::text(language, "领取基金奖励", "Claim fund"))
                            .secondary()
                            .loading(self.milk_tea.pending)
                            .disabled(self.milk_tea.pending)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.milk_tea_request(Some("fundClaim"), serde_json::json!({}), cx)
                            })),
                    ),
            )
            .child(tiers)
    }

    fn milk_tea_rank(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let rank = self.page_raw.get("rank").cloned().unwrap_or(serde_json::Value::Null);
        let has_more = rank.get("hasMore").and_then(serde_json::Value::as_bool) == Some(true);
        let page = js_i64(rank.get("page")).max(1);
        let mut rows = div().v_flex().gap_1();
        for (index, entry) in rank
            .get("ranks")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            rows = rows.child(div().id(("milk-tea-rank", index)).text_sm().child(format!(
                "{} · {} · {} · Lv.{}",
                js_i64(entry.get("rank")),
                entry.get("shopName").and_then(serde_json::Value::as_str).unwrap_or(""),
                js_i64(entry.get("totalRevenue")),
                js_i64(entry.get("level")),
            )));
        }
        div()
            .v_flex()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().child(format!(
                        "{}：{}",
                        crate::i18n::text(language, "我的排名", "My rank"),
                        js_i64(rank.get("myRank")),
                    )))
                    .child(
                        Button::new("milk-tea-rank-prev")
                            .label(crate::i18n::text(language, "上一页", "Previous"))
                            .secondary()
                            .disabled(page <= 1 || self.milk_tea.pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.milk_tea.rank_page = (page - 1).max(1);
                                this.fetch_milk_tea_page(cx);
                            })),
                    )
                    .child(
                        Button::new("milk-tea-rank-next")
                            .label(crate::i18n::text(language, "下一页", "Next"))
                            .secondary()
                            .disabled(!has_more || self.milk_tea.pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.milk_tea.rank_page = page + 1;
                                this.fetch_milk_tea_page(cx);
                            })),
                    ),
            )
            .child(rows)
    }

    fn milk_tea_logs(&self, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let logs = self.page_raw.get("logs").cloned().unwrap_or(serde_json::Value::Null);
        let day = js_i64(logs.get("day"));
        let mut rows = div().v_flex().gap_1();
        for (index, hire) in logs
            .get("hires")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let name =
                hire.pointer("/friend/name").and_then(serde_json::Value::as_str).unwrap_or("");
            let wage = js_i64(hire.pointer("/wages/finalWage"));
            let interaction = hire
                .get("interactionResult")
                .and_then(|result| result.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            rows = rows.child(div().id(("milk-tea-log-hire", index)).text_xs().child(format!(
                "{name} · {wage}{} · {interaction}",
                crate::i18n::text(language, "奶茶券", " tokens"),
            )));
        }
        div()
            .v_flex()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().child(format!("Day {day}")))
                    .child(
                        Button::new("milk-tea-logs-prev")
                            .label(crate::i18n::text(language, "前一天", "Previous day"))
                            .secondary()
                            .disabled(self.milk_tea.pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if day > 86_400 {
                                    this.milk_tea.logs_day = day - 86_400;
                                    this.fetch_milk_tea_page(cx);
                                }
                            })),
                    ),
            )
            .child(div().text_sm().child(format!(
                "{}：{} · {} · {}：{}",
                crate::i18n::text(language, "奶茶券", "Tokens"),
                js_i64(logs.get("token")),
                js_i64(logs.get("coin")),
                crate::i18n::text(language, "杯数", "Cups"),
                js_i64(logs.get("cups")),
            )))
            .child(rows)
    }

    /// 改名弹窗。
    pub(in crate::desktop) fn open_milk_tea_rename_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = self.preferences.language.clone();
        let desktop = cx.entity();
        window.open_dialog(cx, move |dialog, _, app| {
            let pending = desktop.read(app).milk_tea.pending;
            let confirm_entity = desktop.clone();
            dialog
                .title(crate::i18n::text(&language, "修改店名", "Rename shop"))
                .w(px(420.))
                .child(Input::new(&desktop.read(app).milk_tea.rename_name))
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("milk-tea-rename-cancel")
                                .label(crate::i18n::text(&language, "取消", "Cancel"))
                                .ghost()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("milk-tea-rename-confirm")
                                .label(crate::i18n::text(&language, "确认改名", "Confirm"))
                                .primary()
                                .loading(pending)
                                .disabled(pending)
                                .on_click(move |_, window, cx| {
                                    confirm_entity.update(cx, |this, cx| {
                                        let name = this
                                            .milk_tea
                                            .rename_name
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .to_string();
                                        if !name.is_empty() {
                                            this.milk_tea_request(
                                                Some("rename"),
                                                serde_json::json!({ "name": name }),
                                                cx,
                                            );
                                        }
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
        cx.notify();
    }
}

fn milk_tea_notice(cx: &Context<FarmDesktop>, message: &str) -> Div {
    div().py_8().text_center().text_color(cx.theme().muted_foreground).child(message.to_string())
}
