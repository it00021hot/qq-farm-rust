use super::helpers::{category_counts, category_label, selected_sellable_count, BAG_CATEGORIES};
use crate::desktop::*;

impl FarmDesktop {
    pub(in crate::desktop::personal) fn append_bag_controls(
        &self,
        mut controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let language = &self.preferences.language;
        let counts = category_counts(&self.page_raw);
        for ((category, _), count) in BAG_CATEGORIES.into_iter().zip(counts) {
            controls = controls.child(
                Button::new(format!("bag-category-{category}"))
                    .label(format!("{} ({count})", category_label(category, language)))
                    .when(self.farm.bag.category == category, |button| button.primary())
                    .when(self.farm.bag.category != category, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.farm.bag.category = category.into();
                        cx.notify();
                    })),
            );
        }
        if self.farm.bag.category == "all" || self.farm.bag.category == "fruit" {
            controls = controls.child(
                Button::new("bag-batch-toggle")
                    .label(if self.farm.bag.batch_mode {
                        crate::i18n::text(language, "取消", "Cancel")
                    } else {
                        crate::i18n::text(language, "批量出售", "Batch Sell")
                    })
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.farm.bag.batch_mode = !this.farm.bag.batch_mode;
                        if !this.farm.bag.batch_mode {
                            this.farm.bag.selected_ids.clear();
                        }
                        cx.notify();
                    })),
            );
        }
        let selected_count = selected_sellable_count(
            &self.page_raw,
            &self.farm.bag.category,
            &self.farm.bag.selected_ids,
        );
        if self.farm.bag.batch_mode {
            controls = controls
                .child(
                    Button::new("bag-select-all")
                        .label(crate::i18n::text(language, "全选", "Select All"))
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| this.select_all_bag_sellable(cx))),
                )
                .child(
                    Button::new("bag-sell-selected")
                        .label(
                            crate::i18n::text(
                                language,
                                &format!("批量出售 ({selected_count})"),
                                &format!("Batch Sell ({selected_count})"),
                            )
                            .to_string(),
                        )
                        .danger()
                        .loading(self.farm.bag.batch_selling)
                        .disabled(
                            selected_count == 0 || self.loading || self.farm.bag.batch_selling,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.run_bag_batch_sell(cx))),
                );
        }
        controls
    }
}
