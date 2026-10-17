//! 图鉴收藏奖励：详情弹窗 + 手动领取（bot 0533264）。

use super::super::super::*;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::WindowExt;

/// 档位状态文案（对齐 bot IllustratedPanel：已领取 / 可领取 / 未达成）。
fn level_state_label(level: &serde_json::Value, language: &str) -> &'static str {
    if level.get("claimed").and_then(serde_json::Value::as_bool) == Some(true) {
        crate::i18n::text(language, "已领取", "Claimed")
    } else if level.get("canClaim").and_then(serde_json::Value::as_bool) == Some(true) {
        crate::i18n::text(language, "可领取", "Claimable")
    } else {
        crate::i18n::text(language, "未达成", "Not reached")
    }
}

fn reward_text(reward: &serde_json::Value) -> String {
    let name = reward.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
    let count = reward.get("count").and_then(serde_json::Value::as_i64).unwrap_or(0);
    format!("{name} ×{count}")
}

fn buff_text(buff: &serde_json::Value) -> String {
    let name = buff.get("name").and_then(serde_json::Value::as_str).unwrap_or("");
    let value = buff.get("value").and_then(serde_json::Value::as_i64).unwrap_or(0);
    if buff.get("valueType").and_then(serde_json::Value::as_str) == Some("probability") {
        format!("{name} +{}%", value / 10)
    } else {
        format!("{name} +{value}")
    }
}

impl FarmDesktop {
    /// 详情弹窗（作物：收藏奖励档位；变异：当前加成 + 全量加成）。
    pub(in crate::desktop) fn open_illustrated_rewards_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = self.preferences.language.clone();
        let mutant = self.farm.illustrated.book == "mutant";
        let Some(book) = self
            .page_raw
            .get("illustrated")
            .and_then(|snapshot| snapshot.get(&self.farm.illustrated.book))
            .filter(|book| !book.is_null())
        else {
            return;
        };
        let book = book.clone();
        let claimable = book.get("claimable").and_then(serde_json::Value::as_bool) == Some(true);
        let title = if mutant {
            crate::i18n::text(&language, "超变图鉴属性加成", "Mutant bonuses")
        } else {
            crate::i18n::text(&language, "图鉴收藏奖励", "Collection rewards")
        };
        let claim_label = crate::i18n::text(&language, "领取奖励", "Claim rewards");
        let cancel_label = crate::i18n::text(&language, "取消", "Cancel");
        let muted_foreground = cx.theme().muted_foreground;
        let desktop = cx.entity();
        window.open_dialog(cx, move |dialog, _, app| {
            let pending = desktop.read(app).farm.illustrated.claim_pending;
            let claim_entity = desktop.clone();
            let mut content = div().v_flex().gap_2();
            if mutant {
                let (current_title, all_title) = (
                    crate::i18n::text(&language, "当前加成", "Current bonuses"),
                    crate::i18n::text(&language, "全部加成", "All bonuses"),
                );
                content = content.child(div().text_sm().font_medium().child(current_title));
                let current = book
                    .get("currentBuffs")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if current.is_empty() {
                    content = content.child(
                        div().text_sm().text_color(muted_foreground).child(crate::i18n::text(
                            &language,
                            "暂无属性加成",
                            "No bonuses yet",
                        )),
                    );
                } else {
                    content = content.child(
                        div()
                            .text_sm()
                            .child(current.iter().map(buff_text).collect::<Vec<_>>().join("、")),
                    );
                }
                content = content.child(div().text_sm().font_medium().child(all_title));
                let level = book.get("level").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let rows = book
                    .get("buffs")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|buff| {
                        let reached =
                            buff.get("level").and_then(serde_json::Value::as_i64).unwrap_or(0)
                                <= level;
                        let state = if reached {
                            crate::i18n::text(&language, "已达成", "Reached")
                        } else {
                            crate::i18n::text(&language, "未达成", "Not reached")
                        };
                        h_flex()
                            .justify_between()
                            .text_sm()
                            .child(buff_text(&buff))
                            .child(state)
                            .into_any_element()
                    })
                    .collect::<Vec<_>>();
                content = content.child(
                    div().v_flex().gap_1().max_h(px(420.)).overflow_y_scrollbar().children(rows),
                );
            } else {
                let progress =
                    book.get("progress").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let target =
                    book.get("nextLevelProgress").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let level = book.get("level").and_then(serde_json::Value::as_i64).unwrap_or(0);
                content = content
                    .child(div().text_sm().child(format!("Lv.{level} · {progress} / {target}")));
                let rows = book
                    .get("levels")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|level_entry| {
                        let level_no = level_entry
                            .get("level")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0);
                        let progress = level_entry
                            .get("progress")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0);
                        let rewards = level_entry
                            .get("rewards")
                            .and_then(serde_json::Value::as_array)
                            .map(|rewards| {
                                rewards.iter().map(reward_text).collect::<Vec<_>>().join("、")
                            })
                            .unwrap_or_default();
                        let state = level_state_label(&level_entry, &language);
                        let row_text = if rewards.is_empty() {
                            format!("{level_no} 级 · {progress} {state}")
                        } else {
                            format!("{level_no} 级 · {progress} {rewards} {state}")
                        };
                        let row = h_flex()
                            .justify_between()
                            .gap_3()
                            .text_sm()
                            .child(format!("{level_no} 级 · {progress}"))
                            .child(rewards)
                            .child(state);
                        #[cfg(test)]
                        let row = row
                            .id(format!("illustrated-level-{level_no}"))
                            .aria_label(row_text)
                            .test_support();
                        #[cfg(not(test))]
                        let _ = row_text;
                        row.into_any_element()
                    })
                    .collect::<Vec<_>>();
                content = content.child(
                    div().v_flex().gap_1().max_h(px(420.)).overflow_y_scrollbar().children(rows),
                );
            }
            let show_claim = !mutant || claimable;
            let footer = h_flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("illustrated-details-close")
                        .label(cancel_label)
                        .ghost()
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
                .when(show_claim, |footer| {
                    footer.child(
                        Button::new("illustrated-claim-confirm")
                            .label(claim_label)
                            .primary()
                            .loading(pending)
                            .disabled(pending || !claimable)
                            .on_click(move |_, _, cx| {
                                claim_entity
                                    .update(cx, |this, cx| this.start_illustrated_claim(cx));
                            }),
                    )
                });
            dialog.title(title).w(px(560.)).child(content).footer(footer)
        });
        cx.notify();
    }

    /// 领取奖励：成功用回包快照原地更新，刷新失败时提示手动刷新（不吞成功）。
    pub(in crate::desktop) fn start_illustrated_claim(&mut self, cx: &mut Context<Self>) {
        if self.farm.illustrated.claim_pending {
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
        let kind: i32 = if self.farm.illustrated.book == "mutant" { 2 } else { 1 };
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let account_id_check = account_id.clone();
        self.farm.illustrated.claim_generation =
            self.farm.illustrated.claim_generation.wrapping_add(1);
        let generation = self.farm.illustrated.claim_generation;
        self.farm.illustrated.claim_pending = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    qq_farm_app::friend::illustrated_claim(&app, &account_id, kind)
                        .await
                        .map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.farm.illustrated.claim_generation != generation {
                    return;
                }
                this.farm.illustrated.claim_pending = false;
                if this.selected_account.as_deref() != Some(account_id_check.as_str()) {
                    cx.notify();
                    return;
                }
                let language = this.preferences.language.clone();
                match result {
                    Ok(Ok(value)) => {
                        let rewards = value
                            .get("rewards")
                            .and_then(serde_json::Value::as_array)
                            .map(|rewards| {
                                rewards.iter().map(reward_text).collect::<Vec<_>>().join("、")
                            })
                            .filter(|text| !text.is_empty())
                            .unwrap_or_else(|| {
                                crate::i18n::text(&language, "领取成功", "Claimed").to_string()
                            });
                        let refresh_required =
                            value.get("refreshRequired").and_then(serde_json::Value::as_bool)
                                == Some(true);
                        let snapshot =
                            value.get("snapshot").filter(|snapshot| !snapshot.is_null()).cloned();
                        match snapshot {
                            Some(snapshot) => {
                                if let Some(page) = this.page_raw.as_object_mut() {
                                    page.insert("illustrated".into(), snapshot);
                                }
                            }
                            None => this.refresh(cx),
                        }
                        let message = if refresh_required {
                            format!(
                                "{rewards}。{}",
                                crate::i18n::text(
                                    &language,
                                    "请刷新确认最新进度",
                                    "Refresh to confirm the latest progress"
                                )
                            )
                        } else {
                            rewards
                        };
                        this.show_illustrated_notice(message, !refresh_required, cx);
                    }
                    Ok(Err(error)) => {
                        this.show_illustrated_notice(error, false, cx);
                    }
                    Err(error) => {
                        this.show_illustrated_notice(
                            crate::i18n::background_task_error(&language, error),
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
}
