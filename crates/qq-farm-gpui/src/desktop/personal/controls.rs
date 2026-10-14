use crate::desktop::*;

impl FarmDesktop {
    pub(in crate::desktop) fn append_personal_controls(
        &self,
        controls: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        match (self.page, self.page_section.as_str()) {
            (Page::Personal, "lands") => self.append_farming_controls(controls, cx),
            (Page::Personal, "bag") => self.append_bag_controls(controls, cx),
            _ => controls,
        }
    }
}
