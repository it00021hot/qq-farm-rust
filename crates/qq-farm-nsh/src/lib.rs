mod features;
mod host;
mod i18n;
mod navigation;
mod purchase_count;
mod session;
mod shell;
mod ui;

pub use host::run;

use nsh_base::theme;
use nsh_core::prelude::*;

#[native_module("qq-farm/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

#[component]
pub fn App() -> impl IntoElement {
    i18n::install();
    set_tokens(nsh_base::tokens_of(Some(include_str!("../theme.json"))).expect("validated theme"));
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(shell::workspace())
}
