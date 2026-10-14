use serde_json::Value;

use crate::desktop::activities::value_to_action_string;

pub(in crate::desktop::activities::actions) fn prepare(
    page_raw: &Value,
    choice_id: &str,
    language: &str,
) -> Result<String, String> {
    let choice = choice_id.trim().to_string();
    let exists =
        page_raw.pointer("/autumn/wish/choices").and_then(Value::as_array).is_some_and(|choices| {
            choices
                .iter()
                .any(|item| item.get("id").map(value_to_action_string).as_deref() == Some(&choice))
        });

    if exists {
        Ok(choice)
    } else {
        Err(crate::i18n::text(
            language,
            "请先选择当前秋日祈愿选项。",
            "Choose a wish option from the current autumn activity.",
        )
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::prepare;
    use serde_json::json;

    #[test]
    fn draw_requires_an_id_from_the_loaded_choice_list() {
        let raw = json!({"autumn":{"wish":{"choices":[{"id":12},{"id":"34"}]}}});

        assert_eq!(prepare(&raw, "12", "zh-CN"), Ok("12".into()));
        assert_eq!(prepare(&raw, "34", "zh-CN"), Ok("34".into()));
        assert_eq!(
            prepare(&raw, "99", "en-US").unwrap_err(),
            "Choose a wish option from the current autumn activity."
        );
    }
}
