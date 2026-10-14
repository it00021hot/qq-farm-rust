use gpui_kit::component::WindowExt;
#[cfg(test)]
use gpui_kit::TestSupportExt;
use std::collections::HashSet;

use super::super::*;

#[derive(Clone)]
struct RuleSection {
    title: String,
    paragraphs: Vec<String>,
}

fn strings(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::trim))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_rules(value: &serde_json::Value, fallback_title: &str) -> Option<RuleSection> {
    let paragraphs = strings(value.get("paragraphs"))
        .into_iter()
        .chain(strings(value.pointer("/tips/txt")))
        .chain(strings(value.get("lines")))
        .collect::<Vec<_>>();
    if paragraphs.is_empty() {
        return None;
    }
    let title = value
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or(fallback_title)
        .to_string();
    Some(RuleSection { title, paragraphs })
}

fn collect_rules(value: &serde_json::Value, fallback_title: &str, output: &mut Vec<RuleSection>) {
    if let Some(section) = normalize_rules(value, fallback_title) {
        output.push(section);
        return;
    }
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object {
                let title = child
                    .get("name")
                    .or_else(|| child.get("title"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or(fallback_title);
                if key == "rules" {
                    if let Some(section) = normalize_rules(child, title) {
                        output.push(section);
                    } else {
                        collect_rules(child, title, output);
                    }
                } else {
                    collect_rules(child, title, output);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_rules(item, fallback_title, output);
            }
        }
        _ => {}
    }
}

impl FarmDesktop {
    fn activity_rule_sections(&self) -> Vec<RuleSection> {
        let null = serde_json::Value::Null;
        let source = match self.page_section.as_str() {
            "season" => self
                .page_raw
                .pointer("/season/pass")
                .or_else(|| self.page_raw.get("season"))
                .unwrap_or(&null),
            "weather" => self
                .page_raw
                .pointer("/weather/snapshot")
                .or_else(|| self.page_raw.get("weather"))
                .unwrap_or(&null),
            "petDiary" => self.page_raw.get("petDiary").unwrap_or(&null),
            "autumn" => self.page_raw.get("autumn").unwrap_or(&null),
            "qixi" => self.page_raw.get("qixi").unwrap_or(&null),
            "charity" => self.page_raw.get("charity").unwrap_or(&null),
            "constellation" => self.page_raw.get("constellation").unwrap_or(&null),
            "qingmei" => self
                .page_raw
                .get("qingmei")
                .or_else(|| self.page_raw.get("qingMei"))
                .or_else(|| self.page_raw.get("greenPlum"))
                .unwrap_or(&null),
            _ => &self.page_raw,
        };
        let mut sections = Vec::new();
        collect_rules(source, "活动说明", &mut sections);
        for section in &mut sections {
            section.title =
                crate::i18n::activity_rule_title(&section.title, &self.preferences.language);
        }
        let mut seen = HashSet::new();
        sections
            .retain(|section| seen.insert((section.title.clone(), section.paragraphs.join("\n"))));
        sections
    }

    pub(in crate::desktop) fn open_activity_rules_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sections = self.activity_rule_sections();
        if sections.is_empty() {
            return;
        }
        let language = self.preferences.language.clone();
        let title = sections
            .first()
            .map(|section| section.title.clone())
            .unwrap_or_else(|| crate::i18n::text(&language, "活动规则", "Activity rules").into());
        let viewport = window.viewport_size();
        let width = (viewport.width.as_f32() - 32.).clamp(280., 520.);
        let max_height = (viewport.height.as_f32() * 0.68).max(240.);
        window.open_dialog(cx, move |dialog, _, app| {
            let content = div().id("activity-rules").v_flex().gap_3().children(
                sections.iter().enumerate().flat_map(|(index, section)| {
                    let mut rows = Vec::with_capacity(section.paragraphs.len() + 1);
                    if index > 0 || section.title != title {
                        rows.push(
                            div().font_medium().child(section.title.clone()).into_any_element(),
                        );
                    }
                    rows.extend(section.paragraphs.iter().map(|paragraph| {
                        div()
                            .text_sm()
                            .text_color(app.theme().muted_foreground)
                            .child(paragraph.clone())
                            .into_any_element()
                    }));
                    rows
                }),
            );
            #[cfg(test)]
            let content = content.test_support();
            let body = div().max_h(px(max_height)).overflow_y_scrollbar().child(content);
            dialog.title(title.clone()).w(px(width)).child(body)
        });
    }

    pub(in crate::desktop) fn render_activity_rules(&self, cx: &mut Context<Self>) -> AnyElement {
        let sections = self.activity_rule_sections();
        if sections.is_empty() {
            return div().into_any_element();
        }

        let rules = div().id("activity-rules").v_flex().gap_3().children(sections.into_iter().map(
            |section| {
                div()
                    .v_flex()
                    .gap_2()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().group_box)
                    .p_4()
                    .child(div().font_medium().child(section.title))
                    .children(section.paragraphs.into_iter().map(|paragraph| {
                        div().text_sm().text_color(cx.theme().muted_foreground).child(paragraph)
                    }))
            },
        ));
        #[cfg(test)]
        let rules = rules.test_support();
        rules.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::collect_rules;
    use serde_json::json;

    #[test]
    fn extracts_weather_rules_from_snapshot_payload() {
        let raw = json!({
            "weather": {
                "snapshot": {
                    "rules": {
                        "title": "雨落成诗规则",
                        "paragraphs": ["第一条", "  ", "第二条"]
                    }
                }
            }
        });
        let mut sections = Vec::new();
        collect_rules(&raw, "活动说明", &mut sections);

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "雨落成诗规则");
        assert_eq!(sections[0].paragraphs, ["第一条", "第二条"]);
    }

    #[test]
    fn extracts_vue_compatible_tip_and_line_shapes() {
        let raw = json!({
            "rules": {"tips": {"txt": ["提示一"]}, "lines": ["说明二"]}
        });
        let mut sections = Vec::new();
        collect_rules(&raw, "活动说明", &mut sections);

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].paragraphs, ["提示一", "说明二"]);
    }
}
