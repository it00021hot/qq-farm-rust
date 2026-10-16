use gpui_kit::component::WindowExt;

use super::super::super::*;

/// 千分位（对齐 bot LandDetailModal 的 toLocaleString）
fn format_amount(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut output = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(ch);
    }
    if value < 0 {
        output.insert(0, '-');
    }
    output
}

/// 文案取词（format 参数需要先落 String，包一层避免临时值生命周期问题）
fn tr(language: &str, chinese: &str, english: &str) -> String {
    crate::i18n::text(language, chinese, english).to_string()
}

/// 指标行（对齐 bot LandDetailModal 的 metrics 表）。
const METRICS: [(&str, &str, &str, &str); 5] = [
    ("yieldBonus", "产量加成", "Yield bonus", "%"),
    ("growthReduction", "成长提速", "Growth speed-up", "%"),
    ("expBonus", "经验加成", "EXP bonus", "%"),
    ("mutantMultiplier", "变异概率", "Mutation chance", "x"),
    ("resonanceBonus", "共鸣加成", "Resonance bonus", "%"),
];

/// 余额对比：1001=金币 1002=点券 1005=金豆豆（对齐 bot LandDetailModal balances）
fn balance_of(balances: Option<&serde_json::Value>, cost_id: &str) -> Option<i64> {
    let key = match cost_id {
        "1001" => "gold",
        "1002" => "coupon",
        "1005" => "goldBean",
        _ => return None,
    };
    balances?.get(key).and_then(serde_json::Value::as_i64)
}

impl FarmDesktop {
    /// 打开土地详情弹窗（bot e5f3552：LandCard「土地详情」入口），
    /// 同时按账号拉取等级/余额做条件对比（对齐 bot 传入 player 状态）。
    pub(in crate::desktop) fn open_land_detail(
        &mut self,
        land: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.farm.lands.pending_detail = Some(land);
        self.farm.lands.detail_dialog_open = true;
        self.farm.lands.detail_balances = None;
        self.farm.lands.detail_balances_loading = self.selected_account.is_some();
        self.farm.lands.detail_generation = self.farm.lands.detail_generation.wrapping_add(1);
        let generation = self.farm.lands.detail_generation;

        if let Some(account_id) = self.selected_account.clone() {
            let app = Arc::clone(&self.app);
            let runtime = Arc::clone(&self.tokio_runtime);
            cx.spawn(async move |this, cx| {
                let balances = runtime
                    .spawn(async move {
                        let status =
                            qq_farm_app::farm::panel_status_with_progress(&app, &account_id);
                        let bag = qq_farm_app::farm::bag(&app, &account_id).await.ok();
                        let item = |id: i64| {
                            bag.as_ref()
                                .and_then(|detail| detail.items.iter().find(|entry| entry.id == id))
                                .map(|entry| entry.count)
                        };
                        serde_json::json!({
                            "level": status.level,
                            "gold": status.gold,
                            "coupon": item(1002),
                            "goldBean": item(1005),
                        })
                    })
                    .await
                    .unwrap_or_default();
                let _ = this.update(cx, |this, cx| {
                    if this.farm.lands.detail_generation != generation {
                        return;
                    }
                    this.farm.lands.detail_balances_loading = false;
                    this.farm.lands.detail_balances = Some(balances);
                    cx.notify();
                });
            })
            .detach();
        }

        let language = self.preferences.language.clone();
        let land_id = self
            .farm
            .lands
            .pending_detail
            .as_ref()
            .and_then(|land| land.get("id"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        let title =
            tr(&language, &format!("土地详情 #{land_id}"), &format!("Land Details #{land_id}"));
        let desktop = cx.entity();
        let state_entity = desktop.clone();
        let close_entity = desktop;
        window.open_dialog(cx, move |dialog, _, app| {
            let close_entity = close_entity.clone();
            let content = state_entity.read(app).render_land_detail_content(app);
            dialog.title(title.clone()).w(px(460.)).child(content).on_close(move |_, _, cx| {
                close_entity.update(cx, |this, cx| {
                    this.farm.lands.pending_detail = None;
                    this.farm.lands.detail_dialog_open = false;
                    this.farm.lands.detail_balances = None;
                    cx.notify();
                });
            })
        });
    }

    /// 详情内容（每次弹窗渲染时从 state 读取；对齐 bot LandDetailModal 的信息结构）。
    fn render_land_detail_content(&self, app: &App) -> Div {
        let theme = app.theme();
        let language = self.preferences.language.as_str();
        let Some(land) = self.farm.lands.pending_detail.clone() else {
            return div();
        };
        let Some(info) = land.get("landInfo").filter(|info| !info.is_null()).cloned() else {
            return div().text_sm().text_color(theme.muted_foreground).child(crate::i18n::text(
                language,
                "暂无土地详情数据",
                "No land details available",
            ));
        };
        let mode = info.get("mode").and_then(serde_json::Value::as_str).unwrap_or("");
        let current = info.get("current").filter(|v| !v.is_null());
        let next = info.get("next").filter(|v| !v.is_null());
        let balances = self.farm.lands.detail_balances.as_ref();

        // 当前 → 下一级 土壤对比
        let mut content = div().v_flex().gap_3();
        if current.is_some() || next.is_some() {
            let mut soils = h_flex().items_center().justify_center().gap_4();
            for level in [current, next].into_iter().flatten() {
                let name = level.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
                let image = level.get("image").and_then(serde_json::Value::as_str).unwrap_or("");
                let mut soil = div().v_flex().items_center().gap_1();
                if !image.is_empty() {
                    soil = soil.child(render_asset_image(image, 56., app));
                }
                soil = soil.child(div().text_sm().font_medium().child(name.to_string()));
                soils = soils.child(soil);
            }
            if current.is_some() && next.is_some() {
                soils = soils.child(div().text_lg().text_color(theme.muted_foreground).child("→"));
            }
            content = content.child(soils);
        }

        // 加成对比表
        let mut metrics = div().v_flex();
        metrics = metrics.child(
            h_flex()
                .justify_between()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(crate::i18n::text(language, "土地加成", "Land bonuses"))
                .child(crate::i18n::text(
                    language,
                    if current.is_some() { "当前 → 升级后" } else { "解锁后" },
                    if current.is_some() { "Current → After upgrade" } else { "After unlock" },
                )),
        );
        for (key, label_zh, label_en, unit) in METRICS {
            let value = |level: Option<&serde_json::Value>| -> String {
                let Some(level) = level else { return "—".to_string() };
                let raw = level.get(key).and_then(serde_json::Value::as_f64).unwrap_or(0.);
                if key == "mutantMultiplier" && raw == 0. {
                    return crate::i18n::text(language, "无", "None").to_string();
                }
                format!("{raw}{unit}")
            };
            let current_text = value(current);
            let next_text = value(next);
            let improved = next
                .and_then(|n| n.get(key))
                .and_then(serde_json::Value::as_f64)
                .zip(current.and_then(|c| c.get(key)).and_then(serde_json::Value::as_f64))
                .is_some_and(|(n, c)| n > c);
            metrics = metrics.child(
                h_flex()
                    .justify_between()
                    .text_sm()
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(crate::i18n::text(language, label_zh, label_en)),
                    )
                    .child(h_flex().gap_2().child(current_text).when(
                        current.is_some() && next.is_some(),
                        |row| {
                            row.child(
                                div()
                                    .text_color(if improved {
                                        theme.success
                                    } else {
                                        theme.muted_foreground
                                    })
                                    .child(format!(
                                        "{next_text}{}",
                                        if improved { " ↑" } else { "" }
                                    )),
                            )
                        },
                    )),
            );
        }
        content = content.child(metrics);

        if mode == "max" {
            return content.child(
                div()
                    .mt_2()
                    .rounded(theme.radius)
                    .bg(theme.success.opacity(0.14))
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(theme.success)
                    .child(crate::i18n::text(language, "当前已满级", "Already at max level")),
            );
        }

        // 解锁/升级条件
        let mut conditions = div().v_flex().gap_2().mt_1();
        conditions = conditions.child(div().text_sm().font_medium().child(crate::i18n::text(
            language,
            if mode == "unlock" { "解锁条件" } else { "升级条件" },
            if mode == "unlock" { "Unlock requirements" } else { "Upgrade requirements" },
        )));

        if let Some(need_level) = info.get("needLevel").and_then(serde_json::Value::as_i64) {
            let player_level =
                balances.and_then(|b| b.get("level")).and_then(serde_json::Value::as_i64);
            let met = player_level.is_some_and(|level| level >= need_level);
            conditions = conditions.child(
                h_flex()
                    .justify_between()
                    .text_sm()
                    .child(tr(
                        language,
                        &format!("种植等级达到 {need_level} 级"),
                        &format!("Planting level {need_level} required"),
                    ))
                    .child(
                        div()
                            .text_color(match met {
                                true => theme.success,
                                false => theme.danger,
                            })
                            .child(match player_level {
                                Some(level) => format!("{level} / {need_level}"),
                                None => {
                                    if self.farm.lands.detail_balances_loading {
                                        "…".to_string()
                                    } else {
                                        format!("— / {need_level}")
                                    }
                                }
                            }),
                    ),
            );
        }
        if let Some(prerequisite) = info.get("prerequisite").filter(|value| !value.is_null()) {
            let land_id =
                prerequisite.get("landId").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let met = prerequisite.get("met").and_then(serde_json::Value::as_bool).unwrap_or(false);
            conditions = conditions.child(
                h_flex()
                    .justify_between()
                    .text_sm()
                    .child(tr(
                        language,
                        &format!("解锁第 {land_id} 块土地"),
                        &format!("Unlock land #{land_id} first"),
                    ))
                    .child(div().text_color(if met { theme.success } else { theme.danger }).child(
                        crate::i18n::text(
                            language,
                            if met { "已满足" } else { "未满足" },
                            if met { "Met" } else { "Not met" },
                        ),
                    )),
            );
        }
        if let (Some(current), Some(_next)) = (current, next) {
            let name = current.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
            conditions = conditions.child(
                h_flex()
                    .justify_between()
                    .text_sm()
                    .child(tr(
                        language,
                        &format!("当前地块为{name}"),
                        &format!("Current soil is {name}"),
                    ))
                    .child(div().text_color(theme.success).child(crate::i18n::text(
                        language,
                        "已满足",
                        "Met",
                    ))),
            );
        }
        let costs =
            info.get("costs").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        for cost in costs {
            let cost_id =
                cost.get("id").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
            let count_text = cost.get("count").and_then(serde_json::Value::as_str).unwrap_or("0");
            let required: i128 = count_text.parse().unwrap_or(0);
            let name = cost.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
            let image = cost.get("image").and_then(serde_json::Value::as_str).unwrap_or("");
            let mut row = h_flex().justify_between().text_sm();
            let mut left = h_flex().items_center().gap_2();
            if !image.is_empty() {
                left = left.child(render_asset_image(image, 20., app));
            }
            left = left.child(name.to_string());
            let balance = balance_of(balances, &cost_id);
            let missing =
                balance.map(|balance| required.saturating_sub(i128::from(balance.max(0))));
            let missing_text = match missing {
                None => {
                    if self.farm.lands.detail_balances_loading {
                        "…".to_string()
                    } else {
                        crate::i18n::text(language, "余额待同步", "Balance pending").to_string()
                    }
                }
                Some(0) => crate::i18n::text(language, "余额充足", "Sufficient").to_string(),
                Some(amount) => {
                    tr(language, &format!("还差 {amount}"), &format!("Need {amount} more"))
                }
            };
            let missing_color = match missing {
                None => theme.muted_foreground,
                Some(0) => theme.success,
                Some(_) => theme.danger,
            };
            row = row.child(left).child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div().font_medium().child(
                            required
                                .try_into()
                                .map(format_amount)
                                .unwrap_or_else(|_| count_text.to_string()),
                        ),
                    )
                    .child(div().text_xs().text_color(missing_color).child(missing_text)),
            );
            conditions = conditions.child(row);
        }
        if info.get("unknown").and_then(serde_json::Value::as_bool) == Some(true) {
            conditions = conditions.child(div().text_xs().text_color(theme.danger).child(
                crate::i18n::text(language, "部分条件暂未获取", "Some requirements unavailable"),
            ));
        }
        content.child(conditions)
    }
}
