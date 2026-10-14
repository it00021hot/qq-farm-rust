use super::super::*;
use super::{PRICE_OPTIONS, RARITY_OPTIONS};
use gpui_kit::component::select::Select;

impl FarmDesktop {
    pub(super) fn append_fruit_fields(&self, mut form: Div, cx: &mut Context<Self>) -> Div {
        form = form
            .when(!self.game_config.record.is_null(), |form| {
                form.child(self.render_config_input(
                    "果实ID",
                    &self.game_config.id,
                    "config-id",
                    cx,
                ))
            })
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(128.)).child(crate::i18n::text(
                        &self.preferences.language,
                        "植物",
                        "Plant",
                    )))
                    .child(
                        Select::new(&self.game_config.plant_form_selector)
                            .id("config-plant")
                            .w(px(280.))
                            .placeholder(crate::i18n::text(
                                &self.preferences.language,
                                "请选择植物",
                                "Select a plant",
                            ))
                            .accessibility_label(crate::i18n::text(
                                &self.preferences.language,
                                "植物",
                                "Plant",
                            ))
                            .disabled(!self.game_config.record.is_null()),
                    ),
            )
            .child(self.render_config_input("名称", &self.game_config.name, "config-name", cx))
            .child(self.render_extra_input("价格", "price", "config-price", cx))
            .child(self.render_choice_field(
                "货币",
                "priceId",
                "config-price-id",
                PRICE_OPTIONS,
                cx,
            ))
            .child(self.render_choice_field(
                "稀有度",
                "rarity",
                "config-rarity",
                RARITY_OPTIONS,
                cx,
            ))
            .child(self.render_extra_input("堆叠上限", "maxCount", "config-max-count", cx))
            .child(self.render_extra_input("收获数", "fruitCount", "config-fruit-count", cx))
            .child(self.render_config_input("资源名", &self.game_config.asset, "config-asset", cx))
            .child(self.render_config_textarea("描述", &self.game_config.desc, "config-desc"))
            .child(self.render_extra_input("效果说明", "effectDesc", "config-effect-desc", cx));

        form
    }
}
