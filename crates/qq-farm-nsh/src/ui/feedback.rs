use nsh_base::theme;
use nsh_core::prelude::*;

use super::controls::token;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeTone {
    Neutral,
    Success,
}

pub(crate) fn badge(label: impl IntoReactive<SharedString>, tone: BadgeTone) -> View {
    view()
        .px_2()
        .py_1()
        .rounded(move || theme().radius)
        .bg(move || match tone {
            BadgeTone::Neutral => theme().muted,
            BadgeTone::Success => token("success"),
        })
        .child(text(label).text_xs().text_color(move || match tone {
            BadgeTone::Neutral => theme().muted_foreground,
            BadgeTone::Success => token("success-foreground"),
        }))
}

pub(crate) fn notice(message: impl IntoReactive<SharedString>) -> View {
    let message = message.into_reactive();
    view().py_2().child(text(message).text_sm().text_color(move || theme().muted_foreground))
}

pub(crate) fn failure(message: impl IntoReactive<SharedString>) -> View {
    let message = message.into_reactive();
    let visible = message.clone();
    view()
        .py_2()
        .style_when(move || visible.get().is_empty(), |style| style.hidden())
        .child(text(message).text_sm().text_color(move || token("destructive")))
}
