use super::super::super::game_config::search::{game_config_filters_match, matches_vue_keyword};
use super::*;
use crate::desktop::friend_action_type_number;

fn friend_action_type_matches(value: Option<&serde_json::Value>, expected: i64) -> bool {
    friend_action_type_number(value) == Some(expected as f64)
}

impl FarmDesktop {
    pub(super) fn visible_row_indices(&self, cx: &mut Context<Self>) -> (Vec<usize>, bool) {
        let query = if self.page == Page::GameConfig {
            self.game_config.applied_keyword.trim().to_lowercase()
        } else {
            self.search.read(cx).value().trim().to_lowercase()
        };
        let account_level = self
            .page_raw
            .get("accountLevel")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(1)
            .clamp(1, 200);
        let requested_level = self
            .analytics
            .level
            .read(cx)
            .value()
            .trim()
            .parse::<i64>()
            .unwrap_or(account_level)
            .clamp(1, 200);
        let analytics_level =
            if requested_level == 1 && account_level > 1 { account_level } else { requested_level };
        let rows_use_grid =
            self.page == Page::Mall || (self.page == Page::Personal && self.page_section == "bag");
        let friend_page_start = self.friends.page * 25;
        let game_config_page_start = self.game_config.page * self.game_config.page_size;
        let mut friend_offset = 0usize;
        let mut game_config_offset = 0usize;
        let visible_row_indices = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let matches_page = (self.page_section == "all"
                    || row.section == self.page_section
                    || (self.page == Page::Activities
                        && self.page_section == "qixi"
                        && row.section == "activityFriends"))
                    && (self.page != Page::Activities
                        || self.page_section != "autumn"
                        || row.section != "autumn"
                        || row.id.contains(&format!("/{}/", self.activities.autumn_mode)))
                    && (self.page != Page::Friends
                        || self.page_section != "records"
                        || self.friends.record_filter.is_none_or(|filter| {
                            friend_action_type_matches(row.record.get("actionType"), filter)
                        }))
                    && (self.page != Page::Personal
                        || self.page_section != "bag"
                        || self.farm.bag.category == "all"
                        || row
                            .record
                            .get("itemType")
                            .and_then(serde_json::Value::as_i64)
                            .is_some_and(|item_type| {
                                bag_category_for_type(item_type) == self.farm.bag.category
                            }))
                    && (self.page != Page::Analytics
                        || row.section != "rankings"
                        || row
                            .record
                            .get("level")
                            .and_then(serde_json::Value::as_i64)
                            .is_none_or(|required| required <= analytics_level))
                    && (self.page != Page::GameConfig
                        || game_config_filters_match(
                            row,
                            &self.page_section,
                            self.game_config.applied_seasons,
                            self.game_config.applied_rarity,
                            self.game_config.applied_item_type,
                        ))
                    && !(self.page == Page::Settings
                        && matches!(self.page_section.as_str(), "automation" | "offlineReminder"))
                    && (query.is_empty()
                        || if self.page == Page::Friends
                            && matches!(row.section.as_str(), "friends" | "blacklist")
                        {
                            friend_matches_search(&row.record, &query)
                        } else if self.page == Page::GameConfig {
                            matches_vue_keyword(row, &query)
                        } else {
                            row.title.to_lowercase().contains(&query)
                                || row.detail.to_lowercase().contains(&query)
                                || row.state.to_lowercase().contains(&query)
                                || row.record.to_string().to_lowercase().contains(&query)
                        })
                    && !(self.page == Page::Activities
                        && self.page_section == "weather"
                        && row.id.starts_with("weather/friends/"));
                if !matches_page {
                    return None;
                }
                if self.page == Page::Friends
                    && self.page_section == "friends"
                    && row.section == "friends"
                {
                    let position = friend_offset;
                    friend_offset += 1;
                    if !(friend_page_start..friend_page_start + 25).contains(&position) {
                        return None;
                    }
                }
                if self.page == Page::GameConfig {
                    let position = game_config_offset;
                    game_config_offset += 1;
                    if !(game_config_page_start
                        ..game_config_page_start + self.game_config.page_size)
                        .contains(&position)
                    {
                        return None;
                    }
                }
                Some(index)
            })
            .collect::<Vec<_>>();
        (visible_row_indices, rows_use_grid)
    }
}

#[cfg(test)]
mod tests {
    use super::friend_action_type_matches;

    #[test]
    fn visitor_filter_matches_vue_number_conversion() {
        assert!(friend_action_type_matches(Some(&serde_json::json!(2)), 2));
        assert!(friend_action_type_matches(Some(&serde_json::json!(2.0)), 2));
        assert!(friend_action_type_matches(Some(&serde_json::json!("2")), 2));
        assert!(friend_action_type_matches(Some(&serde_json::json!("2.0")), 2));
        assert!(friend_action_type_matches(Some(&serde_json::json!(" 2 ")), 2));
        assert!(friend_action_type_matches(Some(&serde_json::json!("0x2")), 2));
        assert!(friend_action_type_matches(Some(&serde_json::json!([2])), 2));
        assert!(friend_action_type_matches(Some(&serde_json::json!(["2"])), 2));
        assert!(friend_action_type_matches(Some(&serde_json::json!(true)), 1));
        assert!(!friend_action_type_matches(Some(&serde_json::json!("help")), 2));
        assert!(!friend_action_type_matches(None, 2));
    }
}
