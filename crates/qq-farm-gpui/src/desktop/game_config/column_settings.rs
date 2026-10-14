use super::super::*;
use super::columns::move_game_config_column;
use gpui_kit::base::v_flex;
use gpui_kit::component::popover::Popover;

impl FarmDesktop {
    pub(super) fn render_game_config_column_settings(&self, cx: &mut Context<Self>) -> Popover {
        let section = self.page_section.clone();
        let columns = self.game_config.columns.get(&section).cloned().unwrap_or_default();
        let all_checked = columns.iter().all(|column| column.checked);
        let desktop = cx.entity();
        let desktop_open = desktop.clone();
        let language = self.preferences.language.clone();
        let content_language = language.clone();
        let trigger_label = crate::i18n::text(&language, "列设置", "Columns");

        Popover::new("game-config-column-settings")
            // Match the account page and Vue's placement="bottom-end" picker.
            .anchor(gpui_kit::Anchor::TopRight)
            .trigger(
                Button::new("game-config-column-settings-trigger").label(trigger_label).ghost(),
            )
            .open(self.game_config.column_picker_open)
            .on_open_change(move |open, _, cx| {
                desktop_open.update(cx, |this, cx| {
                    this.game_config.column_picker_open = *open;
                    cx.notify();
                });
            })
            .content(move |_, _, _| {
                let desktop_all = desktop.clone();
                let select_all = crate::i18n::text(&content_language, "全选", "Select all");
                let section_for_all = section.clone();
                v_flex()
                    .w(px(340.))
                    .max_h(px(360.))
                    .gap_2()
                    .child(
                        Checkbox::new("game-config-columns-select-all")
                            .label(select_all)
                            .checked(all_checked)
                            .on_change(move |checked, _, cx| {
                                let section = section_for_all.clone();
                                desktop_all.update(cx, |this, cx| {
                                    if let Some(columns) =
                                        this.game_config.columns.get_mut(&section)
                                    {
                                        for column in columns {
                                            column.checked = *checked;
                                        }
                                    }
                                    cx.notify();
                                });
                            }),
                    )
                    .children(columns.iter().enumerate().map(|(index, column)| {
                        let desktop_visible = desktop.clone();
                        let desktop_pin = desktop.clone();
                        let desktop_up = desktop.clone();
                        let desktop_down = desktop.clone();
                        let section_visible = section.clone();
                        let section_pin = section.clone();
                        let section_up = section.clone();
                        let section_down = section.clone();
                        let key = column.key;
                        let title =
                            super::columns::game_config_column_title(key, &content_language);
                        let pin_label = column.pin.label(&content_language);
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Checkbox::new(format!("game-config-column-visible-{key}"))
                                    .label(title)
                                    .checked(column.checked)
                                    .on_change(move |checked, _, cx| {
                                        desktop_visible.update(cx, |this, cx| {
                                            if let Some(column) = this
                                                .game_config
                                                .columns
                                                .get_mut(&section_visible)
                                                .and_then(|columns| columns.get_mut(index))
                                            {
                                                column.checked = *checked;
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new(format!("game-config-column-pin-{key}"))
                                    .label(pin_label)
                                    .ghost()
                                    .accessibility_label(format!("{title}: {pin_label}"))
                                    .on_click(move |_, _, cx| {
                                        desktop_pin.update(cx, |this, cx| {
                                            if let Some(column) = this
                                                .game_config
                                                .columns
                                                .get_mut(&section_pin)
                                                .and_then(|columns| columns.get_mut(index))
                                            {
                                                column.pin = column.pin.next();
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new(format!("game-config-column-up-{key}"))
                                    .label("↑")
                                    .ghost()
                                    .accessibility_label(if content_language == "en-US" {
                                        format!("Move {title} up")
                                    } else {
                                        format!("将{title}上移")
                                    })
                                    .disabled(index == 0)
                                    .on_click(move |_, _, cx| {
                                        desktop_up.update(cx, |this, cx| {
                                            if let Some(columns) =
                                                this.game_config.columns.get_mut(&section_up)
                                            {
                                                move_game_config_column(columns, index, -1);
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new(format!("game-config-column-down-{key}"))
                                    .label("↓")
                                    .ghost()
                                    .accessibility_label(if content_language == "en-US" {
                                        format!("Move {title} down")
                                    } else {
                                        format!("将{title}下移")
                                    })
                                    .disabled(index + 1 == columns.len())
                                    .on_click(move |_, _, cx| {
                                        desktop_down.update(cx, |this, cx| {
                                            if let Some(columns) =
                                                this.game_config.columns.get_mut(&section_down)
                                            {
                                                move_game_config_column(columns, index, 1);
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                    }))
            })
    }
}
