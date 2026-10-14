use gpui_kit::component::combobox::{Combobox, ComboboxState};
use gpui_kit::component::searchable_list::{SearchableListDelegate, SearchableListItem};
use gpui_kit::component::IndexPath;
use gpui_kit::{App, SharedString, Task, Window};
use serde_json::Value;

use super::super::super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PetDiaryFriendOption {
    gid: String,
    label: String,
}

impl SearchableListItem for PetDiaryFriendOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.gid
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.label.to_lowercase().contains(&query) || self.gid.to_lowercase().contains(&query)
    }
}

#[derive(Clone)]
pub(crate) struct PetDiaryFriendDelegate {
    language: String,
    options: Vec<PetDiaryFriendOption>,
    visible: Vec<usize>,
    custom_gid: Option<PetDiaryFriendOption>,
}

impl PetDiaryFriendDelegate {
    fn new(options: Vec<PetDiaryFriendOption>, language: &str) -> Self {
        Self {
            language: language.to_string(),
            visible: (0..options.len()).collect(),
            options,
            custom_gid: None,
        }
    }

    pub(crate) fn empty(language: &str) -> Self {
        Self::new(Vec::new(), language)
    }
}

impl SearchableListDelegate for PetDiaryFriendDelegate {
    type Item = PetDiaryFriendOption;

    fn items_count(&self, _: usize) -> usize {
        self.visible.len() + usize::from(self.custom_gid.is_some())
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.visible.get(ix.row).and_then(|index| self.options.get(*index)).or_else(|| {
            (ix.row == self.visible.len()).then_some(self.custom_gid.as_ref()).flatten()
        })
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.visible
            .iter()
            .position(|index| self.options[*index].value() == value)
            .map(IndexPath::new)
    }

    fn perform_search(&mut self, query: &str, _: &mut Window, _: &mut App) -> Task<()> {
        let query = query.trim();
        self.visible = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| option.matches(query).then_some(index))
            .collect();
        self.custom_gid = (!query.is_empty() && self.visible.is_empty())
            .then(|| query.to_string())
            .map(|gid| PetDiaryFriendOption {
                label: if self.language == "en-US" {
                    format!("Use GID {gid}")
                } else {
                    format!("使用 GID {gid}")
                },
                gid,
            });
        Task::ready(())
    }
}

pub(crate) type PetDiaryFriendSelector = ComboboxState<PetDiaryFriendDelegate>;

pub(crate) fn options(
    raw: &Value,
    selected_gid: &str,
    language: &str,
) -> Vec<PetDiaryFriendOption> {
    let unnamed = crate::i18n::text(language, "未命名", "Unnamed");
    let mut options =
        raw.get("petDiaryFriends")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|friend| {
                let gid = friend.get("gid").and_then(Value::as_str).map(str::to_string).or_else(
                    || friend.get("gid").and_then(Value::as_i64).map(|gid| gid.to_string()),
                )?;
                if gid.parse::<i64>().ok().is_none_or(|gid| gid <= 0) {
                    return None;
                }
                let nickname = friend
                    .get("nickname")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(unnamed);
                Some(PetDiaryFriendOption { label: format!("{nickname}（{gid}）"), gid })
            })
            .collect::<Vec<_>>();
    options.sort_by(|left, right| {
        left.label
            .to_lowercase()
            .cmp(&right.label.to_lowercase())
            .then_with(|| left.gid.cmp(&right.gid))
    });

    if !selected_gid.is_empty() && !options.iter().any(|option| option.gid == selected_gid) {
        options.push(PetDiaryFriendOption {
            label: format!("GID {selected_gid}"),
            gid: selected_gid.to_string(),
        });
    }
    options
}

pub(crate) fn render(
    language: &str,
    selector: &Entity<PetDiaryFriendSelector>,
) -> impl IntoElement {
    let selector = div().id("pet-diary-friend-gid").child(
        Combobox::new(selector)
            .placeholder(crate::i18n::text(
                language,
                "搜索好友（可手输 GID）",
                "Search friends or enter a GID",
            ))
            .search_placeholder(crate::i18n::text(
                language,
                "搜索好友名称或 GID",
                "Search friends by name or GID",
            ))
            .menu_width(px(320.))
            .menu_max_h(px(300.))
            .cleanable(true),
    );
    #[cfg(test)]
    let selector = selector.test_support();
    selector
}

impl FarmDesktop {
    pub(in crate::desktop) fn sync_pet_diary_friend_selector(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) {
        let selected_gid = self.activities.pet_diary_friend_gid.clone();
        let options = options(&self.page_raw, &selected_gid, &self.preferences.language);
        let delegate = PetDiaryFriendDelegate::new(options, &self.preferences.language);
        self.activities.pet_diary_friend_selector.update(cx, |selector, cx| {
            selector.set_items(delegate, window, cx);
            if selected_gid.is_empty() {
                selector.set_selected_indices([], window, cx);
            } else {
                selector.set_selected_values(&[selected_gid], window, cx);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{options, PetDiaryFriendDelegate};
    use gpui_kit::component::searchable_list::{SearchableListDelegate, SearchableListItem};
    use gpui_kit::component::IndexPath;
    use gpui_kit::TestAppContext;

    #[test]
    fn friend_options_follow_vue_nickname_fallback_and_allow_manual_gid() {
        let options = options(
            &serde_json::json!({
                "petDiaryFriends": [
                    {"gid": "42", "nickname": "Alice", "name": "Ignored"},
                    {"gid": "43", "nickname": ""},
                    {"gid": "0", "nickname": "Invalid"}
                ]
            }),
            "99",
            "zh-CN",
        );
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].title().as_ref(), "Alice（42）");
        assert_eq!(options[1].title().as_ref(), "未命名（43）");
        assert_eq!(options[2].value(), "99");
    }

    #[gpui_kit::test]
    fn typed_numeric_gid_is_an_extra_search_result(cx: &mut TestAppContext) {
        cx.add_empty_window().update(|window, cx| {
            let mut delegate = PetDiaryFriendDelegate::new(
                options(
                    &serde_json::json!({"petDiaryFriends": [{"gid": "42", "nickname": "Alice"}]}),
                    "",
                    "zh-CN",
                ),
                "zh-CN",
            );
            _ = delegate.perform_search("99", window, cx);
            assert_eq!(delegate.items_count(0), 1);
            assert_eq!(delegate.item(IndexPath::new(0)).unwrap().title().as_ref(), "使用 GID 99");
            assert_eq!(delegate.item(IndexPath::new(0)).unwrap().value(), "99");

            _ = delegate.perform_search("invalid input", window, cx);
            assert_eq!(delegate.item(IndexPath::new(0)).unwrap().value(), "invalid input");
        });
    }

    #[gpui_kit::test]
    fn name_search_returns_friends_without_a_manual_tag(cx: &mut TestAppContext) {
        cx.add_empty_window().update(|window, cx| {
            let mut delegate = PetDiaryFriendDelegate::new(
                options(
                    &serde_json::json!({"petDiaryFriends": [{"gid": "42", "nickname": "Alice"}]}),
                    "",
                    "zh-CN",
                ),
                "zh-CN",
            );
            _ = delegate.perform_search("Ali", window, cx);
            assert_eq!(delegate.items_count(0), 1);
            assert_eq!(delegate.item(IndexPath::new(0)).unwrap().value(), "42");
        });
    }
}
