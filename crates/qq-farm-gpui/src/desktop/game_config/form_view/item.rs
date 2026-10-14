use super::super::*;
use super::{CAN_USE_OPTIONS, PRICE_OPTIONS, RARITY_OPTIONS};
use gpui_kit::component::select::Select;

impl FarmDesktop {
    pub(super) fn append_item_fields(&self, mut form: Div, cx: &mut Context<Self>) -> Div {
        form = form
            .child(self.render_config_input("物品ID", &self.game_config.id, "config-id", cx))
            .child(self.render_dynamic_item_type(cx))
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
            .child(self.render_choice_field(
                "可使用",
                "canUse",
                "config-can-use",
                CAN_USE_OPTIONS,
                cx,
            ))
            .child(self.render_config_input("等级", &self.game_config.level, "config-level", cx))
            .child(self.render_extra_input("堆叠上限", "maxCount", "config-max-count", cx))
            .child(self.render_extra_input(
                "交互类型",
                "interactionType",
                "config-interaction-type",
                cx,
            ))
            .child(self.render_config_input("资源名", &self.game_config.asset, "config-asset", cx))
            .child(self.render_config_textarea("描述", &self.game_config.desc, "config-desc"))
            .child(self.render_extra_input("效果描述", "effectDesc", "config-effect-desc", cx));

        form
    }

    pub(super) fn render_dynamic_item_type(&self, _cx: &mut Context<Self>) -> Div {
        h_flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(div().w(px(128.)).child(crate::i18n::text(
                &self.preferences.language,
                "类型",
                "Type",
            )))
            .child(
                Select::new(&self.game_config.item_type_form_selector)
                    .id("config-type")
                    .w(px(280.))
                    .placeholder(crate::i18n::text(
                        &self.preferences.language,
                        "请选择类型",
                        "Select a type",
                    ))
                    .accessibility_label(crate::i18n::text(
                        &self.preferences.language,
                        "类型",
                        "Type",
                    )),
            )
    }
}
