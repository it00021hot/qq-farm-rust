use super::super::super::*;
use super::super::AccountColumnSetting;
use gpui_kit::base::v_flex;

impl FarmDesktop {
    /// One pinned column lane: a fixed header row plus a body that scrolls
    /// vertically in lockstep with the other lanes through the shared handle.
    pub(super) fn render_account_pinned_lane(
        &self,
        columns: &[AccountColumnSetting],
        records: &[serde_json::Value],
        side: &'static str,
        row_scroll: &ScrollHandle,
        show_scrollbar: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let width = columns.iter().map(|column| column.id.width()).sum::<f32>()
            + 12. * columns.len().saturating_sub(1) as f32
            + 32.;
        let rows = div()
            .id(format!("account-pinned-{side}-rows"))
            .size_full()
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .track_scroll(row_scroll)
            .children(records.iter().enumerate().map(|(index, record)| {
                self.render_account_row(columns, index, false, false, record, cx)
            }));
        let body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .when(show_scrollbar, |body| body.vertical_scrollbar(row_scroll))
            .child(rows);
        let lane = v_flex()
            .id(format!("account-pinned-{side}"))
            .flex_none()
            .w(px(width))
            .h_full()
            .when(side == "left", |lane| lane.border_r_1().border_color(cx.theme().border))
            .when(side == "right", |lane| lane.border_l_1().border_color(cx.theme().border))
            .child(self.render_account_header(columns, None, false, cx))
            .child(body);
        #[cfg(test)]
        let lane = lane.test_support();
        lane.into_any_element()
    }
}
