use super::super::*;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageItem {
    Page(usize),
    JumpTo(usize),
}

/// Build the same numbered-page window used by Naive UI's desktop pagination.
pub(crate) fn page_items(
    current_page: usize,
    page_count: usize,
    page_slot: usize,
) -> Vec<PageItem> {
    let page_count = page_count.max(1);
    let page_slot = page_slot.max(5);
    let current_page = current_page.min(page_count - 1) + 1;
    if page_count <= page_slot {
        return (0..page_count).map(PageItem::Page).collect();
    }

    let last_page = page_count;
    let middle_delta = (page_slot - 5) / 2;
    let middle_end = (current_page + middle_delta).max(1 + page_slot - 3).min(last_page - 2);
    let middle_start =
        (current_page.saturating_sub(middle_delta)).min(last_page - page_slot + 3).max(3);
    let left_split = middle_start > 3;
    let right_split = middle_end < last_page - 2;

    let mut items = vec![PageItem::Page(0)];
    if left_split {
        items.push(PageItem::JumpTo(middle_start - 2));
    } else {
        items.push(PageItem::Page(1));
    }
    items.extend((middle_start..=middle_end).map(|page| PageItem::Page(page - 1)));
    if right_split {
        items.push(PageItem::JumpTo(middle_end));
    } else if middle_end == last_page - 2 {
        items.push(PageItem::Page(last_page - 2));
    }
    if items.last() != Some(&PageItem::Page(last_page - 1)) {
        items.push(PageItem::Page(last_page - 1));
    }
    items
}

/// Which list a shared pagination footer drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TablePagerTarget {
    Accounts,
    GameConfig,
}

/// Values rendered by the shared table pagination footer.
pub(crate) struct TablePager<'a> {
    pub target: TablePagerTarget,
    pub page: usize,
    pub page_count: usize,
    pub page_size: usize,
    pub page_size_options: &'a [usize],
    pub total: usize,
    pub start_index: usize,
    pub end_index: usize,
    pub selected: usize,
    pub page_size_disabled: bool,
}

impl FarmDesktop {
    /// Render the footer shared by the accounts and game-config tables so both
    /// catalog pages keep one pagination geometry and behavior.
    pub(in crate::desktop) fn render_table_pagination(
        &self,
        pager: TablePager<'_>,
        cx: &mut Context<Self>,
    ) -> Div {
        let english = self.preferences.language == "en-US";
        let target = pager.target;
        let items = page_items(pager.page, pager.page_count, 9);
        let selected_label =
            if english { pager.page_size.to_string() } else { format!("{} 条", pager.page_size) };
        let size_selector_label = if english {
            format!("Rows per page: {}", pager.page_size)
        } else {
            format!("每页 {} 条", pager.page_size)
        };
        let range_label = if english {
            format!(
                "{}–{} of {} · {} selected",
                if pager.total == 0 { 0 } else { pager.start_index },
                pager.end_index,
                pager.total,
                pager.selected
            )
        } else {
            format!(
                "{}–{} 条，共 {} 条 · 已选 {} 条",
                if pager.total == 0 { 0 } else { pager.start_index },
                pager.end_index,
                pager.total,
                pager.selected
            )
        };
        let (size_selector_id, previous_id, next_id, indicator_id) = match target {
            TablePagerTarget::Accounts => (
                "account-page-size-selector",
                "account-page-prev",
                "account-page-next",
                "account-page-indicator",
            ),
            TablePagerTarget::GameConfig => (
                "game-config-page-size-selector",
                "game-config-page-previous",
                "game-config-page-next",
                "game-config-page-indicator",
            ),
        };
        let number_id = move |page: usize| match target {
            TablePagerTarget::Accounts => ("account-page-number", page),
            TablePagerTarget::GameConfig => ("game-config-page-number", page),
        };
        let jump_id = move |page: usize| match target {
            TablePagerTarget::Accounts => ("account-page-jump", page),
            TablePagerTarget::GameConfig => ("game-config-page-jump", page),
        };
        let page_size_options = pager.page_size_options.to_vec();
        let desktop = cx.entity().downgrade();
        h_flex()
            .items_center()
            .justify_between()
            .mt_auto()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(range_label))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div().text_sm().text_color(cx.theme().muted_foreground).child(if english {
                            "Rows"
                        } else {
                            "每页"
                        }),
                    )
                    .child(
                        Button::new(size_selector_id)
                            .label(selected_label)
                            .dropdown_caret(true)
                            .accessibility_label(size_selector_label)
                            .disabled(pager.page_size_disabled)
                            .dropdown_menu_with_anchor(
                                gpui_kit::Anchor::BottomRight,
                                move |mut menu, _, _| {
                                    for page_size in &page_size_options {
                                        let desktop = desktop.clone();
                                        let page_size = *page_size;
                                        let label = if english {
                                            format!("{page_size} rows")
                                        } else {
                                            format!("{page_size} 条")
                                        };
                                        menu = menu.item(
                                            PopupMenuItem::new(label)
                                                .checked(page_size == pager.page_size)
                                                .on_click(move |_, _, cx| {
                                                    let _ = desktop.update(cx, |this, cx| {
                                                        match target {
                                                            TablePagerTarget::Accounts => {
                                                                this.accounts.page_size = page_size;
                                                                this.accounts.page = 0;
                                                            }
                                                            TablePagerTarget::GameConfig => {
                                                                this.game_config.page_size =
                                                                    page_size;
                                                                this.game_config.page = 0;
                                                            }
                                                        }
                                                        cx.notify();
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                },
                            ),
                    )
                    .child(
                        Button::new(previous_id)
                            .label(if english { "Previous" } else { "上一页" })
                            .ghost()
                            .disabled(pager.page == 0)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                match target {
                                    TablePagerTarget::Accounts => {
                                        this.accounts.page = this.accounts.page.saturating_sub(1);
                                    }
                                    TablePagerTarget::GameConfig => {
                                        this.game_config.page =
                                            this.game_config.page.saturating_sub(1);
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .children(items.into_iter().map(|item| match item {
                        PageItem::Page(target_page) => {
                            let label = (target_page + 1).to_string();
                            Button::new(number_id(target_page))
                                .label(label.clone())
                                .accessibility_label(if english {
                                    format!("Page {label}")
                                } else {
                                    format!("第 {label} 页")
                                })
                                .when(pager.page == target_page, |button| button.primary())
                                .when(pager.page != target_page, |button| button.ghost())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    match target {
                                        TablePagerTarget::Accounts => {
                                            this.accounts.page = target_page;
                                        }
                                        TablePagerTarget::GameConfig => {
                                            this.game_config.page = target_page;
                                        }
                                    }
                                    cx.notify();
                                }))
                        }
                        PageItem::JumpTo(target_page) => {
                            let label = if english {
                                format!("Jump to page {}", target_page + 1)
                            } else {
                                format!("跳转到第 {} 页", target_page + 1)
                            };
                            Button::new(jump_id(target_page))
                                .label("…")
                                .accessibility_label(label)
                                .ghost()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    match target {
                                        TablePagerTarget::Accounts => {
                                            this.accounts.page =
                                                target_page.min(pager.page_count.saturating_sub(1));
                                        }
                                        TablePagerTarget::GameConfig => {
                                            this.game_config.page =
                                                target_page.min(pager.page_count.saturating_sub(1));
                                        }
                                    }
                                    cx.notify();
                                }))
                        }
                    }))
                    .child({
                        let label = if english {
                            format!("Page {} of {}", pager.page + 1, pager.page_count)
                        } else {
                            format!("第 {} / {} 页", pager.page + 1, pager.page_count)
                        };
                        let indicator =
                            div().id(indicator_id).aria_label(label.clone()).text_sm().child(label);
                        #[cfg(test)]
                        let indicator = indicator.test_support();
                        indicator
                    })
                    .child(
                        Button::new(next_id)
                            .label(if english { "Next" } else { "下一页" })
                            .ghost()
                            .disabled(pager.page + 1 >= pager.page_count)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                match target {
                                    TablePagerTarget::Accounts => {
                                        this.accounts.page =
                                            (this.accounts.page + 1).min(pager.page_count - 1);
                                    }
                                    TablePagerTarget::GameConfig => {
                                        this.game_config.page =
                                            (this.game_config.page + 1).min(pager.page_count - 1);
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{page_items, PageItem};

    #[test]
    fn page_slots_match_naive_ui_first_middle_and_last_windows() {
        assert_eq!(page_items(0, 1, 9), vec![PageItem::Page(0)]);
        assert_eq!(page_items(0, 8, 9), (0..8).map(PageItem::Page).collect::<Vec<_>>());
        assert_eq!(
            page_items(9, 20, 9),
            vec![
                PageItem::Page(0),
                PageItem::JumpTo(6),
                PageItem::Page(7),
                PageItem::Page(8),
                PageItem::Page(9),
                PageItem::Page(10),
                PageItem::Page(11),
                PageItem::JumpTo(12),
                PageItem::Page(19),
            ]
        );
        assert_eq!(
            page_items(19, 20, 9),
            vec![
                PageItem::Page(0),
                PageItem::JumpTo(12),
                PageItem::Page(13),
                PageItem::Page(14),
                PageItem::Page(15),
                PageItem::Page(16),
                PageItem::Page(17),
                PageItem::Page(18),
                PageItem::Page(19),
            ]
        );
    }

    #[test]
    fn five_page_slot_keeps_endpoints_and_jump_targets() {
        assert_eq!(
            page_items(9, 20, 5),
            vec![
                PageItem::Page(0),
                PageItem::JumpTo(8),
                PageItem::Page(9),
                PageItem::JumpTo(10),
                PageItem::Page(19),
            ]
        );
    }
}
