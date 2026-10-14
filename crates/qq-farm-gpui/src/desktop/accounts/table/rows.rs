use super::super::super::*;
use super::super::{account_value, AccountColumnSetting};

impl FarmDesktop {
    pub(super) fn render_account_row(
        &self,
        columns: &[AccountColumnSetting],
        row_index: usize,
        include_selection: bool,
        stretch: bool,
        record: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = h_flex()
            .items_center()
            .gap_3()
            .px_4()
            .h(px(64.))
            .flex_none()
            .border_b_1()
            .border_color(cx.theme().border);
        let id = account_value(record, &["id"]);
        // Highlight the whole row while the account is checked, matching the
        // game-config table and Vue's checked-row treatment. Pinned lanes call
        // this renderer too, so the highlight stays continuous across lanes.
        let is_selected = self.accounts.selected.contains(&id);
        if include_selection {
            let name = account_value(record, &["name"]);
            let id_for_checkbox = id.clone();
            row = row.child(
                Checkbox::new(format!("account-select-{id}"))
                    .checked(is_selected)
                    .accessibility_label(if self.preferences.language == "en-US" {
                        format!("Select account {name}")
                    } else {
                        format!("选择账号 {name}")
                    })
                    .on_change(cx.listener(move |this, checked, _, cx| {
                        if *checked {
                            this.accounts.selected.insert(id_for_checkbox.clone());
                        } else {
                            this.accounts.selected.remove(&id_for_checkbox);
                        }
                        cx.notify();
                    })),
            );
        }
        row.when(is_selected, |row| row.bg(cx.theme().group_box)).children(columns.iter().map(
            |column| self.render_account_column_cell(column.id, row_index, record, stretch, cx),
        ))
    }
}
