use super::super::form;
use super::super::*;
use gpui_kit::component::input::NumberInput;

impl FarmDesktop {
    pub(super) fn config_form_title(&self) -> &'static str {
        match (self.game_config.record.is_null(), self.game_config.kind.as_str()) {
            (true, "seed") => crate::i18n::text(&self.preferences.language, "录入种子", "Add Seed"),
            (true, "fruit") => {
                crate::i18n::text(&self.preferences.language, "录入果实", "Add Fruit")
            }
            (true, _) => crate::i18n::text(&self.preferences.language, "录入道具", "Add Item"),
            (false, "seed") => {
                crate::i18n::text(&self.preferences.language, "编辑种子", "Edit Seed")
            }
            (false, "fruit") => {
                crate::i18n::text(&self.preferences.language, "编辑果实", "Edit Fruit")
            }
            (false, _) => crate::i18n::text(&self.preferences.language, "编辑道具", "Edit Item"),
        }
    }

    pub(super) fn render_config_input(
        &self,
        label: &str,
        state: &Entity<InputState>,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let disabled = id == "config-id" && !self.game_config.record.is_null();
        let input = if id == "config-id" || id == "config-level" {
            div()
                .id(id)
                .w(px(280.))
                .child(NumberInput::new(state).disabled(disabled).w_full())
                .into_any_element()
        } else {
            Input::new(state).id(id).w(px(280.)).into_any_element()
        };
        h_flex()
            .items_center()
            .gap_2()
            .child(div().w(px(128.)).child(
                super::super::labels::form_field(&self.preferences.language, label).to_string(),
            ))
            .child(input)
            .when(id == "config-id" && !self.game_config.record.is_null(), |row| {
                row.child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                    crate::i18n::text(
                        &self.preferences.language,
                        "编辑时编号保持不变",
                        "The ID cannot be changed while editing.",
                    ),
                ))
            })
    }

    pub(super) fn render_extra_input(
        &self,
        label: &str,
        key: &str,
        id: &'static str,
        _: &mut Context<Self>,
    ) -> impl IntoElement {
        let editor = if let Some(state) = self.game_config.extra_textareas.get(key) {
            Textarea::new(state).accessibility_id(id).h(px(82.)).w(px(280.)).into_any_element()
        } else if matches!(key, "price" | "fruitCount" | "exp" | "maxCount") {
            NumberInput::new(&self.game_config.extra_fields[key]).w(px(280.)).into_any_element()
        } else {
            Input::new(&self.game_config.extra_fields[key]).id(id).w(px(280.)).into_any_element()
        };
        h_flex()
            .id(format!("config-field-row-{id}"))
            .items_start()
            .gap_2()
            .child(div().w(px(128.)).child(
                super::super::labels::form_field(&self.preferences.language, label).to_string(),
            ))
            .child(editor)
            .test_support()
    }

    pub(super) fn render_config_textarea(
        &self,
        label: &str,
        state: &Entity<TextareaState>,
        id: &'static str,
    ) -> Div {
        h_flex()
            .items_start()
            .gap_2()
            .child(div().w(px(128.)).child(
                super::super::labels::form_field(&self.preferences.language, label).to_string(),
            ))
            .child(Textarea::new(state).accessibility_id(id).h(px(82.)).w(px(280.)))
    }

    pub(super) fn render_choice_field(
        &self,
        label: &str,
        key: &'static str,
        id: &'static str,
        options: &[(&str, &str, &str)],
        cx: &mut Context<Self>,
    ) -> Div {
        let selected = form::read_extra_field(
            &self.game_config.extra_fields,
            &self.game_config.extra_textareas,
            key,
            cx,
        );
        h_flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(div().w(px(128.)).child(
                super::super::labels::form_field(&self.preferences.language, label).to_string(),
            ))
            .children(options.iter().enumerate().map(|(index, (value, chinese, english))| {
                let selected_option = selected == *value;
                let value = (*value).to_string();
                Button::new(format!("{id}-{index}"))
                    .label(crate::i18n::text(&self.preferences.language, chinese, english))
                    .when(selected_option, |button| button.primary())
                    .when(!selected_option, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_game_config_extra_field(key, &value, window, cx)
                    }))
            }))
    }
}
