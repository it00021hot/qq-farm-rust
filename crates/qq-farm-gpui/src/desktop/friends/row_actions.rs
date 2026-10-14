use super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn append_friend_row_actions(
        &self,
        mut actions: Div,
        row: &DisplayRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.page == Page::Friends {
            let language = &self.preferences.language;
            let gid = row
                .record
                .get("gid")
                .map(|value| super::helpers::friend_value_number(Some(value)))
                .or_else(|| row.record.as_i64())
                .filter(|gid| *gid > 0);
            if let Some(gid) = gid {
                let friend_name = row
                    .record
                    .get("nickname")
                    .and_then(serde_json::Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| gid.to_string());
                let bad_name = friend_name.clone();
                let blacklist_name = friend_name.clone();
                let delete_name = friend_name.clone();
                let unblacklist_name = friend_name;
                if row.id.starts_with("friends/") {
                    let can_steal =
                        super::helpers::friend_number_at(&row.record, "/plant/stealNum") > 0;
                    let can_help = ["dryNum", "weedNum", "insectNum"].iter().any(|field| {
                        super::helpers::friend_number_at(&row.record, &format!("/plant/{field}"))
                            > 0
                    });
                    let steal_loading =
                        self.friends.operation_loading.contains_key(&(gid, "steal"))
                            || self.friends.steal_all_current_gid == Some(gid);
                    let help_loading = self.friends.operation_loading.contains_key(&(gid, "help"));
                    let bad_loading = self.friends.operation_loading.contains_key(&(gid, "bad"));
                    if can_steal {
                        actions = actions.child(
                            Button::new(("friend-steal", index))
                                .label(crate::i18n::text(language, "一键偷取", "One-click steal"))
                                .ghost()
                                .loading(steal_loading)
                                .disabled(steal_loading)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_friend_action(gid, "steal", cx)
                                })),
                        );
                    }
                    if can_help {
                        actions = actions.child(
                            Button::new(("friend-help", index))
                                .label(crate::i18n::text(language, "一键务农", "One-click Help"))
                                .ghost()
                                .loading(help_loading)
                                .disabled(help_loading)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_friend_action(gid, "help", cx)
                                })),
                        );
                    }
                    let bad_trigger = Button::new(("friend-bad", index))
                        .label(crate::i18n::text(language, "捣乱", "Make trouble"))
                        .ghost()
                        .loading(bad_loading)
                        .disabled(bad_loading);
                    let blacklist_trigger = Button::new(("friend-block", index))
                        .label(crate::i18n::text(language, "加入黑名单", "Add to Blacklist"))
                        .ghost()
                        .loading(self.friends.blacklist_loading)
                        .disabled(self.friends.blacklist_loading);
                    let delete_trigger = Button::new(("friend-delete", index))
                        .label(crate::i18n::text(language, "删除好友", "Delete friend"))
                        .ghost()
                        .danger()
                        .loading(self.friends.delete_loading.contains_key(&gid))
                        .disabled(self.friends.delete_loading.contains_key(&gid));
                    actions = actions
                        .child(self.render_friend_confirmation(
                            gid,
                            "bad",
                            bad_name,
                            index,
                            bad_trigger,
                            cx,
                        ))
                        .child(self.render_friend_confirmation(
                            gid,
                            "blacklist",
                            blacklist_name,
                            index,
                            blacklist_trigger,
                            cx,
                        ))
                        .child(self.render_friend_confirmation(
                            gid,
                            "delete",
                            delete_name,
                            index,
                            delete_trigger,
                            cx,
                        ));
                } else if row.id.starts_with("blacklist/") {
                    let unblock_trigger = Button::new(("friend-unblock", index))
                        .label(crate::i18n::text(language, "移出黑名单", "Remove from Blacklist"))
                        .ghost()
                        .danger()
                        .loading(self.friends.blacklist_loading)
                        .disabled(self.friends.blacklist_loading);
                    actions = actions.child(self.render_friend_confirmation(
                        gid,
                        "unblacklist",
                        unblacklist_name,
                        index,
                        unblock_trigger,
                        cx,
                    ));
                }
            }
        }
        actions
    }
}
