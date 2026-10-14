use super::*;

pub(super) fn gid(friend: &serde_json::Value) -> String {
    friend.get("gid").map(value_to_action_string).unwrap_or_default()
}

pub(super) fn friend_name(friend: &serde_json::Value, language: &str) -> String {
    text(friend, "name").or_else(|| text(friend, "nickname")).unwrap_or_else(|| {
        format!("{} {}", crate::i18n::text(language, "好友", "Friend"), gid(friend))
    })
}

pub(crate) fn availability_label(friend: &serde_json::Value, language: &str) -> &'static str {
    if text(friend, "scanError").is_some() {
        return crate::i18n::text(language, "读取失败", "Read failed");
    }
    match friend.get("availability").and_then(serde_json::Value::as_str) {
        Some("available") => crate::i18n::text(language, "可采", "Available"),
        Some("collected") => crate::i18n::text(language, "已采", "Collected"),
        Some("expired") => crate::i18n::text(language, "已失效", "Expired"),
        Some("unavailable") => crate::i18n::text(language, "晴天", "Clear"),
        _ => crate::i18n::text(language, "未检查", "Not checked"),
    }
}

pub(crate) fn base_friends(desktop: &FarmDesktop) -> &[serde_json::Value] {
    desktop
        .page_raw
        .pointer("/weather/friends")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

pub(crate) fn merged_friend(desktop: &FarmDesktop, base: &serde_json::Value) -> serde_json::Value {
    let mut friend = base.clone();
    if let Some(scan) = desktop.activities.weather.scanned_friends.get(&gid(base)) {
        if let (Some(friend), Some(scan)) = (friend.as_object_mut(), scan.row.as_object()) {
            friend.extend(scan.iter().map(|(key, value)| (key.clone(), value.clone())));
        }
    }
    friend
}

pub(crate) fn snapshot_value(raw: &serde_json::Value) -> &serde_json::Value {
    raw.pointer("/weather/snapshot").unwrap_or(&serde_json::Value::Null)
}

pub(super) fn avatar(friend: &serde_json::Value, language: &str, cx: &App) -> AnyElement {
    let avatar = friend.get("avatarUrl").and_then(serde_json::Value::as_str).unwrap_or_default();
    if !avatar.is_empty() {
        return render_asset_image(avatar, cx.theme().font_size.as_f32() * 2.4, cx);
    }
    div()
        .size(rems(2.4))
        .rounded_full()
        .bg(cx.theme().muted)
        .flex()
        .items_center()
        .justify_center()
        .child(friend_name(friend, language).chars().next().unwrap_or('友').to_string())
        .into_any_element()
}
