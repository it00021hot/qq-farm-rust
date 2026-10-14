use serde_json::Value;

use crate::desktop::activities::qixi_controls::qixi_friend_is_selected;

pub(in crate::desktop::activities::actions) fn prepare(
    page_raw: &Value,
    selected_gid: &str,
    count_input: &str,
    language: &str,
) -> Result<(i64, i64), String> {
    let gid = selected_gid.trim().parse::<i64>().ok();
    let balance = page_raw
        .pointer("/qixi/balances/sachet")
        .and_then(|value| {
            value.as_i64().or_else(|| {
                value
                    .as_f64()
                    .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
                    .filter(|number| number.is_finite())
                    .map(f64::trunc)
                    .map(|number| number as i64)
            })
        })
        .unwrap_or_default();
    let count = count_input
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .map(f64::trunc)
        .map(|number| number as i64);

    match (gid, count) {
        (Some(gid), Some(count)) if gid > 0 && count > 0 && count <= balance => {
            if !qixi_friend_is_selected(page_raw, selected_gid) {
                return Err(crate::i18n::text(
                    language,
                    "请先从当前七夕好友列表选择赠送对象。",
                    "Choose a recipient from the current Magpie Bridge friend list.",
                )
                .into());
            }
            Ok((gid, count))
        }
        _ => Err(if language == "en-US" {
            format!("Check the selected friend and gift quantity (sachets available: {balance}).")
        } else {
            format!("请检查好友选择和赠送数量（当前香囊 {balance} 个）。")
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::prepare;
    use serde_json::json;

    #[test]
    fn gift_requires_a_loaded_friend_and_a_count_within_balance() {
        let raw = json!({
            "qixi": {"balances": {"sachet": "4"}},
            "activityFriends": [{"gid": 123}]
        });

        assert_eq!(prepare(&raw, "123", "4", "zh-CN"), Ok((123, 4)));
        assert_eq!(prepare(&raw, "123", "2.9", "zh-CN"), Ok((123, 2)));
        assert_eq!(
            prepare(&raw, "456", "1", "en-US").unwrap_err(),
            "Choose a recipient from the current Magpie Bridge friend list."
        );
        assert_eq!(
            prepare(&raw, "123", "5", "en-US").unwrap_err(),
            "Check the selected friend and gift quantity (sachets available: 4)."
        );
        assert_eq!(
            prepare(&raw, "123", "0", "zh-CN").unwrap_err(),
            "请检查好友选择和赠送数量（当前香囊 4 个）。"
        );
    }
}
