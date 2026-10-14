use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop::activities) fn start_pet_diary_battle(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let current_gid = self.activities.pet_diary_friend_gid.trim().to_string();
        let can_plunder =
            self.page_raw.pointer("/petDiary/hunt/canPlunder").and_then(serde_json::Value::as_bool)
                == Some(true);
        let Some((gid, treasure_id, challenge_id)) =
            self.activities.pet_diary_battle_target.clone()
        else {
            self.error = Some(
                crate::i18n::text(
                    &self.preferences.language,
                    "请先查询好友，再选择可挑战的宝藏和挑战书。",
                    "Search for a friend, then select an available treasure and challenge card.",
                )
                .into(),
            );
            cx.notify();
            return;
        };
        if !can_plunder || current_gid != gid {
            self.error = Some(if !can_plunder {
                crate::i18n::text(
                    &self.preferences.language,
                    "当前没有可用的夺宝次数。",
                    "No plunder attempts are available.",
                )
                .into()
            } else {
                crate::i18n::text(
                    &self.preferences.language,
                    "好友已切换，请重新查询并选择宝藏。",
                    "The friend changed. Search again and select a treasure.",
                )
                .into()
            });
            cx.notify();
            return;
        }
        self.run_pet_diary_action(
            "battle",
            serde_json::json!({
                "gid": gid,
                "treasureId": treasure_id,
                "challengeId": challenge_id,
            }),
            cx,
        );
    }
}
