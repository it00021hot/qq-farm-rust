mod state;
pub(crate) use state::DashboardState;

use super::*;
mod account;
mod assets;
mod clock;
mod events;
mod format;
mod logs;
mod operations;
mod refresh;
mod schedule;
mod view;

impl FarmDesktop {
    pub(super) fn append_dashboard_controls(&self, controls: Div, _cx: &mut Context<Self>) -> Div {
        controls
    }

    pub(super) fn render_dashboard_panel(&self, cx: &mut Context<Self>) -> Div {
        self.render_dashboard_view(cx)
    }
}
