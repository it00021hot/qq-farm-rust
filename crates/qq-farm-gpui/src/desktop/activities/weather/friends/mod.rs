use super::super::super::*;
use super::shared::{empty, flag, gate, list, muted, number, panel, text};

mod detail;
mod list;
mod model;

use detail::render_detail as detail;
use list::render_friend_list as friend_list;
pub(super) use model::{availability_label, base_friends, merged_friend, snapshot_value};
use model::{avatar, friend_name, gid};

const FRIEND_SHOW_LIMIT: usize = 60;

pub(super) fn render(
    desktop: &FarmDesktop,
    snapshot: &serde_json::Value,
    window: &mut Window,
    cx: &mut Context<FarmDesktop>,
) -> Div {
    let language = desktop.preferences.language.as_str();
    let state = &desktop.activities.weather;
    let selected = base_friends(desktop)
        .iter()
        .find(|friend| gid(friend) == state.selected_gid)
        .map(|friend| merged_friend(desktop, friend));
    let compact = desktop.weather_is_compact(window);
    let mut view = panel(cx).child(
        h_flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(div().font_medium().child(crate::i18n::text(
                language,
                "好友天气采集",
                "Friends' weather",
            )))
            .child(muted(
                if language == "en-US" {
                    format!(
                        "Daily limit: {}",
                        snapshot
                            .pointer("/actions/collectWeather/dailyLimit")
                            .map(value_to_action_string)
                            .unwrap_or_else(|| "--".into())
                    )
                } else {
                    format!(
                        "每日上限 {} 次",
                        snapshot
                            .pointer("/actions/collectWeather/dailyLimit")
                            .map(value_to_action_string)
                            .unwrap_or_else(|| "--".into())
                    )
                },
                cx,
            )),
    );
    if !gate(snapshot, "scanFriendWeather") {
        view = view.child(muted(
            text(
                snapshot.pointer("/actions/scanFriendWeather").unwrap_or(&serde_json::Value::Null),
                "reason",
            )
            .map(|reason| crate::i18n::weather_reason(&reason, language))
            .unwrap_or_else(|| {
                crate::i18n::text(language, "暂不可扫描", "Scanning is unavailable").into()
            }),
            cx,
        ));
    }
    if compact && selected.is_some() {
        view = view
            .child(
                Button::new("weather-friend-back")
                    .label(crate::i18n::text(language, "返回好友列表", "Back to friends"))
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.activities.weather.selected_gid.clear();
                        this.activities.weather.search.read(cx).focus_handle(cx).focus(window, cx);
                        cx.notify();
                    })),
            )
            .child(detail(desktop, snapshot, selected.as_ref(), language, cx));
    } else {
        let list = friend_list(desktop, language, cx);
        if compact {
            view = view.child(list);
        } else {
            view =
                view.child(h_flex().items_start().gap_4().child(list.flex_1()).child(
                    detail(desktop, snapshot, selected.as_ref(), language, cx).w(rems(22.)),
                ));
        }
    }
    view.when(!state.deferred_gids.is_empty(), |view| {
        view.child(div().text_sm().text_color(cx.theme().warning).child(if language == "en-US" {
            format!(
                "{} friends will be retried later (friend scan is busy)",
                state.deferred_gids.len()
            )
        } else {
            format!("有 {} 位好友稍后重试（好友巡查忙）", state.deferred_gids.len())
        }))
    })
}
