use nsh_core::prelude::*;

use crate::i18n::text_key;
use crate::navigation::Section;
use crate::ui::notice;

pub(crate) fn page(section: Section) -> impl IntoElement {
    let detail = section.detail_key().unwrap_or("common.unavailable");
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(
            text(move || text_key(section.label_key())).text_lg().font_weight(FontWeight::SEMIBOLD),
        )
        .child(notice(move || text_key(detail)))
}
