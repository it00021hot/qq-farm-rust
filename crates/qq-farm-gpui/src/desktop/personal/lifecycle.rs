use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn prepare_personal_section(
        &mut self,
        section: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(raw) = self.page_raw.as_object_mut() {
            raw.remove(section);
            if section == "pets" {
                raw.remove("petGiftStatus");
            }
            if let Some(errors) =
                raw.get_mut("sectionErrors").and_then(serde_json::Value::as_object_mut)
            {
                errors.remove(section);
                if section == "pets" {
                    errors.remove("petGiftStatus");
                }
            }
        }

        match section {
            "lands" => self.farm.lands.error = None,
            "bag" => {
                self.farm.bag.category = "fruit".into();
                self.farm.bag.batch_mode = false;
                self.farm.bag.selected_ids.clear();
                self.farm.bag.error = None;
                self.farm.bag.notice = None;
                self.farm.bag.notice_generation = self.farm.bag.notice_generation.wrapping_add(1);
            }
            "pets" => {
                self.farm.pets.food_inputs.clear();
                self.farm.pets.logs.clear();
                self.farm.pets.notice = None;
                self.farm.pets.notice_generation = self.farm.pets.notice_generation.wrapping_add(1);
            }
            "illustrated" => self.farm.illustrated.book = "crop".into(),
            "interactionItems" => {
                self.farm.interaction.land_ids.update(cx, |state, cx| {
                    state.set_value("", window, cx);
                });
                self.farm.interaction.picker.update(cx, |picker, cx| {
                    picker.set_selected_index(None, window, cx);
                });
            }
            _ => {}
        }
    }
}
