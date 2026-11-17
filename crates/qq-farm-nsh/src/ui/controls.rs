use nsh_base::theme;
use nsh_core::element::StyleRefinement;
use nsh_core::prelude::*;

use super::motion::{FAST_MS, PRESS_SCALE};

#[derive(Clone, Copy)]
pub(crate) enum ButtonVariant {
    Primary,
    Secondary,
    Quiet,
    Danger,
}

pub(crate) fn button(
    label: impl IntoReactive<SharedString>,
    variant: ButtonVariant,
) -> nsh_base::ButtonElement {
    let label = label.into_reactive();
    let name = label.clone();
    surface(variant)
        .accessibility_label(move || Some(name.get()))
        .child(text(label).text_sm().font_weight(FontWeight::MEDIUM))
}

fn surface(variant: ButtonVariant) -> nsh_base::ButtonElement {
    nsh_base::button()
        .min_h(px(32.0))
        .px_3()
        .items_center()
        .justify_center()
        .rounded(move || theme().radius)
        .bg(move || fill(variant))
        .text_color(move || ink(variant))
        .transition()
        .duration(move || if reduce_motion() { 0.0 } else { FAST_MS })
        .ease_out()
        .hover(move |style| style.bg(move || hover_fill(variant)).text_color(move || ink(variant)))
        .active(|style| {
            style.opacity(0.8).scale(move || if reduce_motion() { 1.0 } else { PRESS_SCALE })
        })
        .disabled_style(|style: StyleRefinement| style.opacity(0.5))
        .focus_visible(|style| style.outline(px(2.0)).outline_color(move || theme().ring))
}

fn fill(variant: ButtonVariant) -> nsh_core::element::AdaptiveColor {
    match variant {
        ButtonVariant::Primary => theme().primary,
        ButtonVariant::Secondary => theme().secondary,
        ButtonVariant::Quiet => theme().background.opacity(0.0),
        ButtonVariant::Danger => token("destructive"),
    }
}

fn hover_fill(variant: ButtonVariant) -> nsh_core::element::AdaptiveColor {
    match variant {
        ButtonVariant::Primary => theme().primary.opacity(0.9),
        ButtonVariant::Danger => token("destructive").opacity(0.9),
        ButtonVariant::Secondary | ButtonVariant::Quiet => theme().accent,
    }
}

fn ink(variant: ButtonVariant) -> nsh_core::element::AdaptiveColor {
    match variant {
        ButtonVariant::Primary => theme().primary_foreground,
        ButtonVariant::Danger => token("destructive-foreground"),
        ButtonVariant::Secondary | ButtonVariant::Quiet => theme().foreground,
    }
}

pub(crate) fn token(name: &'static str) -> nsh_core::element::AdaptiveColor {
    theme().color(name).unwrap_or(theme().foreground)
}

pub(crate) fn field(
    value: impl IntoReactive<String>,
    placeholder: impl IntoReactive<String>,
    changed: impl Fn(&str) + 'static,
) -> nsh_base::Input {
    nsh_base::input()
        .min_h(px(32.0))
        .px_3()
        .flex_1()
        .rounded(move || theme().radius)
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .border(px(1.0))
        .border_color(move || theme().border)
        .value(value)
        .placeholder(placeholder)
        .on_change(move |text: &str| changed(text))
}

pub(crate) fn choice(
    checked: impl IntoReactive<bool>,
    label: impl IntoReactive<SharedString>,
) -> nsh_base::RadioElement {
    let checked = checked.into_reactive();
    let checked_now = checked.clone();
    nsh_base::radio()
        .checked(checked)
        .min_h(px(32.0))
        .flex_row()
        .items_center()
        .px_3()
        .rounded(move || theme().radius)
        .text_color(move || theme().sidebar_foreground)
        .transition()
        .duration(move || if reduce_motion() { 0.0 } else { FAST_MS })
        .ease_out()
        .hover(move |style| {
            style.bg(
                move || {
                    if checked_now.get() {
                        theme().sidebar_accent
                    } else {
                        theme().accent
                    }
                },
            )
        })
        .checked_style(|style: StyleRefinement| {
            style
                .bg(move || theme().sidebar_accent)
                .text_color(move || theme().sidebar_accent_foreground)
        })
        .focus_visible(|style| style.outline(px(2.0)).outline_color(move || theme().ring))
        .child(text(label).text_sm())
}
