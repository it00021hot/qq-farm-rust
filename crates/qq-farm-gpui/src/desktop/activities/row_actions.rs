use super::super::*;

mod autumn;
mod charity;
mod constellation;
mod directory;
mod friends;
mod pet_diary;
mod pet_diary_friend;
mod qingmei;
mod solar_terms;
mod star_sand;

impl FarmDesktop {
    pub(in crate::desktop) fn append_activity_row_actions(
        &self,
        actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page != Page::Activities {
            return actions;
        }
        match row.section.as_str() {
            "activityFriends" => self.append_activity_friend_action(actions, row, index, cx),
            "petDiary" => self.append_pet_diary_action(actions, row, index, cx),
            "petDiaryFriend" => self.append_pet_diary_friend_action(actions, row, index, cx),
            "directory" => self.append_activity_directory_action(actions, row, index, cx),
            "qingmei" => self.append_qingmei_action(actions, row, index, cx),
            "autumn" => self.append_autumn_action(actions, row, index, cx),
            "solarTerms" => self.append_solar_term_action(actions, row, index, cx),
            "shop" => self.append_star_sand_action(actions, row, index, cx),
            "charity" => self.append_charity_action(actions, row, index, cx),
            "constellation" => self.append_constellation_action(actions, row, cx),
            _ => actions,
        }
    }
}
