use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{SearchableVec, SelectState};
use gpui_kit::component::IndexPath;
use gpui_kit::SharedString;

#[derive(Clone)]
pub(in crate::desktop) struct OfflineProviderOption {
    key: &'static str,
    label: &'static str,
    disabled: bool,
}

impl OfflineProviderOption {
    pub(in crate::desktop) const fn new(
        key: &'static str,
        label: &'static str,
        disabled: bool,
    ) -> Self {
        Self { key, label, disabled }
    }
}

impl SearchableListItem for OfflineProviderOption {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.label.into()
    }

    fn value(&self) -> &Self::Value {
        &self.key
    }

    fn disabled(&self) -> bool {
        self.disabled
    }
}

pub(in crate::desktop) type OfflineProviderSelect =
    SelectState<SearchableVec<OfflineProviderOption>>;

pub(in crate::desktop) fn offline_provider_options(language: &str) -> Vec<OfflineProviderOption> {
    [
        ("none", "不推送", false),
        ("qq_bot", "QQ 官方机器人", false),
        ("ding_talk", "钉钉机器人", false),
        ("wechat_bot", "微信机器人（敬请期待）", true),
    ]
    .into_iter()
    .map(|(key, _label, disabled)| {
        let label = match (key, language) {
            ("none", "en-US") => "Disabled",
            ("qq_bot", "en-US") => "Official QQ Bot",
            ("ding_talk", "en-US") => "DingTalk Bot",
            ("wechat_bot", "en-US") => "WeChat Bot (Coming Soon)",
            ("none", _) => "不推送",
            ("qq_bot", _) => "QQ 官方机器人",
            ("ding_talk", _) => "钉钉机器人",
            ("wechat_bot", _) => "微信机器人（敬请期待）",
            _ => "",
        };
        OfflineProviderOption::new(key, label, disabled)
    })
    .collect()
}

pub(in crate::desktop) fn provider_index(provider: &str) -> IndexPath {
    let row = match provider {
        "qq_bot" => 1,
        "ding_talk" => 2,
        "wechat_bot" => 3,
        _ => 0,
    };
    IndexPath::new(row)
}

#[cfg(test)]
mod tests {
    use super::provider_index;

    #[test]
    fn provider_selection_maps_each_supported_backend_to_its_option() {
        assert_eq!(provider_index("none").row, 0);
        assert_eq!(provider_index("qq_bot").row, 1);
        assert_eq!(provider_index("ding_talk").row, 2);
        assert_eq!(provider_index("wechat_bot").row, 3);
        assert_eq!(provider_index("unknown").row, 0);
    }
}
