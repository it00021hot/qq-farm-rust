use nsh_core::prelude::*;
use qq_farm_app::dto::FriendSummary;
use qq_farm_core::models::store::account_config::{
    add_known_friend_gids, get_friends_list_cache_ttl_sec, get_known_friend_gid_sync_cooldown_sec,
    get_known_friend_gids, remove_known_friend_gid, remove_known_friend_gids,
    set_friends_list_cache_ttl_sec, set_known_friend_gid_sync_cooldown_sec,
};
use qq_farm_core::models::store::normalize::{
    normalize_friends_list_cache_ttl_sec, normalize_known_friend_gid_sync_cooldown_sec,
};

use super::confirm::confirm;
use crate::i18n::text_key;
use crate::session;
use crate::ui::{badge, button, field, notice, BadgeTone, ButtonVariant};

const GID_LIMIT: usize = 48;

#[derive(Clone, PartialEq, Eq)]
struct KnownSettings {
    gids: Vec<i64>,
    cooldown: i64,
    cache_ttl: i64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct GidLine {
    gid: i64,
    synced: Option<bool>,
}

struct GidPage {
    lines: Vec<GidLine>,
    hidden: usize,
}

pub(crate) fn section(
    account_id: Signal<Option<String>>,
    friends: Resource<Vec<FriendSummary>, String>,
    refresh_friends: Signal<u64>,
) -> impl IntoElement {
    let pending = signal(false);
    let failure = signal(None::<String>);
    let query = signal(String::new());
    let gid_draft = signal(String::new());
    let cooldown_draft = signal(None::<String>);
    let cache_draft = signal(None::<String>);
    let drafted_for = signal(String::new());
    let settings = resource(
        move || account_id.get().filter(|id| !id.is_empty()),
        |id, _| async move { load_settings(id).await },
    );
    effect(move || {
        let account = account_id.get().unwrap_or_default();
        if drafted_for.get_untracked() != account {
            drafted_for.set(account);
            cooldown_draft.set(None);
            cache_draft.set(None);
            gid_draft.set(String::new());
            query.set(String::new());
        }
    });
    view()
        .gap_3()
        .child(text(move || text_key("friends.qqSync")).text_sm().font_weight(FontWeight::SEMIBOLD))
        .child(text(move || text_key("friends.qqSyncHint")).text_sm())
        .child(crate::ui::failure(move || {
            failure.get().map(SharedString::from).unwrap_or_default()
        }))
        .child(when(
            move || settings.loading() && settings.get().is_none(),
            move || notice(move || text_key("common.loading")),
        ))
        .child(when(
            move || settings.error().is_some(),
            move || {
                view()
                    .gap_2()
                    .child(crate::ui::failure(move || {
                        settings.error().map(SharedString::from).unwrap_or_default()
                    }))
                    .child(
                        button(move || text_key("common.retry"), ButtonVariant::Secondary)
                            .on_press(move || settings.refetch()),
                    )
            },
        ))
        .child(when(
            move || settings.get().is_some(),
            move || {
                known_body(
                    account_id,
                    friends,
                    refresh_friends,
                    settings,
                    pending,
                    failure,
                    query,
                    gid_draft,
                    cooldown_draft,
                    cache_draft,
                )
            },
        ))
}

fn known_body(
    account_id: Signal<Option<String>>,
    friends: Resource<Vec<FriendSummary>, String>,
    refresh_friends: Signal<u64>,
    settings: Resource<KnownSettings, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    query: Signal<String>,
    gid_draft: Signal<String>,
    cooldown_draft: Signal<Option<String>>,
    cache_draft: Signal<Option<String>>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(number_row(
            "friends.syncCooldown",
            move || draft_or_loaded(cooldown_draft, settings, |item| item.cooldown),
            move |text| cooldown_draft.set(Some(text.to_owned())),
        ))
        .child(number_row(
            "friends.listCache",
            move || draft_or_loaded(cache_draft, settings, |item| item.cache_ttl),
            move |text| cache_draft.set(Some(text.to_owned())),
        ))
        .child(
            button(move || text_key("friends.saveSync"), ButtonVariant::Primary)
                .disabled(move || pending.get())
                .on_press(move || {
                    save_timing(
                        account_id,
                        settings,
                        pending,
                        failure,
                        draft_or_loaded(cooldown_draft, settings, |item| item.cooldown),
                        draft_or_loaded(cache_draft, settings, |item| item.cache_ttl),
                    )
                }),
        )
        .child(text(move || text_key("friends.addGids")).text_sm())
        .child(
            view().flex_row().items_center().gap_2().child(
                field(
                    move || gid_draft.get(),
                    move || text_key("friends.gidPlaceholder").to_string(),
                    move |text| gid_draft.set(text.to_owned()),
                )
                .flex_1(),
            ),
        )
        .child(
            button(move || text_key("friends.gidAdd"), ButtonVariant::Secondary)
                .disabled(move || pending.get() || gid_draft.get().trim().is_empty())
                .on_press(move || {
                    add_gids(account_id, settings, refresh_friends, pending, failure, gid_draft)
                }),
        )
        .child(
            field(
                move || query.get(),
                move || text_key("friends.gidSearch").to_string(),
                move |text| query.set(text.to_owned()),
            )
            .flex_1(),
        )
        .child(when(
            move || settings.get().is_some_and(|item| item.gids.is_empty()),
            move || notice(move || text_key("friends.gidEmpty")),
        ))
        .child(
            button(
                move || {
                    let count = unsynced_count(settings, friends);
                    SharedString::from(format!(
                        "{} ({count})",
                        text_key("friends.gidRemoveUnsynced")
                    ))
                },
                ButtonVariant::Danger,
            )
            .disabled(move || pending.get() || unsynced_count(settings, friends) == 0)
            .on_press(move || {
                remove_unsynced(account_id, settings, friends, refresh_friends, pending, failure)
            }),
        )
        .child(each(
            move || {
                visible_gids(
                    &settings.get().map(|item| item.gids).unwrap_or_default(),
                    friend_ids(friends).as_deref(),
                    &query.get(),
                )
                .lines
            },
            |line: &GidLine| line.gid.to_string(),
            move |line, _| {
                render_gid(
                    line.get_untracked(),
                    account_id,
                    settings,
                    refresh_friends,
                    pending,
                    failure,
                )
            },
        ))
        .child(when(
            move || {
                visible_gids(
                    &settings.get().map(|item| item.gids).unwrap_or_default(),
                    friend_ids(friends).as_deref(),
                    &query.get(),
                )
                .hidden
                    > 0
            },
            move || notice(move || text_key("friends.gidMore")),
        ))
}

fn number_row(
    label: &'static str,
    value: impl Fn() -> String + 'static,
    changed: impl Fn(&str) + 'static,
) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(text(move || text_key(label)).text_sm())
        .child(field(move || value(), move || String::new(), changed).w(px(120.0)))
}

fn render_gid(
    line: GidLine,
    account_id: Signal<Option<String>>,
    settings: Resource<KnownSettings, String>,
    refresh_friends: Signal<u64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) -> View {
    let gid = line.gid;
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .py_1()
        .child(text(gid.to_string()).text_sm())
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(when(
                    move || line.synced.is_some(),
                    move || {
                        badge(
                            move || {
                                text_key(if line.synced == Some(true) {
                                    "friends.gidSynced"
                                } else {
                                    "friends.gidUnsynced"
                                })
                            },
                            if line.synced == Some(true) {
                                BadgeTone::Success
                            } else {
                                BadgeTone::Neutral
                            },
                        )
                    },
                ))
                .child(
                    button(move || text_key("friends.gidRemove"), ButtonVariant::Quiet)
                        .disabled(move || pending.get())
                        .on_press(move || {
                            remove_one(account_id, gid, settings, refresh_friends, pending, failure)
                        }),
                ),
        )
}

fn draft_or_loaded(
    draft: Signal<Option<String>>,
    settings: Resource<KnownSettings, String>,
    read: impl Fn(&KnownSettings) -> i64,
) -> String {
    draft
        .get()
        .unwrap_or_else(|| settings.get().map(|item| read(&item).to_string()).unwrap_or_default())
}

fn friend_ids(friends: Resource<Vec<FriendSummary>, String>) -> Option<Vec<i64>> {
    friends.get().map(|rows| rows.iter().map(|friend| friend.gid).filter(|gid| *gid > 0).collect())
}

fn unsynced_count(
    settings: Resource<KnownSettings, String>,
    friends: Resource<Vec<FriendSummary>, String>,
) -> usize {
    let Some(friend_ids) = friend_ids(friends) else {
        return 0;
    };
    settings.get().map(|item| unsynced_ids(&item.gids, &friend_ids).len()).unwrap_or(0)
}

fn visible_gids(known: &[i64], friend_ids: Option<&[i64]>, query: &str) -> GidPage {
    let query = query.trim();
    let matched: Vec<i64> = known
        .iter()
        .copied()
        .filter(|gid| *gid > 0 && (query.is_empty() || gid.to_string().contains(query)))
        .collect();
    let hidden = matched.len().saturating_sub(GID_LIMIT);
    let lines = matched
        .into_iter()
        .take(GID_LIMIT)
        .map(|gid| GidLine { gid, synced: friend_ids.map(|ids| ids.contains(&gid)) })
        .collect();
    GidPage { lines, hidden }
}

fn unsynced_ids(known: &[i64], friend_ids: &[i64]) -> Vec<i64> {
    known.iter().copied().filter(|gid| *gid > 0 && !friend_ids.contains(gid)).collect()
}

fn parse_gids(input: &str) -> Vec<i64> {
    let mut gids = Vec::new();
    for part in input.split(|ch: char| ch == ',' || ch == '，' || ch.is_whitespace()) {
        let Ok(gid) = part.trim().parse::<i64>() else {
            continue;
        };
        if gid > 0 && !gids.contains(&gid) {
            gids.push(gid);
        }
    }
    gids
}

fn timing_seconds(text: &str, loaded: i64, normalize: fn(Option<i64>) -> i64) -> i64 {
    match text.trim().parse::<i64>() {
        Ok(value) => normalize(Some(value)),
        Err(_) => loaded,
    }
}

fn save_timing(
    account_id: Signal<Option<String>>,
    settings: Resource<KnownSettings, String>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    cooldown_text: String,
    cache_text: String,
) {
    let Some(account) = account_id.get().filter(|id| !id.is_empty()) else {
        return;
    };
    let loaded = settings.get();
    let cooldown = timing_seconds(
        &cooldown_text,
        loaded.as_ref().map(|item| item.cooldown).unwrap_or(0),
        normalize_known_friend_gid_sync_cooldown_sec,
    );
    let cache = timing_seconds(
        &cache_text,
        loaded.as_ref().map(|item| item.cache_ttl).unwrap_or(0),
        normalize_friends_list_cache_ttl_sec,
    );
    pending.set(true);
    failure.set(None);
    spawn(async move {
        let saved = session::session()
            .runtime
            .spawn_blocking(move || {
                set_known_friend_gid_sync_cooldown_sec(&account, cooldown);
                set_friends_list_cache_ttl_sec(&account, cache);
            })
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        pending.set(false);
        finish(saved, settings, None, failure);
    });
}

fn add_gids(
    account_id: Signal<Option<String>>,
    settings: Resource<KnownSettings, String>,
    refresh_friends: Signal<u64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
    gid_draft: Signal<String>,
) {
    let Some(account) = account_id.get().filter(|id| !id.is_empty()) else {
        return;
    };
    let gids = parse_gids(&gid_draft.get());
    if gids.is_empty() {
        failure.set(Some(text_key("friends.gidInvalid").to_string()));
        return;
    }
    pending.set(true);
    failure.set(None);
    spawn(async move {
        let saved = session::session()
            .runtime
            .spawn_blocking(move || {
                add_known_friend_gids(&account, &gids);
            })
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        pending.set(false);
        if saved.is_ok() {
            gid_draft.set(String::new());
        }
        finish(saved, settings, Some(refresh_friends), failure);
    });
}

fn remove_one(
    account_id: Signal<Option<String>>,
    gid: i64,
    settings: Resource<KnownSettings, String>,
    refresh_friends: Signal<u64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) {
    let Some(account) = account_id.get().filter(|id| !id.is_empty()) else {
        return;
    };
    confirm(
        "friends.confirmRemoveGid.title",
        "friends.confirmRemoveGid.body",
        "remove-gid",
        "friends.gidRemove",
        move |accepted| {
            if !accepted {
                return;
            }
            pending.set(true);
            failure.set(None);
            spawn(async move {
                let saved = session::session()
                    .runtime
                    .spawn_blocking(move || {
                        remove_known_friend_gid(&account, gid);
                    })
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string());
                pending.set(false);
                finish(saved, settings, Some(refresh_friends), failure);
            });
        },
    );
}

fn remove_unsynced(
    account_id: Signal<Option<String>>,
    settings: Resource<KnownSettings, String>,
    friends: Resource<Vec<FriendSummary>, String>,
    refresh_friends: Signal<u64>,
    pending: Signal<bool>,
    failure: Signal<Option<String>>,
) {
    let Some(account) = account_id.get().filter(|id| !id.is_empty()) else {
        return;
    };
    let Some(friend_ids) = friend_ids(friends) else {
        return;
    };
    let gids = settings.get().map(|item| unsynced_ids(&item.gids, &friend_ids)).unwrap_or_default();
    if gids.is_empty() {
        return;
    }
    confirm(
        "friends.confirmRemoveUnsynced.title",
        "friends.confirmRemoveUnsynced.body",
        "remove-unsynced",
        "friends.gidRemoveUnsynced",
        move |accepted| {
            if !accepted {
                return;
            }
            pending.set(true);
            failure.set(None);
            spawn(async move {
                let saved = session::session()
                    .runtime
                    .spawn_blocking(move || {
                        remove_known_friend_gids(&account, &gids);
                    })
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string());
                pending.set(false);
                finish(saved, settings, Some(refresh_friends), failure);
            });
        },
    );
}

fn finish(
    saved: Result<(), String>,
    settings: Resource<KnownSettings, String>,
    refresh_friends: Option<Signal<u64>>,
    failure: Signal<Option<String>>,
) {
    match saved {
        Ok(()) => {
            settings.refetch();
            if let Some(refresh_friends) = refresh_friends {
                refresh_friends.set(refresh_friends.get_untracked().saturating_add(1));
            }
        }
        Err(error) => failure.set(Some(error)),
    }
}

async fn load_settings(account: String) -> Result<KnownSettings, String> {
    session::session()
        .runtime
        .spawn_blocking(move || KnownSettings {
            gids: get_known_friend_gids(Some(&account)),
            cooldown: get_known_friend_gid_sync_cooldown_sec(Some(&account)),
            cache_ttl: get_friends_list_cache_ttl_sec(Some(&account)),
        })
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_gids, timing_seconds, unsynced_ids, visible_gids};
    use qq_farm_core::models::store::normalize::{
        normalize_friends_list_cache_ttl_sec, normalize_known_friend_gid_sync_cooldown_sec,
    };

    #[test]
    fn gid_text_accepts_separators_and_skips_invalid_values() {
        assert_eq!(parse_gids(" 12, 12，34\n0 99abc 7 "), vec![12, 34, 7]);
        assert!(parse_gids("  ，\n").is_empty());
        assert!(parse_gids("-4").is_empty());
    }

    #[test]
    fn gid_list_marks_sync_hides_the_overflow_and_can_search() {
        let known: Vec<i64> = (1..=50).collect();
        let friends = [2_i64, 4];
        let page = visible_gids(&known, Some(&friends), "");
        assert_eq!(page.lines.len(), 48);
        assert_eq!(page.hidden, 2);
        assert_eq!(page.lines[1].synced, Some(true));
        assert_eq!(page.lines[0].synced, Some(false));
        let searched = visible_gids(&known, None, "49");
        assert_eq!(searched.lines.len(), 1);
        assert_eq!(searched.lines[0].gid, 49);
        assert_eq!(searched.lines[0].synced, None);
        assert_eq!(unsynced_ids(&[2, 4, 9], &friends), vec![9]);
        assert!(unsynced_ids(&[0, 2], &friends).is_empty());
    }

    #[test]
    fn timing_keeps_the_loaded_value_when_the_text_is_not_a_number() {
        assert_eq!(timing_seconds("1", 300, normalize_known_friend_gid_sync_cooldown_sec), 30);
        assert_eq!(timing_seconds("abc", 300, normalize_known_friend_gid_sync_cooldown_sec), 300);
        assert_eq!(timing_seconds("5", 60, normalize_friends_list_cache_ttl_sec), 10);
    }
}
