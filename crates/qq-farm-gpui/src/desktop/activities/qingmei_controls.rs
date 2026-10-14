use super::super::*;

impl FarmDesktop {
    pub(super) fn toggle_qingmei_ingredient(
        &mut self,
        uid: String,
        stock: i64,
        cx: &mut Context<Self>,
    ) {
        if self.activities.qingmei_selected_ingredients.remove(&uid).is_none() {
            self.activities.qingmei_selected_ingredients.insert(uid, stock.max(1));
        }
        cx.notify();
    }

    pub(super) fn adjust_qingmei_ingredient(
        &mut self,
        uid: String,
        stock: i64,
        delta: i64,
        cx: &mut Context<Self>,
    ) {
        let count = self.activities.qingmei_selected_ingredients.entry(uid).or_insert(1);
        *count = (*count + delta).clamp(1, stock.max(1));
        cx.notify();
    }

    pub(super) fn set_qingmei_ingredient_count(
        &mut self,
        uid: String,
        stock: i64,
        count: i64,
        cx: &mut Context<Self>,
    ) {
        if let Some(selected) = self.activities.qingmei_selected_ingredients.get_mut(&uid) {
            *selected = count.clamp(1, stock.max(1));
            cx.notify();
        }
    }

    pub(super) fn set_qingmei_all(
        &mut self,
        ingredients: &[(String, i64)],
        cx: &mut Context<Self>,
    ) {
        let all_selected = !ingredients.is_empty()
            && ingredients
                .iter()
                .all(|(uid, _)| self.activities.qingmei_selected_ingredients.contains_key(uid));
        if all_selected {
            self.activities.qingmei_selected_ingredients.clear();
        } else {
            self.activities.qingmei_selected_ingredients = ingredients
                .iter()
                .map(|(uid, stock)| {
                    let count = self
                        .activities
                        .qingmei_selected_ingredients
                        .get(uid)
                        .copied()
                        .unwrap_or(*stock)
                        .clamp(1, (*stock).max(1));
                    (uid.clone(), count)
                })
                .collect();
        }
        cx.notify();
    }

    pub(super) fn clear_qingmei_selection(&mut self, cx: &mut Context<Self>) {
        self.activities.qingmei_selected_ingredients.clear();
        cx.notify();
    }
}
