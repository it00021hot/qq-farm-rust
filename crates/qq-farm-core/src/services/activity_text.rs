//! 活动文案工具：剥离活动说明里的 HTML（`<br>` 家族替换为换行，其余标签剥除）。

/// 对齐 bot 的 `replace(/<br\s*(\/\s*)?>/gi, '\n').replace(/<[^>]+>/g, '')`。
pub(crate) fn strip_html(line: &str) -> String {
    scan(line, true)
}

/// 仅剥除标签（对齐 bot 的 `replace(/<[^>]+>/g, '')`，`<br>` 直接移除不留换行）。
pub(crate) fn strip_tags(line: &str) -> String {
    scan(line, false)
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
