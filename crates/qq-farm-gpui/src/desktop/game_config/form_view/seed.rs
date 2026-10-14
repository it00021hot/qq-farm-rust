use super::super::*;
use super::{GROW_PHASE_TEMPLATES, PRICE_OPTIONS, SEASON_OPTIONS, SIZE_OPTIONS};

impl FarmDesktop {
    pub(super) fn append_seed_fields(&self, mut form: Div, cx: &mut Context<Self>) -> Div {
        form = form
            .child(self.render_config_input("种子ID", &self.game_config.id, "config-id", cx))
            .child(self.render_config_input("名称", &self.game_config.name, "config-name", cx))
            .child(self.render_choice_field(
                "阶段模板",
                "growPhases",
                "config-grow-template",
                GROW_PHASE_TEMPLATES,
                cx,
            ))
            .child(self.render_extra_input("生长阶段", "growPhases", "config-grow-phases", cx))
            .child(self.render_config_input("等级", &self.game_config.level, "config-level", cx))
            .child(self.render_choice_field(
                "季节",
                "seasons",
                "config-seasons",
                SEASON_OPTIONS,
                cx,
            ))
            .child(self.render_extra_input("收获数", "fruitCount", "config-fruit-count", cx))
            .child(self.render_extra_input("价格", "price", "config-price", cx))
            .child(self.render_choice_field(
                "货币",
                "priceId",
                "config-price-id",
                PRICE_OPTIONS,
                cx,
            ))
            .child(self.render_extra_input("经验", "exp", "config-exp", cx))
            .child(self.render_choice_field("占地", "size", "config-size", SIZE_OPTIONS, cx));

        form
    }
}
