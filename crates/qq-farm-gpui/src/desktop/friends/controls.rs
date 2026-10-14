use super::super::*;
use gpui_kit::component::select::Select;

fn format_friend_range(start: usize, end: usize, total: usize, english: bool) -> String {
    if english {
        format!("{start}-{end} of {total} friends")
    } else {
        format!("第 {start}-{end} 名 / 共 {total} 名好友")
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn append_friend_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page == Page::Friends {
            let language = self.preferences.language.as_str();
            if let Some((message, level)) = &self.friends.feedback {
                let color = match level {
                    FriendFeedbackLevel::Success => cx.theme().success,
                    FriendFeedbackLevel::Info => cx.theme().muted_foreground,
                    FriendFeedbackLevel::Error => cx.theme().danger,
                };
                controls = controls.child(div().text_sm().text_color(color).child(message.clone()));
            }
            match self.page_section.as_str() {
                "friends" => {
                    controls = controls.child(
                        Button::new("refresh-friends")
                            .label(crate::i18n::text(language, "刷新列表", "Refresh List"))
                            .ghost()
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(id) = this.selected_account.as_deref() {
                                    if let Err(error) =
                                        qq_farm_app::friend::clear_friends_cache(&this.app, id)
                                    {
                                        this.error = Some(error.to_string());
                                    }
                                    this.refresh(cx);
                                }
                            })),
                    );
                }
                "records" => {
                    controls = controls.child(
                        Button::new("refresh-friend-visitors")
                            .label(crate::i18n::text(language, "刷新", "Refresh"))
                            .ghost()
                            .loading(self.friends.records_refresh_loading)
                            .disabled(self.friends.records_refresh_loading)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.refresh_friend_records(cx)),
                            ),
                    );
                }
                "interactionItems" => {
                    controls = controls.child(
                        Button::new("refresh-friend-interaction-items")
                            .label(crate::i18n::text(language, "刷新", "Refresh"))
                            .ghost()
                            .loading(self.friends.interaction_items_refresh_loading)
                            .disabled(self.friends.interaction_items_refresh_loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_friend_interaction_items(cx)
                            })),
                    );
                }
                _ => {}
            }
            if self.page_section == "friends" {
                let pet_sync = friend_pet_sync_progress(&self.page_raw);
                if pet_sync.1 > 0 && pet_sync.0 < pet_sync.1 {
                    controls = controls.child(
                        div()
                            .items_center()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                crate::i18n::text(
                                    language,
                                    &format!(
                                        "宠物状态 {}/{} 已确认，其余由每日同步自动补齐",
                                        pet_sync.0, pet_sync.1
                                    ),
                                    &format!(
                                    "Pet status {}/{} confirmed; the rest fills in via daily sync",
                                    pet_sync.0, pet_sync.1
                                ),
                                )
                                .to_string(),
                            ),
                    );
                }
                let query = self.search.read(cx).value();
                let visible_total = self
                    .rows
                    .iter()
                    .filter(|row| row.section == "friends")
                    .filter(|row| friend_matches_search(&row.record, &query))
                    .count();
                // The Vue template binds the filtered range but passes friends.length as total.
                let total_friends = self
                    .page_raw
                    .get("friends")
                    .and_then(serde_json::Value::as_array)
                    .map_or(0, Vec::len);
                let start = if visible_total == 0 { 0 } else { self.friends.page * 25 + 1 };
                let end = visible_total.min((self.friends.page + 1) * 25);
                let count_label = format_friend_range(
                    start,
                    end,
                    total_friends,
                    self.preferences.language == "en-US",
                );
                let range = div()
                    .id("friends-range")
                    .aria_label(count_label.clone())
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(count_label);
                #[cfg(test)]
                let range = range.test_support();
                controls = controls.child(range);
                let pages = visible_total.div_ceil(25).max(1);
                if pages > 1 {
                    controls = controls.child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("friends-page-previous")
                                    .label(crate::i18n::text(language, "上一页", "Previous"))
                                    .ghost()
                                    .disabled(self.friends.page == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.friends.page = this.friends.page.saturating_sub(1);
                                        cx.notify();
                                    })),
                            )
                            .children(page_items(self.friends.page, pages, 5).into_iter().map(
                                |item| {
                                    match item {
                                        PageItem::Page(page) => {
                                            let label = (page + 1).to_string();
                                            Button::new(("friends-page-number", page))
                                                .label(label.clone())
                                                .accessibility_label(crate::i18n::text(
                                                    language,
                                                    &format!("第 {label} 页"),
                                                    &format!("Page {label}"),
                                                ))
                                                .when(self.friends.page == page, |button| {
                                                    button.primary()
                                                })
                                                .when(self.friends.page != page, |button| {
                                                    button.ghost()
                                                })
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.friends.page = page;
                                                    cx.notify();
                                                }))
                                        }
                                        PageItem::JumpTo(page) => {
                                            Button::new(("friends-page-jump", page))
                                                .label("…")
                                                .accessibility_label(crate::i18n::text(
                                                    language,
                                                    &format!("跳转到第 {} 页", page + 1),
                                                    &format!("Jump to page {}", page + 1),
                                                ))
                                                .ghost()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.friends.page = page.min(pages - 1);
                                                    cx.notify();
                                                }))
                                        }
                                    }
                                },
                            ))
                            .child(
                                Button::new("friends-page-next")
                                    .label(crate::i18n::text(language, "下一页", "Next"))
                                    .ghost()
                                    .disabled(self.friends.page + 1 >= pages)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.friends.page = (this.friends.page + 1).min(pages - 1);
                                        cx.notify();
                                    })),
                            ),
                    );
                }
            }
            if self.page_section == "records" {
                for (filter, label) in [
                    (None, crate::i18n::text(language, "全部", "All")),
                    (Some(1), crate::i18n::text(language, "偷菜", "Steal")),
                    (Some(2), crate::i18n::text(language, "帮忙", "Help")),
                    (Some(3), crate::i18n::text(language, "捣乱", "Trouble")),
                ] {
                    controls = controls.child(
                        Button::new(format!("visitor-filter-{}", filter.unwrap_or(0)))
                            .label(label)
                            .when(self.friends.record_filter == filter, |button| button.primary())
                            .when(self.friends.record_filter != filter, |button| button.ghost())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.friends.record_filter = filter;
                                cx.notify();
                            })),
                    );
                }
            }
            if self.page_section == "blacklist" {
                controls =
                    controls.child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                        crate::i18n::text(
                            language,
                            "加入黑名单的好友在自动偷菜和帮助时会被跳过。",
                            "Blacklisted friends are skipped during auto steal and help.",
                        ),
                    ));
            }
            if self.page_section == "friends" {
                let search_keyword = self.search.read(cx).value();
                let has_stealable =
                    !stealable_friends_matching(&self.page_raw, &search_keyword).is_empty();
                controls = controls.child(
                    Button::new("steal-all-friends")
                        .label(crate::i18n::text(language, "全部偷取", "Steal all"))
                        .primary()
                        .outline()
                        .loading(self.friends.steal_all_loading)
                        .disabled(self.friends.steal_all_loading || !has_stealable)
                        .on_click(cx.listener(|this, _, _, cx| this.steal_all_friends(cx))),
                );
            }
            if self.page_section == "interactionItems" {
                if self.rows.iter().any(|row| row.section == "interactionItems") {
                    controls = controls
                        .child(
                            div().w(px(180.)).flex_none().child(
                                Select::new(&self.friends.interaction_picker)
                                    .id("friend-interaction-picker")
                                    .w_full()
                                    .accessibility_label(crate::i18n::text(
                                        language,
                                        "选择好友互动道具",
                                        "Select an item to use on a friend",
                                    ))
                                    .disabled(self.friends.interaction_using),
                            ),
                        )
                        .child(
                            Input::new(&self.friends.interaction_gid)
                                .id("friend-interaction-gid")
                                .w(px(160.)),
                        )
                        .child(
                            Input::new(&self.friends.interaction_land_ids)
                                .id("friend-interaction-land-ids")
                                .w(px(260.)),
                        )
                        .child(
                            Button::new("friend-interaction-use")
                                .label(crate::i18n::text(language, "使用", "Use"))
                                .primary()
                                .loading(self.friends.interaction_using)
                                .disabled(
                                    self.friends.interaction_using
                                        || self.friends.selected_interaction_item_id.is_none(),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.run_friend_interaction_item(cx)
                                })),
                        );
                } else if !self.loading && self.page_raw.get("interactionItemsError").is_none() {
                    let message = self
                        .page_raw
                        .pointer("/interactionItems/message")
                        .and_then(serde_json::Value::as_str)
                        .filter(|message| !message.trim().is_empty())
                        .unwrap_or(if language == "en-US" {
                            "No usable items in the bag"
                        } else {
                            "背包中暂无可用道具"
                        });
                    controls = controls.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(message.to_string()),
                    );
                }
            }
        }
        controls
    }
}

#[cfg(test)]
mod tests {
    use super::format_friend_range;

    #[test]
    fn friend_range_uses_vue_copy_and_all_friend_count() {
        assert_eq!(format_friend_range(0, 0, 3, false), "第 0-0 名 / 共 3 名好友");
        assert_eq!(format_friend_range(1, 25, 28, true), "1-25 of 28 friends");
    }
}
