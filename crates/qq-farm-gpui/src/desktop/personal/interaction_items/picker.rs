use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{SearchableVec, SelectState};
use gpui_kit::SharedString;

use super::super::super::*;

#[derive(Clone)]
pub(crate) struct InteractionItemOption {
    pub(crate) id: i64,
    pub(crate) label: String,
}

impl SearchableListItem for InteractionItemOption {
    type Value = i64;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &Self::Value {
        &self.id
    }
}

pub(crate) type InteractionItemSelect = SelectState<SearchableVec<InteractionItemOption>>;

impl FarmDesktop {
    pub(in crate::desktop) fn sync_personal_interaction_picker(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) {
        let items = self
            .page_raw
            .pointer("/interactionItems/items")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let id = item.get("itemId").and_then(serde_json::Value::as_i64)?;
                Some(InteractionItemOption {
                    id,
                    label: format!(
                        "{} x{}",
                        item.get("name").and_then(serde_json::Value::as_str).unwrap_or(
                            crate::i18n::text(
                                &self.preferences.language,
                                "互动道具",
                                "Interaction item",
                            ),
                        ),
                        item.get("count").and_then(serde_json::Value::as_i64).unwrap_or(0)
                    ),
                })
            })
            .collect::<Vec<_>>();
        let selected = self.farm.interaction.picker.read(cx).selected_value().copied();
        let selected = selected
            .filter(|id| items.iter().any(|item| item.id == *id))
            .or_else(|| items.first().map(|item| item.id));
        self.farm.interaction.picker.update(cx, |picker, cx| {
            picker.set_items(SearchableVec::new(items), window, cx);
            if let Some(selected) = selected {
                picker.set_selected_value(&selected, window, cx);
            } else {
                picker.set_selected_index(None, window, cx);
            }
        });
    }
}

pub(super) fn parse_land_ids(value: &str) -> Vec<i64> {
    value
        .split(|ch: char| ch == ',' || ch == '，' || ch.is_whitespace())
        .filter_map(|value| value.trim().parse::<i64>().ok())
        .filter(|id| *id > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn land_numbers_accept_chinese_commas_and_whitespace_like_vue() {
        assert_eq!(super::parse_land_ids("1， 2,3\n4\t5 -1 0 bad"), vec![1, 2, 3, 4, 5]);
    }
}
