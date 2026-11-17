use nsh_core::i18n::Translations;
use nsh_core::platform::Locale;
use nsh_core::prelude::*;

pub(crate) const LANGUAGE_KEY: &str = "qq-farm.locale";
pub(crate) const APPEARANCE_KEY: &str = "qq-farm.appearance";

pub(crate) fn install() {
    let translations = Translations::from_json(include_str!("../locales.json")).expect("locales");
    global::<Translations>().set(translations);
    let saved = nsh_core::storage::Storage::get_item(LANGUAGE_KEY);
    let language = language_id(saved.as_deref(), locale().as_str());
    set_locale(Some(Locale::parse(language).expect("supported locale")));
    let appearance = nsh_core::storage::Storage::get_item(APPEARANCE_KEY);
    set_color_scheme(appearance_value(appearance_id(appearance.as_deref())));
}

pub(crate) fn language_id(saved: Option<&str>, system_language: &str) -> &'static str {
    match saved {
        Some("zh-CN") => "zh-CN",
        Some("en-US") => "en-US",
        _ if system_language.starts_with("zh") => "zh-CN",
        _ => "en-US",
    }
}

pub(crate) fn appearance_id(saved: Option<&str>) -> &'static str {
    match saved {
        Some("light") => "light",
        Some("dark") => "dark",
        _ => "system",
    }
}

pub(crate) fn appearance_value(id: &str) -> Appearance {
    match id {
        "light" => Appearance::Light,
        "dark" => Appearance::Dark,
        _ => Appearance::System,
    }
}

pub(crate) fn text_key(key: &'static str) -> SharedString {
    SharedString::from(t!(key))
}

#[cfg(test)]
mod tests {
    use super::{appearance_id, appearance_value, language_id};
    use nsh_core::prelude::Appearance;

    #[test]
    fn language_keeps_a_saved_choice_and_otherwise_follows_the_system() {
        assert_eq!(language_id(Some("en-US"), "zh-CN"), "en-US");
        assert_eq!(language_id(Some("zh-CN"), "en-US"), "zh-CN");
        assert_eq!(language_id(Some("fr"), "zh-Hans"), "zh-CN");
        assert_eq!(language_id(None, "en"), "en-US");
        assert_eq!(language_id(Some(""), "zh"), "zh-CN");
    }

    #[test]
    fn appearance_only_accepts_light_dark_or_system() {
        assert_eq!(appearance_id(Some("dark")), "dark");
        assert_eq!(appearance_id(Some("light")), "light");
        assert_eq!(appearance_id(Some("neon")), "system");
        assert_eq!(appearance_id(None), "system");
        assert_eq!(appearance_value("dark"), Appearance::Dark);
        assert_eq!(appearance_value("light"), Appearance::Light);
        assert_eq!(appearance_value("system"), Appearance::System);
        assert_eq!(appearance_value(""), Appearance::System);
    }
}
