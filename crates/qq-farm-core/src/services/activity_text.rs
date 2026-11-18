//! 活动文案工具：剥离活动说明里的 HTML（`<br>` 家族替换为换行，其余标签剥除）。

/// 对齐 bot 的 `replace(/<br\s*(\/\s*)?>/gi, '\n').replace(/<[^>]+>/g, '')`。
pub(crate) fn strip_html(line: &str) -> String {
    scan(line, true)
}

/// 仅剥除标签（对齐 bot 的 `replace(/<[^>]+>/g, '')`，`<br>` 直接移除不留换行）。
pub(crate) fn strip_tags(line: &str) -> String {
    scan(line, false)
}

/// 对齐 bot `textContent`：有 `tips.txt` 用那段和标题；否则按原文顺序收集字符串值并首次去重。
pub(crate) fn text_content(extra: &[u8]) -> (String, Vec<String>) {
    let text = String::from_utf8_lossy(extra).trim().to_owned();
    if text.is_empty() {
        return (String::new(), Vec::new());
    }
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
        let plain = plain_line(&text);
        return (String::new(), if plain.is_empty() { Vec::new() } else { vec![plain] });
    };
    let paragraphs = parsed
        .pointer("/tips/txt")
        .and_then(serde_json::Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(serde_json::Value::as_str)
                .map(plain_line)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !paragraphs.is_empty() {
        let title = parsed
            .pointer("/tips/title")
            .and_then(serde_json::Value::as_str)
            .map(plain_line)
            .unwrap_or_default();
        return (title, paragraphs);
    }
    (String::new(), ordered_string_values(&text))
}

fn plain_line(value: &str) -> String {
    strip_html(value).trim().to_owned()
}

fn ordered_string_values(text: &str) -> Vec<String> {
    use serde::de::DeserializeSeed as _;
    struct Seed<'a>(&'a mut Vec<String>);
    impl<'de, 'a> serde::de::DeserializeSeed<'de> for Seed<'a> {
        type Value = ();
        fn deserialize<D>(self, deserializer: D) -> std::result::Result<(), D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            deserializer.deserialize_any(self)
        }
    }
    impl<'de, 'a> serde::de::Visitor<'de> for Seed<'a> {
        type Value = ();
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("json")
        }
        fn visit_str<E: serde::de::Error>(self, value: &str) -> std::result::Result<(), E> {
            let plain = plain_line(value);
            if !plain.is_empty() && !self.0.iter().any(|have| have == &plain) {
                self.0.push(plain);
            }
            Ok(())
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<(), A::Error> {
            let output = self.0;
            while seq.next_element_seed(Seed(output))?.is_some() {}
            Ok(())
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<(), A::Error> {
            let output = self.0;
            while map.next_key::<serde::de::IgnoredAny>()?.is_some() {
                map.next_value_seed(Seed(output))?;
            }
            Ok(())
        }
        fn visit_bool<E: serde::de::Error>(self, _value: bool) -> std::result::Result<(), E> {
            Ok(())
        }
        fn visit_i64<E: serde::de::Error>(self, _value: i64) -> std::result::Result<(), E> {
            Ok(())
        }
        fn visit_u64<E: serde::de::Error>(self, _value: u64) -> std::result::Result<(), E> {
            Ok(())
        }
        fn visit_f64<E: serde::de::Error>(self, _value: f64) -> std::result::Result<(), E> {
            Ok(())
        }
        fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<(), E> {
            Ok(())
        }
    }
    let mut found = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let _ = Seed(&mut found).deserialize(&mut deserializer);
    found
}

fn scan(line: &str, br_to_newline: bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end) = after.find('>') else {
            out.push_str(after);
            return out;
        };
        let tag = &after[..=end];
        let name = tag
            .trim_start_matches('<')
            .trim_start_matches('/')
            .split(['/', ' ', '>'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if br_to_newline && name == "br" {
            out.push('\n');
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn strip_html_handles_br_variants_and_tags() {
        assert_eq!(super::strip_html("a<br/>b<br />c<br>d"), "a\nb\nc\nd");
        assert_eq!(super::strip_html("<b>粗体</b>与<color=#d37137>颜色</color>"), "粗体与颜色");
        assert_eq!(super::strip_html("未闭合 <b 标签"), "未闭合 <b 标签");
    }
}
