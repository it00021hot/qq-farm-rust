mod clock;
#[cfg(any(test, feature = "ui-preview"))]
mod fixtures;
#[cfg(any(test, feature = "ui-preview"))]
pub(crate) use fixtures::weather_fixture;
mod friends;
mod overview;
mod research;
mod shared;
mod state;

pub(super) use shared::{flag, number};
pub(crate) use state::WeatherState;

use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn weather_is_compact(&self, window: &Window) -> bool {
        let sidebar = if self.sidebar_collapsed {
            window.rem_size() * 4.
        } else {
            px(self.preferences.sidebar_width as f32)
        };
        window.viewport_size().width - sidebar < window.rem_size() * 64.
    }

    pub(crate) fn render_weather_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let language = self.preferences.language.as_str();
        let Some(snapshot) = self.page_raw.pointer("/weather/snapshot").filter(|v| v.is_object())
        else {
            let refresh = Button::new("weather-refresh")
                .label(crate::i18n::text(language, "刷新", "Refresh"))
                .ghost()
                .loading(self.loading)
                .disabled(self.loading || self.selected_account.is_none())
                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)));
            let panel = shared::panel(cx)
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(div().text_lg().font_semibold().child(crate::i18n::text(
                            language,
                            "雨落成诗",
                            "Poem in the Rain",
                        )))
                        .child(refresh),
                )
                .when_some(self.error.as_ref(), |panel, error| {
                    let error_panel = div()
                        .id("weather-error")
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.clone());
                    #[cfg(test)]
                    let error_panel = error_panel.test_support();
                    panel.child(error_panel)
                })
                .when(self.error.is_none(), |panel| {
                    panel.child(shared::empty(
                        crate::i18n::text(
                            language,
                            "雨落成诗活动数据尚未加载",
                            "Weather activity data has not loaded",
                        ),
                        cx,
                    ))
                });
            return panel;
        };
        let state = &self.activities.weather;
        div()
            .v_flex()
            .gap_4()
            .min_w_0()
            .child(overview::header(self, snapshot, language, cx))
            .child(
                h_flex()
                    .items_stretch()
                    .gap_4()
                    .child(overview::supply(self, snapshot, language, cx).flex_1())
                    .child(overview::tasks(snapshot, language, cx).flex_1()),
            )
            .child(research::render(self, snapshot, language, cx))
            .child(overview::inventory(snapshot, language, cx))
            .child(friends::render(self, snapshot, window, cx))
            .when_some(state.notice.as_ref(), |panel, notice| {
                panel.child(
                    div()
                        .id("weather-notice")
                        .text_sm()
                        .text_color(cx.theme().success)
                        .child(notice.clone()),
                )
            })
            .when_some(state.error.as_ref(), |panel, error| {
                panel.child(
                    div()
                        .id("weather-error")
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.clone()),
                )
            })
            .when(
                snapshot
                    .pointer("/rules/paragraphs")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|v| !v.is_empty()),
                |panel| {
                    panel
                        .child(
                            Button::new("weather-rules-toggle")
                                .label(
                                    shared::text(
                                        snapshot.get("rules").unwrap_or(&serde_json::Value::Null),
                                        "title",
                                    )
                                    .map(|title| crate::i18n::activity_rule_title(&title, language))
                                    .unwrap_or_else(|| {
                                        crate::i18n::text(language, "活动说明", "Activity rules")
                                            .into()
                                    }),
                                )
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.activities.weather.rules_open =
                                        !this.activities.weather.rules_open;
                                    cx.notify();
                                })),
                        )
                        .when(state.rules_open, |panel| panel.child(self.render_activity_rules(cx)))
                },
            )
    }

    pub(in crate::desktop) fn weather_action_available(
        &self,
        action: &str,
        gid: Option<i64>,
    ) -> bool {
        if self.loading || self.selected_account.is_none() {
            return false;
        }
        let snapshot = friends::snapshot_value(&self.page_raw);
        if !snapshot.is_object() {
            return false;
        }
        let friend = gid
            .filter(|gid| *gid > 0)
            .and_then(|gid| {
                friends::base_friends(self)
                    .iter()
                    .find(|friend| number(friend.get("gid")) == Some(gid))
            })
            .map(|friend| friends::merged_friend(self, friend));
        match action {
            "summon" => shared::gate(snapshot, "summonThunderstorm"),
            "exchange" => {
                shared::gate(snapshot, "exchangeCollector")
                    && snapshot.get("shop").is_some_and(|shop| flag(shop, "available"))
            }
            "scan" => friend.is_some() && shared::gate(snapshot, "scanFriendWeather"),
            "collect" => {
                friend.as_ref().is_some_and(|friend| flag(friend, "canCollect"))
                    && shared::gate(snapshot, "collectWeather")
            }
            "frog" => friend.is_some() && shared::gate(snapshot, "frogMischief"),
            "cloud" => {
                friend
                    .as_ref()
                    .is_some_and(|friend| !shared::list(friend, "eligibleCloudLandIds").is_empty())
                    && shared::gate(snapshot, "cloudMischief")
            }
            _ => false,
        }
    }

    pub(in crate::desktop) fn apply_weather_result(
        &mut self,
        action: &str,
        value: &serde_json::Value,
    ) {
        let now = clock::now_seconds();
        if action == "scan" {
            for friend in shared::list(value, "friends") {
                self.activities.weather.merge_friend(friend, now);
            }
            self.activities.weather.deferred_gids =
                shared::list(value, "deferredGids").iter().map(value_to_action_string).collect();
        } else {
            if let Some(friend) = value.get("friend") {
                self.activities.weather.merge_friend(friend, now);
            }
            if let Some(snapshot) = value.get("snapshot").filter(|snapshot| snapshot.is_object()) {
                self.page_raw["weather"]["snapshot"] = snapshot.clone();
                self.activities.weather.now_sec = now;
                self.activities.weather.loaded_at_sec = now;
            }
            self.activities.weather.notice =
                Some(shared::action_notice(action, value, &self.preferences.language));
        }
    }
}
