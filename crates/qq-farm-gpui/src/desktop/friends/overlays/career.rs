pub(super) fn format_career_count(value: i64, english: bool) -> String {
    if value < 10_000 {
        return value.to_string();
    }
    if english {
        let mut thousands = format!("{:.2}", value as f64 / 1_000.);
        while thousands.ends_with('0') {
            thousands.pop();
        }
        if thousands.ends_with('.') {
            thousands.pop();
        }
        return format!("{thousands}k");
    }
    let mut value = format!("{:.2}", value as f64 / 10_000.);
    while value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    format!("{value}万")
}

#[cfg(test)]
mod tests {
    use super::format_career_count;

    #[test]
    fn career_counts_follow_vue_compact_format() {
        assert_eq!(format_career_count(9_999, false), "9999");
        assert_eq!(format_career_count(10_000, false), "1万");
        assert_eq!(format_career_count(12_340, false), "1.23万");
        assert_eq!(format_career_count(12_340, true), "12.34k");
    }
}
