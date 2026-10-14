mod state;
pub(in crate::desktop) use state::IllustratedState;

mod card;
mod summary;

use super::super::*;
use std::rc::Rc;

impl FarmDesktop {
    pub(in crate::desktop) fn render_personal_illustrated(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let book = self
            .page_raw
            .get("illustrated")
            .and_then(|snapshot| snapshot.get(&self.farm.illustrated.book));
        let level_pill = book
            .and_then(|book| book.get("level"))
            .and_then(serde_json::Value::as_i64)
            .map(|level| {
                let chinese = format!("等级 {level}");
                let english = format!("Lv {level}");
                let label =
                    crate::i18n::text(&self.preferences.language, &chinese, &english).to_string();
                let pill = div()
                    .id("illustrated-level")
                    .aria_label(label.clone())
                    .rounded_full()
                    .bg(cx.theme().info.opacity(0.16))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().info)
                    .child(label);
                #[cfg(test)]
                let pill = pill.test_support();
                pill.into_any_element()
            });
        let mut panel = div().v_flex().flex_1().min_h_0().gap_3().child(
            h_flex()
                .items_center()
                .justify_between()
                .child(h_flex().items_center().gap_2().children(
                    [("crop", "作物图鉴"), ("mutant", "变异图鉴")].into_iter().map(
                        |(key, label)| {
                            Button::new(format!("illustrated-book-{key}"))
                                .label(crate::i18n::text(
                                    &self.preferences.language,
                                    label,
                                    if key == "crop" { "Crops" } else { "Mutants" },
                                ))
                                .when(self.farm.illustrated.book == key, |button| button.primary())
                                .when(self.farm.illustrated.book != key, |button| button.ghost())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.farm.illustrated.book = key.into();
                                    cx.notify();
                                }))
                        },
                    ),
                ))
                .children(level_pill),
        );
        let Some(book) = book.filter(|book| !book.is_null()) else {
            return panel.child(
                div().py_8().text_center().text_color(cx.theme().muted_foreground).child(
                    crate::i18n::text(
                        &self.preferences.language,
                        "暂无图鉴数据",
                        "No collection data",
                    ),
                ),
            );
        };
        panel = panel.child(self.render_illustrated_summary(book, cx));
        let query = self.search.read(cx).value().trim().to_lowercase();
        let items = book
            .get("items")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|item| query.is_empty() || item.to_string().to_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        if items.is_empty() {
            return panel.child(
                div().py_8().text_center().text_color(cx.theme().muted_foreground).child(
                    crate::i18n::text(
                        &self.preferences.language,
                        "没有可显示的图鉴项",
                        "No matching entries",
                    ),
                ),
            );
        }
        // 虚拟网格的行测量与卡片使用同一 rem 比例，缩放时一起变化。
        let rem = window.rem_size().as_f32();
        let sidebar =
            if self.sidebar_collapsed { 64. } else { self.preferences.sidebar_width as f32 };
        let available = (window.bounds().size.width.as_f32() - sidebar - rem * 3.).max(rem * 5.);
        let columns = ((available / (rem * 6.5)).floor() as usize).clamp(5, 9);
        let row_count = items.len().div_ceil(columns);
        let sizes = Rc::new(
            (0..row_count).map(|_| Size::new(px(available), window.rem_size() * 7.5)).collect(),
        );
        panel.child(
            gpui_kit::component::v_virtual_list(
                cx.entity(),
                "illustrated-grid",
                sizes,
                move |this, range, _, cx| {
                    range
                        .map(|row| {
                            h_flex().h(rems(7.5)).items_center().gap_2().children(
                                items
                                    .iter()
                                    .skip(row * columns)
                                    .take(columns)
                                    .map(|item| this.render_illustrated_item(item, cx)),
                            )
                        })
                        .collect::<Vec<_>>()
                },
            )
            .with_sizing_behavior(gpui_kit::ListSizingBehavior::Infer)
            .flex_1()
            .min_h_0(),
        )
    }
}
