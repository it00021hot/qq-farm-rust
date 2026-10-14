use super::super::super::*;
use super::super::AccountColumnSetting;

impl FarmDesktop {
    pub(super) fn render_account_header(
        &self,
        columns: &[AccountColumnSetting],
        selection: Option<(Vec<String>, bool)>,
        stretch: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row =
            h_flex().items_center().gap_3().px_4().h(px(48.)).flex_none().bg(cx.theme().group_box);
        if let Some((page_ids, all_selected)) = selection {
            row = row.child(
                Checkbox::new("account-select-all")
                    .checked(all_selected)
                    .accessibility_label(crate::i18n::text(
                        &self.preferences.language,
                        "选择当前页账号",
                        "Select accounts on this page",
                    ))
                    .on_change(cx.listener(move |this, checked, _, cx| {
                        for id in &page_ids {
                            if *checked {
                                this.accounts.selected.insert(id.clone());
                            } else {
                                this.accounts.selected.remove(id);
                            }
                        }
                        cx.notify();
                    })),
            );
        }
        let language = &self.preferences.language;
        row.children(columns.iter().map(|column| {
            super::account_cell_frame(stretch, column.id.width())
                .id(format!("account-column-header-{}", column.id.key()))
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(column.id.title(language))
        }))
    }
}
