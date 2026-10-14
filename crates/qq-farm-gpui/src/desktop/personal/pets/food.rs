use super::*;
use gpui_kit::component::input::NumberInput;

impl FarmDesktop {
    pub(in crate::desktop) fn sync_personal_pet_food_inputs(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) {
        let language = &self.preferences.language;
        let food_ids = self
            .page_raw
            .pointer("/pets/foods")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|food| food.get("id").and_then(serde_json::Value::as_i64))
            .collect::<HashSet<_>>();
        self.farm.pets.food_inputs.retain(|id, _| food_ids.contains(id));
        for id in food_ids {
            self.farm.pets.food_inputs.entry(id).or_insert_with(|| {
                cx.new(|cx| {
                    let mut state = InputState::new(window, cx)
                        .placeholder(crate::i18n::text(language, "数量", "Quantity"));
                    state.set_value("1", window, cx);
                    state.set_min(Some(1.), window, cx);
                    state.set_max(Some(99.), window, cx);
                    state
                })
            });
        }
    }

    pub(super) fn render_personal_pet_food(&self, cx: &mut Context<Self>) -> Div {
        let language = &self.preferences.language;
        let foods = self.page_raw.pointer("/pets/foods").and_then(serde_json::Value::as_array);
        let empty = foods.is_none_or(Vec::is_empty);
        div()
            .v_flex()
            .gap_3()
            .pt_4()
            .child(div().font_semibold().child(crate::i18n::text(language, "狗粮", "Pet food")))
            .when(empty, |panel| {
                panel.child(
                    div()
                        .id("personal-pet-food-empty")
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::i18n::text(language, "暂无狗粮", "No dog food"))
                        .test_support(),
                )
            })
            .children(foods.into_iter().flatten().map(|food| {
                let id = food.get("id").and_then(serde_json::Value::as_i64).unwrap_or_default();
                let name = food
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(crate::i18n::text(language, "狗粮", "Pet food"));
                let stock =
                    food.get("count").and_then(serde_json::Value::as_i64).unwrap_or_default();
                let duration =
                    food.get("duration").and_then(serde_json::Value::as_i64).unwrap_or_default();
                let input = self.farm.pets.food_inputs.get(&id);
                h_flex()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .child(div().w(rems(7.5)).truncate().font_medium().child(name.to_string()))
                    .child(super::view::pet_tag(
                        format!("{}: {stock}", crate::i18n::text(language, "库存", "Stock")),
                        None,
                        cx,
                    ))
                    .child(super::view::pet_tag(
                        format!("+{}{}", duration / 86_400, crate::i18n::text(language, "天", "d")),
                        Some(cx.theme().info),
                        cx,
                    ))
                    .when_some(input, |row, input| {
                        row.child(
                            div()
                                .id(format!("pet-food-count-{id}"))
                                .w(rems(8.))
                                .child(NumberInput::new(input)),
                        )
                    })
                    .child(
                        Button::new(format!("pet-food-use-{id}"))
                            .label(crate::i18n::text(language, "喂食", "Feed"))
                            .disabled(stock <= 0)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let count = this
                                    .farm
                                    .pets
                                    .food_inputs
                                    .get(&id)
                                    .and_then(|state| state.read(cx).value().parse::<i64>().ok())
                                    .unwrap_or(1)
                                    .clamp(1, 99);
                                this.run_personal_pet_action(PetAction::Food(id, count), cx);
                            })),
                    )
            }))
    }
}
