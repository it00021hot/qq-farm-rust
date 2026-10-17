//! JSON 值到展示文本的转换。

/// 把 JSON 值（字符串 / 整数）转成展示文本；其他类型为空串。
pub(crate) fn value_to_action_string(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .unwrap_or_default()
}
