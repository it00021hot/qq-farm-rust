use nsh_core::platform::Locale;
use nsh_core::prelude::*;
use nsh_core::storage::Storage;

use crate::i18n::{
    appearance_id, appearance_value, language_id, text_key, APPEARANCE_KEY, LANGUAGE_KEY,
};
use crate::ui::{button, ButtonVariant};

const APPEARANCES: &[(&str, &str)] = &[
    ("system", "settings.appearanceSystem"),
    ("light", "settings.appearanceLight"),
    ("dark", "settings.appearanceDark"),
];

const LANGUAGES: &[(&str, &str)] = &[("zh-CN", "简体中文"), ("en-US", "English")];

pub(crate) fn section() -> impl IntoElement {
    let appearance = signal(appearance_id(Storage::get_item(APPEARANCE_KEY).as_deref()));
    let language =
        signal(language_id(Storage::get_item(LANGUAGE_KEY).as_deref(), locale().as_str()));
    view()
        .gap_2()
        .child(text(move || text_key("settings.appearance")).text_sm())
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            || APPEARANCES.to_vec(),
            |choice: &(&str, &str)| choice.0.to_owned(),
            move |choice, _| {
                let (id, label) = choice.get_untracked();
                button(move || text_key(label), ButtonVariant::Quiet)
                    .selected(move || appearance.get() == id)
                    .on_press(move || choose_appearance(appearance, id))
            },
        )))
        .child(text(move || text_key("settings.language")).text_sm())
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            || LANGUAGES.to_vec(),
            |choice: &(&str, &str)| choice.0.to_owned(),
            move |choice, _| {
                let (id, label) = choice.get_untracked();
                button(move || SharedString::from(label), ButtonVariant::Quiet)
                    .selected(move || language.get() == id)
                    .on_press(move || choose_language(language, id))
            },
        )))
}

fn choose_appearance(appearance: Signal<&'static str>, id: &'static str) {
    set_color_scheme(appearance_value(id));
    let _ = Storage::set_item(APPEARANCE_KEY, id);
    appearance.set(id);
}

fn choose_language(language: Signal<&'static str>, id: &'static str) {
    set_locale(Some(Locale::parse(id).expect("supported locale")));
    let _ = Storage::set_item(LANGUAGE_KEY, id);
    language.set(id);
}
