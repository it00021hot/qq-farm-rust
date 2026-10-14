use super::super::events;
use crate::pages::DisplayRow;

fn module_matches(filter: &str, row: &DisplayRow, language: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let tag = row.record.get("tag").and_then(serde_json::Value::as_str).unwrap_or("");
    let message = humanize_log_message(&row.title, language);
    let terms: &[&str] = match filter {
        "farm" => &["农场", "收获", "种植", "施肥", "务农"],
        "friend" => &["好友", "偷菜", "帮助", "捣乱"],
        "system" => &["系统", "错误", "状态"],
        _ => return true,
    };
    terms.iter().any(|term| tag.contains(term) || message.contains(term))
}

pub(super) fn log_matches(
    row: &DisplayRow,
    module_filter: &str,
    event_filter: &str,
    level_filter: &str,
    keyword: &str,
    language: &str,
) -> bool {
    let event = event_key(row);
    if events::is_hidden(event)
        || (!event_filter.is_empty() && event != event_filter)
        || (level_filter == "warn" && row.section != "warnings")
        || (level_filter == "info" && row.section != "logs")
        || !module_matches(module_filter, row, language)
    {
        return false;
    }
    let keyword = keyword.trim().to_lowercase();
    keyword.is_empty()
        || format!(
            "{} {} {}",
            humanize_log_message(&row.title, language),
            row.record.get("tag").and_then(serde_json::Value::as_str).unwrap_or(""),
            events::label(event, language)
        )
        .to_lowercase()
        .contains(&keyword)
}

pub(super) fn should_follow_tail_after_scroll(
    offset_y: f32,
    max_offset_y: f32,
    delta_y: f32,
) -> bool {
    let max_offset_y = max_offset_y.max(0.);
    let projected_offset_y = (offset_y + delta_y).clamp(-max_offset_y, 0.);
    (projected_offset_y + max_offset_y).abs() <= 50.
}

pub(super) fn event_key(row: &DisplayRow) -> &str {
    row.record.pointer("/meta/event").and_then(serde_json::Value::as_str).unwrap_or("")
}

pub(super) fn humanize_log_message(message: &str, language: &str) -> String {
    let output = humanize_connection_context(message, language);
    let mut output = humanize_log_fields(&output, language);
    for token in
        ["heartbeat_timeout", "kickout_stop", "disconnect_stop", "kickout", "ws_close", "ws_400"]
    {
        output =
            replace_token_with_word_boundaries(&output, token, &events::label(token, language));
    }
    output
}

fn humanize_log_fields(message: &str, language: &str) -> String {
    let bytes = message.as_bytes();
    let mut output = String::with_capacity(message.len());
    let mut copy_from = 0;
    let mut index = 0;
    while index < bytes.len() {
        let field = ["source", "phase"].into_iter().find(|key| {
            let end = index + key.len();
            index + 1 < bytes.len()
                && (index == 0 || !is_log_word_byte(bytes[index - 1]))
                && message
                    .get(index..end)
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(key))
                && bytes.get(end) == Some(&b'=')
        });
        if let Some(field) = field {
            let value_start = index + field.len() + 1;
            let mut value_end = value_start;
            while value_end < bytes.len()
                && (bytes[value_end].is_ascii_alphanumeric() || bytes[value_end] == b'_')
            {
                value_end += 1;
            }
            if value_end > value_start {
                let value = &message[value_start..value_end];
                let replacement = if field == "source" {
                    events::label(value, language)
                } else {
                    let phase = match value.to_ascii_lowercase().as_str() {
                        "online" => crate::i18n::text(language, "在线", "Online").to_string(),
                        "login" => crate::i18n::text(language, "登录", "Login").to_string(),
                        "connecting" => {
                            crate::i18n::text(language, "连接中", "Connecting").to_string()
                        }
                        "closing" => crate::i18n::text(language, "断开中", "Closing").to_string(),
                        _ => value.to_string(),
                    };
                    format!("{}{}", phase, crate::i18n::text(language, "阶段", " phase"))
                };
                output.push_str(&message[copy_from..index]);
                output.push_str(&replacement);
                copy_from = value_end;
                index = value_end;
                continue;
            }
        }
        let Some(character) = message[index..].chars().next() else { break };
        index += character.len_utf8();
    }
    output.push_str(&message[copy_from..]);
    output
}

fn is_log_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn replace_token_with_word_boundaries(message: &str, token: &str, replacement: &str) -> String {
    let bytes = message.as_bytes();
    let mut output = String::with_capacity(message.len());
    let mut copy_from = 0;
    let mut search_from = 0;
    while let Some(relative) = message[search_from..].find(token) {
        let start = search_from + relative;
        let end = start + token.len();
        let has_word_boundary_before = start == 0 || !is_log_word_byte(bytes[start - 1]);
        let has_word_boundary_after = end == bytes.len() || !is_log_word_byte(bytes[end]);
        if has_word_boundary_before && has_word_boundary_after {
            output.push_str(&message[copy_from..start]);
            output.push_str(replacement);
            copy_from = end;
            search_from = end;
        } else {
            search_from = start + 1;
        }
    }
    output.push_str(&message[copy_from..]);
    output
}

fn humanize_connection_context(message: &str, language: &str) -> String {
    let mut output = message.to_string();
    let mut search_start = 0;
    loop {
        let lower = output.to_ascii_lowercase();
        let Some(relative_start) = lower[search_start..].find('(') else { break };
        let start = search_start + relative_start;
        let Some(relative_end) = lower[start..].find(')') else { break };
        let end = start + relative_end;
        let context = &output[start + 1..end];
        let Some((source, phase)) = connection_context_tokens(context) else {
            search_start = start + 1;
            continue;
        };

        let reason = events::label(source, language);
        let replacement = if phase.eq_ignore_ascii_case("online") {
            format!("（{reason}）")
        } else {
            let phase_label = match phase.to_ascii_lowercase().as_str() {
                "login" => crate::i18n::text(language, "登录", "Login"),
                "connecting" => crate::i18n::text(language, "连接中", "Connecting"),
                "closing" => crate::i18n::text(language, "断开中", "Closing"),
                _ => phase,
            };
            format!("（{reason}，{phase_label}{}）", crate::i18n::text(language, "阶段", " phase"))
        };
        output.replace_range(start..=end, &replacement);
        search_start = start + replacement.len();
    }
    output
}

fn connection_context_tokens(context: &str) -> Option<(&str, &str)> {
    let mut fields = context.split(',');
    let source = context_value(fields.next()?, "source")?;
    let phase = context_value(fields.next()?, "phase")?;
    if fields.next().is_some() {
        return None;
    }
    let is_valid_token = |value: &str| {
        !value.is_empty() && value.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    };
    (is_valid_token(source) && is_valid_token(phase)).then_some((source, phase))
}

fn context_value<'a>(field: &'a str, expected_key: &str) -> Option<&'a str> {
    let (key, value) = field.trim().split_once('=')?;
    key.trim().eq_ignore_ascii_case(expected_key).then_some(value.trim())
}

#[cfg(test)]
mod tests {
    use super::{humanize_log_message, log_matches, should_follow_tail_after_scroll};
    use crate::pages::DisplayRow;

    fn row(section: &str, tag: &str, event: &str, title: &str) -> DisplayRow {
        DisplayRow {
            id: format!("{section}-{event}"),
            section: section.into(),
            title: title.into(),
            detail: "12:00:00".into(),
            state: String::new(),
            record: serde_json::json!({"tag": tag, "meta": {"event": event}}),
        }
    }

    #[test]
    fn dashboard_log_filters_match_module_event_severity_and_humanized_keyword() {
        let farm = row("logs", "农场", "harvest_crop", "收获 source=harvest_crop");
        let warning = row("warnings", "错误", "ws_400", "source=ws_400");

        assert!(log_matches(&farm, "farm", "harvest_crop", "info", "收获作物", "zh-CN"));
        assert!(!log_matches(&farm, "friend", "", "", "", "zh-CN"));
        assert!(!log_matches(&farm, "", "plant_seed", "", "", "zh-CN"));
        assert!(!log_matches(&farm, "", "", "warn", "", "zh-CN"));
        assert!(log_matches(&warning, "system", "ws_400", "warn", "登录失效", "zh-CN"));
        assert!(!log_matches(&warning, "", "", "info", "", "zh-CN"));
    }

    #[test]
    fn dashboard_log_keyword_does_not_search_the_timestamp() {
        let log = row("logs", "农场", "harvest_crop", "收获完成");
        assert!(!log_matches(&log, "", "", "", "12:00", "zh-CN"));
        assert!(log_matches(&log, "", "", "", "收获", "zh-CN"));
    }

    #[test]
    fn dashboard_module_filter_uses_the_humanized_message_like_vue() {
        let log = row("logs", "其他", "", "source=harvest_crop");
        assert!(log_matches(&log, "farm", "", "", "", "zh-CN"));
    }

    #[test]
    fn log_event_labels_and_machine_tokens_follow_the_selected_language() {
        assert_eq!(
            humanize_log_message("source=ws_400 phase=connecting", "en-US"),
            "Session expired Connecting phase"
        );
        let warning = row("warnings", "错误", "ws_400", "source=ws_400");
        assert!(log_matches(&warning, "", "ws_400", "warn", "Session expired", "en-US"));
    }

    #[test]
    fn log_field_humanization_finds_punctuation_adjacent_fields_without_touching_identifiers() {
        assert_eq!(
            humanize_log_message("连接失败,(source=ws_400);phase=connecting!", "zh-CN"),
            "连接失败,(登录失效);连接中阶段!"
        );
        assert_eq!(
            humanize_log_message("identifier=not_source=ws_400", "en-US"),
            "identifier=not_source=Session expired"
        );
        assert_eq!(
            humanize_log_message("xheartbeat_timeout heartbeat_timeout!", "en-US"),
            "xheartbeat_timeout Heartbeat timeout!"
        );
    }

    #[test]
    fn parenthesized_connection_context_matches_vue_humanization() {
        assert_eq!(
            humanize_log_message("断开 (source=ws_400, phase=connecting)", "zh-CN"),
            "断开 （登录失效，连接中阶段）"
        );
        assert_eq!(
            humanize_log_message("Disconnected (SOURCE=ws_400, PHASE=online)", "en-US"),
            "Disconnected （Session expired）"
        );
        assert_eq!(
            humanize_log_message("断开 ( source = ws_400 , phase = connecting )", "zh-CN"),
            "断开 （登录失效，连接中阶段）"
        );
        assert_eq!(
            humanize_log_message("Disconnected ( SOURCE = ws_400 , PHASE = online )", "en-US"),
            "Disconnected （Session expired）"
        );
    }

    #[test]
    fn dashboard_log_tail_following_tracks_scroll_position() {
        assert!(should_follow_tail_after_scroll(-1000., 1000., -400.));
        assert!(!should_follow_tail_after_scroll(-1000., 1000., 300.));
        assert!(should_follow_tail_after_scroll(-800., 1000., -250.));
        assert!(should_follow_tail_after_scroll(0., 0., 0.));
    }
}
