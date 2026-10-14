use super::super::*;
use gpui_kit::component::{sheet::Sheet, WindowExt};

const PRICE_OPTIONS: &[(&str, &str, &str)] =
    &[("0", "金币", "Coins"), ("1005", "金豆豆", "Gold beans"), ("1004", "钻石", "Diamonds")];
const RARITY_OPTIONS: &[(&str, &str, &str)] = &[
    ("0", "普通", "Common"),
    ("1", "优秀", "Uncommon"),
    ("2", "精良", "Fine"),
    ("3", "稀有", "Rare"),
    ("4", "史诗", "Epic"),
    ("5", "传说", "Legendary"),
];
const SEASON_OPTIONS: &[(&str, &str, &str)] =
    &[("1", "单季", "Single season"), ("2", "双季", "Two seasons")];
const SIZE_OPTIONS: &[(&str, &str, &str)] = &[
    ("0", "1×1（普通作物）", "1×1 standard crop"),
    ("2", "2×2（占地4格）", "2×2 uses 4 plots"),
    ("3", "3×3（占地9格）", "3×3 uses 9 plots"),
];
const CAN_USE_OPTIONS: &[(&str, &str, &str)] = &[("0", "否", "No"), ("1", "是", "Yes")];
const GROW_PHASE_TEMPLATES: &[(&str, &str, &str)] = &[
    ("种子:2400;发芽:2400;小叶子:2400;大叶子:2400;开花:2400;成熟:0;", "4 小时", "4 hours"),
    ("种子:4800;发芽:4800;小叶子:4800;大叶子:4800;开花:4800;成熟:0;", "8 小时", "8 hours"),
    ("种子:7200;发芽:7200;小叶子:7200;大叶子:7200;开花:7200;成熟:0;", "12 小时", "12 hours"),
    ("种子:14400;发芽:14400;小叶子:14400;大叶子:14400;开花:14400;成熟:0;", "24 小时", "24 hours"),
];

mod common;
mod fruit;
mod item;
mod seed;

impl FarmDesktop {
    pub(super) fn render_game_config_form(&self, sheet: Sheet, cx: &mut Context<Self>) -> Sheet {
        let mut form = div().v_flex().gap_3().px_2().py_2();

        form = match self.game_config.kind.as_str() {
            "seed" => self.append_seed_fields(form, cx),
            "fruit" => self.append_fruit_fields(form, cx),
            _ => self.append_item_fields(form, cx),
        };

        if self.game_config.kind != "seed" {
            form = form.child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(128.)).child(crate::i18n::text(
                        &self.preferences.language,
                        "上传图片",
                        "Image",
                    )))
                    .child(
                        Input::new(&self.game_config.image_path)
                            .id("config-image-path")
                            .w(px(220.)),
                    )
                    .child(
                        Button::new("config-choose-image")
                            .label(crate::i18n::text(
                                &self.preferences.language,
                                "选择图片…",
                                "Choose image…",
                            ))
                            .ghost()
                            .on_click(
                                cx.listener(|this, _, _, cx| this.choose_game_config_image(cx)),
                            ),
                    ),
            );
        }
        if let Some(error) = self.game_config.form_error.as_ref() {
            form = form.child(div().text_color(cx.theme().danger).child(error.clone()));
        }

        let footer = h_flex()
            .justify_end()
            .gap_2()
            .w_full()
            .child(
                Button::new("config-form-cancel")
                    .label(crate::i18n::text(&self.preferences.language, "取消", "Cancel"))
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.game_config.form_open = false;
                        window.close_sheet(cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("config-form-save")
                    .label(crate::i18n::text(&self.preferences.language, "确认", "Confirm"))
                    .primary()
                    .disabled(self.loading)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.save_game_config_form(window, cx)),
                    ),
            );

        sheet
            .size(rems(34.))
            .resizable(false)
            .overlay_closable(true)
            .title(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_lg().font_semibold().child(self.config_form_title()))
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                        crate::i18n::text(
                            &self.preferences.language,
                            "配置会写入本机覆盖数据，并立即用于后续业务读取。",
                            "The override is saved locally and used by subsequent operations.",
                        ),
                    )),
            )
            .child(form)
            .footer(footer)
            .on_close(cx.listener(|this, _, _, cx| {
                this.game_config.form_open = false;
                cx.notify();
            }))
    }
}
