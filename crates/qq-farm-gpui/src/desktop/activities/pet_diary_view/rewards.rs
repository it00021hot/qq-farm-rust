mod seeds;
mod shop;
mod solar_terms;
mod stories;

use super::super::super::*;
use serde_json::Value;

pub(super) fn seeds(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    seeds::render(this, snapshot, cx)
}

pub(super) fn shop(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    shop::render(this, snapshot, cx)
}

pub(super) fn stories(this: &FarmDesktop, snapshot: &Value, cx: &mut Context<FarmDesktop>) -> Div {
    stories::render(this, snapshot, cx)
}

pub(super) fn solar_terms(
    this: &FarmDesktop,
    snapshot: &Value,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    solar_terms::render(this, snapshot, cx)
}
