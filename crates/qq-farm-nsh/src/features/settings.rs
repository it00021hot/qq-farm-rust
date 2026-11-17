use nsh_core::prelude::*;
use serde_json::{json, Value};

use crate::i18n::text_key;
use crate::session;
use crate::ui::{button, notice, ButtonVariant};

const TOGGLES: &[(&str, &str)] = &[
    ("farm", "settings.farm"),
    ("farm_harvest_tiangong", "settings.harvestTiangong"),
    ("farm_push", "settings.farmPush"),
    ("land_upgrade", "settings.landUpgrade"),
    ("friend", "settings.friend"),
    ("friend_steal", "settings.steal"),
    ("friend_help", "settings.help"),
    ("friend_help_exp_limit", "settings.helpExpLimit"),
    ("friend_help_protect_dog_ignore_exp_limit", "settings.helpDog"),
    ("friend_bad", "settings.bad"),
    ("task", "settings.task"),
    ("sell", "settings.sell"),
    ("fertilizer_gift", "settings.fertilizerGift"),
    ("fertilizer_buy_organic", "settings.buyOrganic"),
    ("fertilizer_buy_normal", "settings.buyNormal"),
    ("fertilizer_multi_season", "settings.multiSeason"),
    ("skip_own_weed_bug", "settings.skipWeed"),
    ("show_manual_fertilizer", "settings.manualFertilizer"),
    ("friend_auto_accept", "settings.accept"),
    ("mystery_shop_auto_buy", "settings.mystery"),
    ("mystery_shop_arrival_notify", "settings.mysteryArrival"),
    ("mystery_shop_purchase_notify", "settings.mysteryBought"),
    ("mystery_shop_allow_gold", "settings.mysteryGold"),
    ("mystery_shop_allow_coupon", "settings.mysteryCoupon"),
    ("mystery_shop_allow_gold_bean", "settings.mysteryBean"),
    ("mystery_shop_allow_diamond", "settings.mysteryDiamond"),
    ("ranch_share", "settings.ranchShare"),
    ("ranch_task", "settings.ranchTask"),
    ("ranch_daily_benefit", "settings.ranchBenefit"),
    ("ranch_harvest", "settings.ranchHarvest"),
    ("ranch_produce", "settings.ranchProduce"),
    ("ranch_clean", "settings.ranchClean"),
    ("ranch_sell", "settings.ranchSell"),
    ("ranch_buy", "settings.ranchBuy"),
    ("ranch_upgrade", "settings.ranchUpgrade"),
    ("ranch_grass", "settings.ranchGrass"),
    ("ranch_grass_buy", "settings.ranchGrassBuy"),
    ("ranch_fill_cans", "settings.ranchFillCans"),
    ("ranch_feed_normal", "settings.ranchFeedNormal"),
    ("ranch_feed_organic", "settings.ranchFeedOrganic"),
    ("ranch_friend_steal", "settings.ranchFriendSteal"),
    ("ranch_stroke", "settings.ranchStroke"),
    ("ranch_friend_clean", "settings.ranchFriendClean"),
    ("ranch_friend_mosquito", "settings.ranchFriendMosquito"),
];

const STRATEGIES: &[(&str, &str)] = &[
    ("level", "settings.strategyLevel"),
    ("preferred", "settings.strategyPreferred"),
    ("bag_priority", "settings.strategyBag"),
];

const FALLBACKS: &[(&str, &str)] =
    &[("level", "settings.strategyLevel"), ("preferred", "settings.strategyPreferred")];

const CHECK_MINUTES: i64 = 5;
const CHECK_MIN: i64 = 1;
const CHECK_MAX: i64 = 60;
const GRASS_PERCENT: i64 = 20;
const GRASS_MIN: i64 = 1;
const GRASS_MAX: i64 = 100;

const FERTILIZER_MODES: &[(&str, &str)] = &[
    ("none", "settings.fertNone"),
    ("normal", "settings.fertNormal"),
    ("organic", "settings.fertOrganic"),
    ("both", "settings.fertBoth"),
    ("smart", "settings.fertSmart"),
];

pub(crate) const LAND_TYPES: &[(&str, &str)] = &[
    ("blue-crystal", "settings.landBlue"),
    ("purple-gold", "settings.landPurple"),
    ("gold", "settings.landGold"),
    ("black", "settings.landBlack"),
    ("red", "settings.landRed"),
    ("normal", "settings.landNormal"),
];

#[derive(Clone, Copy)]
pub(crate) struct SettingsPage {
    pub account_id: Signal<Option<String>>,
}

pub(crate) fn page(page: SettingsPage) -> impl IntoElement {
    let pending = signal(None::<String>);
    let failure = signal(None::<String>);
    let settings = resource(
        move || page.account_id.get().filter(|id| !id.is_empty()),
        |id, _| async move {
            session::session()
                .runtime
                .spawn_blocking(move || qq_farm_app::settings::settings_panel(&id, ""))
                .await
                .map_err(|error| error.to_string())
        },
    );
    scroll()
        .flex_1()
        .p(px(24.0))
        .gap_3()
        .child(text(move || text_key("settings.title")).text_lg().font_weight(FontWeight::SEMIBOLD))
        .child(super::preferences::section())
        .child(crate::ui::failure(move || {
            failure.get().map(SharedString::from).unwrap_or_default()
        }))
        .child(when(
            move || page.account_id.get().is_none(),
            move || notice(move || text_key("dashboard.empty")),
        ))
        .child(when(
            move || {
                page.account_id.get().is_some() && settings.loading() && settings.get().is_none()
            },
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
            move || settings_body(page, pending, failure, settings),
        ))
        .child(super::system_runtime::section())
        .child(super::offline_reminder::section())
}

fn settings_body(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> impl IntoElement {
    view()
        .gap_3()
        .child(super::planting::section(page, pending, failure, settings))
        .child(timing(page, pending, failure, settings))
        .child(
            text(move || text_key("settings.fertilizer"))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            || FERTILIZER_MODES.to_vec(),
            |mode: &(&str, &str)| mode.0.to_owned(),
            move |mode, _| {
                let (mode, label) = mode.get_untracked();
                button(move || text_key(label), ButtonVariant::Quiet)
                    .selected(move || {
                        fertilizer_mode(&settings.get().unwrap_or(Value::Null)) == mode
                    })
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        save(
                            page,
                            pending,
                            failure,
                            settings,
                            json!({ "automation": { "fertilizer": mode } }),
                        )
                    })
            },
        )))
        .child(fertilizer_extra(page, pending, failure, settings))
        .child(each(
            || TOGGLES.to_vec(),
            |toggle: &(&str, &str)| toggle.0.to_owned(),
            move |toggle, _| {
                let (key, label) = toggle.get_untracked();
                toggle_row(key, label, page, pending, failure, settings)
            },
        ))
        .child(accept_filters(page, pending, failure, settings))
        .child(email_hour(page, pending, failure, settings))
        .child(ranch_tuning(page, pending, failure, settings))
        .child(toggle_quiet(page, pending, failure, settings))
}

fn fertilizer_extra(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .gap_2()
        .child(when(
            move || fertilizer_mode(&settings.get().unwrap_or(Value::Null)) == "smart",
            move || {
                stepper(
                    "settings.smartSeconds",
                    "fertilizer_smart_seconds",
                    360,
                    30,
                    3600,
                    30,
                    page,
                    pending,
                    failure,
                    settings,
                )
            },
        ))
        .child(when(
            move || {
                automation_flag(&settings.get().unwrap_or(Value::Null), "fertilizer_buy_organic")
            },
            move || {
                view()
                    .gap_2()
                    .child(text(move || text_key("settings.organicBuy")).text_sm())
                    .child(root_stepper(
                        "settings.buyCount",
                        "fertilizerBuyOrganicCount",
                        1,
                        1,
                        10_000,
                        page,
                        pending,
                        failure,
                        settings,
                    ))
                    .child(root_stepper(
                        "settings.buyHours",
                        "fertilizerBuyOrganicThresholdHours",
                        10,
                        1,
                        990,
                        page,
                        pending,
                        failure,
                        settings,
                    ))
            },
        ))
        .child(when(
            move || {
                automation_flag(&settings.get().unwrap_or(Value::Null), "fertilizer_buy_normal")
            },
            move || {
                view()
                    .gap_2()
                    .child(text(move || text_key("settings.normalBuy")).text_sm())
                    .child(root_stepper(
                        "settings.buyCount",
                        "fertilizerBuyNormalCount",
                        1,
                        1,
                        10_000,
                        page,
                        pending,
                        failure,
                        settings,
                    ))
                    .child(root_stepper(
                        "settings.buyHours",
                        "fertilizerBuyNormalThresholdHours",
                        10,
                        1,
                        990,
                        page,
                        pending,
                        failure,
                        settings,
                    ))
            },
        ))
        .child(when(
            move || {
                let value = settings.get().unwrap_or(Value::Null);
                automation_flag(&value, "fertilizer_buy_organic")
                    || automation_flag(&value, "fertilizer_buy_normal")
            },
            move || {
                root_stepper(
                    "settings.buyInterval",
                    "fertilizerBuyCheckIntervalMinutes",
                    60,
                    1,
                    1440,
                    page,
                    pending,
                    failure,
                    settings,
                )
            },
        ))
        .child(text(move || text_key("settings.landScope")).text_sm())
        .child(land_type_row(page, pending, failure, settings))
}

fn land_type_row(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        || LAND_TYPES.to_vec(),
        |item: &(&str, &str)| item.0.to_owned(),
        move |item, _| {
            let (id, label) = item.get_untracked();
            button(move || text_key(label), ButtonVariant::Quiet)
                .selected(move || {
                    selected_land_types(&settings.get().unwrap_or(Value::Null))
                        .iter()
                        .any(|item| item == id)
                })
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    let current = selected_land_types(&settings.get().unwrap_or(Value::Null));
                    let next = toggled_land_types(&current, id);
                    save(
                        page,
                        pending,
                        failure,
                        settings,
                        json!({ "automation": { "fertilizer_land_types": next } }),
                    )
                })
        },
    ))
}

fn toggle_row(
    key: &'static str,
    label: &'static str,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .py_2()
        .child(text(move || text_key(label)).text_sm().flex_1())
        .child(
            button(
                move || {
                    text_key(if automation_flag(&settings.get().unwrap_or(Value::Null), key) {
                        "settings.on"
                    } else {
                        "settings.off"
                    })
                },
                ButtonVariant::Secondary,
            )
            .selected(move || automation_flag(&settings.get().unwrap_or(Value::Null), key))
            .disabled(move || {
                pending.get().is_some()
                    || toggle_locked(key, &settings.get().unwrap_or(Value::Null))
            })
            .on_press(move || {
                let current = settings.get().unwrap_or(Value::Null);
                if toggle_locked(key, &current) {
                    return;
                }
                let next = !automation_flag(&current, key);
                save(page, pending, failure, settings, json!({ "automation": { key: next } }))
            }),
        )
}

fn timing(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .gap_2()
        .child(
            text(move || text_key("settings.intervals"))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(interval_stepper(
            "settings.farmInterval",
            "farm",
            2,
            1,
            600,
            page,
            pending,
            failure,
            settings,
        ))
        .child(interval_stepper(
            "settings.farmMin",
            "farmMin",
            20,
            1,
            600,
            page,
            pending,
            failure,
            settings,
        ))
        .child(interval_stepper(
            "settings.farmMax",
            "farmMax",
            25,
            1,
            600,
            page,
            pending,
            failure,
            settings,
        ))
        .child(interval_stepper(
            "settings.helpMin",
            "helpMin",
            20,
            1,
            600,
            page,
            pending,
            failure,
            settings,
        ))
        .child(interval_stepper(
            "settings.helpMax",
            "helpMax",
            25,
            1,
            600,
            page,
            pending,
            failure,
            settings,
        ))
        .child(interval_stepper(
            "settings.stealMin",
            "stealMin",
            60,
            1,
            600,
            page,
            pending,
            failure,
            settings,
        ))
        .child(interval_stepper(
            "settings.stealMax",
            "stealMax",
            90,
            1,
            600,
            page,
            pending,
            failure,
            settings,
        ))
        .child(root_stepper(
            "settings.plantDelay",
            "plantDelaySeconds",
            2,
            0,
            60,
            page,
            pending,
            failure,
            settings,
        ))
        .child(root_stepper(
            "settings.stealDelay",
            "stealDelaySeconds",
            1,
            0,
            300,
            page,
            pending,
            failure,
            settings,
        ))
        .child(order_row(page, pending, failure, settings))
}

fn interval_stepper(
    label_key: &'static str,
    key: &'static str,
    default: i64,
    min: i64,
    max: i64,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    let current = move || {
        settings.get().as_ref().map(|value| interval_number(value, key, default)).unwrap_or(default)
    };
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(text(move || text_key(label_key)).text_sm().flex_1())
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || current() <= min)
                .on_press(move || {
                    let next = (current() - 1).clamp(min, max);
                    save(page, pending, failure, settings, json!({ "intervals": { key: next } }));
                }),
        )
        .child(text(move || SharedString::from(current().to_string())).text_sm())
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || current() >= max)
                .on_press(move || {
                    let next = (current() + 1).clamp(min, max);
                    save(page, pending, failure, settings, json!({ "intervals": { key: next } }));
                }),
        )
}

fn order_row(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .py_2()
        .child(text(move || text_key("settings.plantOrder")).text_sm().flex_1())
        .child(
            button(
                move || {
                    text_key(if order_random(&settings.get().unwrap_or(Value::Null)) {
                        "settings.on"
                    } else {
                        "settings.off"
                    })
                },
                ButtonVariant::Secondary,
            )
            .selected(move || order_random(&settings.get().unwrap_or(Value::Null)))
            .disabled(move || pending.get().is_some())
            .on_press(move || {
                let next = !order_random(&settings.get().unwrap_or(Value::Null));
                save(page, pending, failure, settings, json!({ "plantOrderRandom": next }));
            }),
        )
}

fn accept_filters(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> impl IntoElement {
    view().gap_2().child(when(
        move || automation_flag(&settings.get().unwrap_or(Value::Null), "friend_auto_accept"),
        move || {
            view()
                .gap_2()
                .child(text(move || text_key("settings.acceptHint")).text_sm())
                .child(root_stepper(
                    "settings.acceptMin",
                    "autoAcceptFriendMinLevel",
                    0,
                    0,
                    200,
                    page,
                    pending,
                    failure,
                    settings,
                ))
                .child(text(move || text_key("settings.acceptUnlimited")).text_sm())
                .child(root_switch(
                    "settings.acceptOwn",
                    "autoAcceptRequireOwnLevel",
                    false,
                    page,
                    pending,
                    failure,
                    settings,
                ))
                .child(root_switch(
                    "settings.acceptRatio",
                    "autoAcceptHarvestStealEnabled",
                    true,
                    page,
                    pending,
                    failure,
                    settings,
                ))
                .child(when(
                    move || {
                        root_flag(
                            &settings.get().unwrap_or(Value::Null),
                            "autoAcceptHarvestStealEnabled",
                            true,
                        )
                    },
                    move || {
                        view()
                            .gap_2()
                            .child(root_stepper(
                                "settings.acceptHarvest",
                                "autoAcceptHarvestStealHarvest",
                                8,
                                0,
                                9999,
                                page,
                                pending,
                                failure,
                                settings,
                            ))
                            .child(root_stepper(
                                "settings.acceptSteal",
                                "autoAcceptHarvestStealSteal",
                                1,
                                1,
                                9999,
                                page,
                                pending,
                                failure,
                                settings,
                            ))
                    },
                ))
        },
    ))
}

fn root_switch(
    label: &'static str,
    key: &'static str,
    default: bool,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .py_2()
        .child(text(move || text_key(label)).text_sm().flex_1())
        .child(
            button(
                move || {
                    let on = root_flag(&settings.get().unwrap_or(Value::Null), key, default);
                    text_key(if on { "settings.on" } else { "settings.off" })
                },
                ButtonVariant::Secondary,
            )
            .selected(move || root_flag(&settings.get().unwrap_or(Value::Null), key, default))
            .disabled(move || pending.get().is_some())
            .on_press(move || {
                let next = !root_flag(&settings.get().unwrap_or(Value::Null), key, default);
                save(page, pending, failure, settings, json!({ key: next }));
            }),
        )
}

fn email_hour(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .gap_2()
        .child(
            text(move || text_key("settings.emailHour"))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(root_stepper(
            "settings.emailHour",
            "emailClaimHour",
            10,
            0,
            23,
            page,
            pending,
            failure,
            settings,
        ))
}

fn root_stepper(
    label_key: &'static str,
    key: &'static str,
    default: i64,
    min: i64,
    max: i64,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    let current = move || {
        settings.get().and_then(|value| value.get(key).and_then(Value::as_i64)).unwrap_or(default)
    };
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(text(move || text_key(label_key)).text_sm())
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || current() <= min)
                .on_press(move || {
                    let next = (current() - 1).clamp(min, max);
                    save(page, pending, failure, settings, json!({ key: next }));
                }),
        )
        .child(text(move || SharedString::from(current().to_string())).text_sm())
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || current() >= max)
                .on_press(move || {
                    let next = (current() + 1).clamp(min, max);
                    save(page, pending, failure, settings, json!({ key: next }));
                }),
        )
}

fn ranch_tuning(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .gap_3()
        .child(
            text(move || text_key("settings.ranchTune"))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(stepper(
            "settings.ranchMinutes",
            "ranch_check_minutes",
            CHECK_MINUTES,
            CHECK_MIN,
            CHECK_MAX,
            1,
            page,
            pending,
            failure,
            settings,
        ))
        .child(stepper(
            "settings.ranchGrassPercent",
            "ranch_grass_threshold_percent",
            GRASS_PERCENT,
            GRASS_MIN,
            GRASS_MAX,
            1,
            page,
            pending,
            failure,
            settings,
        ))
        .child(text(move || text_key("settings.ranchStrategy")).text_sm())
        .child(choice_row(STRATEGIES, "ranch_strategy", "level", page, pending, failure, settings))
        .child(when(
            move || strategy_of(&settings.get().unwrap_or(Value::Null)) == "bag_priority",
            move || {
                view()
                    .gap_2()
                    .child(text(move || text_key("settings.ranchFallback")).text_sm())
                    .child(choice_row(
                        FALLBACKS,
                        "ranch_bag_fallback",
                        "level",
                        page,
                        pending,
                        failure,
                        settings,
                    ))
            },
        ))
        .child(when(
            move || wants_named_animal(&settings.get().unwrap_or(Value::Null)),
            move || animal_picker(true, page, pending, failure, settings),
        ))
        .child(when(
            move || wants_named_animal(&settings.get().unwrap_or(Value::Null)),
            move || animal_picker(false, page, pending, failure, settings),
        ))
}

fn stepper(
    label: &'static str,
    key: &'static str,
    default: i64,
    min: i64,
    max: i64,
    step: i64,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .child(text(move || text_key(label)).text_sm().flex_1())
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Secondary)
                .accessibility_label(move || Some(text_key(label)))
                .disabled(move || {
                    pending.get().is_some()
                        || automation_number(&settings.get().unwrap_or(Value::Null), key, default)
                            <= min
                })
                .on_press(move || {
                    let current =
                        automation_number(&settings.get().unwrap_or(Value::Null), key, default);
                    save_number(
                        page,
                        pending,
                        failure,
                        settings,
                        key,
                        stepped(current, -step, min, max),
                    )
                }),
        )
        .child(
            text(move || {
                SharedString::from(
                    automation_number(&settings.get().unwrap_or(Value::Null), key, default)
                        .to_string(),
                )
            })
            .text_sm(),
        )
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Secondary)
                .accessibility_label(move || Some(text_key(label)))
                .disabled(move || {
                    pending.get().is_some()
                        || automation_number(&settings.get().unwrap_or(Value::Null), key, default)
                            >= max
                })
                .on_press(move || {
                    let current =
                        automation_number(&settings.get().unwrap_or(Value::Null), key, default);
                    save_number(
                        page,
                        pending,
                        failure,
                        settings,
                        key,
                        stepped(current, step, min, max),
                    )
                }),
        )
}

fn choice_row(
    choices: &'static [(&str, &str)],
    key: &'static str,
    default: &'static str,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view().flex_row().flex_wrap().gap_2().child(each(
        move || choices.to_vec(),
        |choice: &(&str, &str)| choice.0.to_owned(),
        move |choice, _| {
            let (value, label) = choice.get_untracked();
            button(move || text_key(label), ButtonVariant::Quiet)
                .selected(move || {
                    automation_text(&settings.get().unwrap_or(Value::Null), key, default) == value
                })
                .disabled(move || pending.get().is_some())
                .on_press(move || {
                    save(page, pending, failure, settings, json!({ "automation": { key: value } }))
                })
        },
    ))
}

fn animal_picker(
    nest: bool,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    let key = if nest { "ranch_coop_animal" } else { "ranch_shed_animal" };
    let title = if nest { "settings.ranchCoop" } else { "settings.ranchShed" };
    view()
        .gap_2()
        .child(text(move || text_key(title)).text_sm())
        .child(
            view().flex_row().flex_wrap().gap_2().child(
                button(move || text_key("settings.ranchNone"), ButtonVariant::Quiet)
                    .selected(move || {
                        automation_number(&settings.get().unwrap_or(Value::Null), key, 0) == 0
                    })
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        save(page, pending, failure, settings, json!({ "automation": { key: 0 } }))
                    }),
            ),
        )
        .child(view().flex_row().flex_wrap().gap_2().child(each(
            move || animal_rows(&settings.get().unwrap_or(Value::Null), nest),
            |animal: &AnimalChoice| animal.id,
            move |animal, _| {
                let choice = animal.get_untracked();
                let id = choice.id;
                let name = choice.name.clone();
                button(move || SharedString::from(name.clone()), ButtonVariant::Quiet)
                    .selected(move || {
                        automation_number(&settings.get().unwrap_or(Value::Null), key, 0) == id
                    })
                    .disabled(move || pending.get().is_some())
                    .on_press(move || {
                        save(page, pending, failure, settings, json!({ "automation": { key: id } }))
                    })
            },
        )))
}

fn save_number(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
    key: &'static str,
    value: i64,
) {
    save(page, pending, failure, settings, json!({ "automation": { key: value } }));
}

fn toggle_quiet(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .py_2()
                .child(text(move || text_key("settings.quiet")).text_sm().flex_1())
                .child(quiet_switch(
                    "enabled",
                    move || quiet_enabled(&settings.get().unwrap_or(Value::Null)),
                    page,
                    pending,
                    failure,
                    settings,
                )),
        )
        .child(quiet_clock("settings.quietStart", "start", 1, 0, page, pending, failure, settings))
        .child(quiet_clock("settings.quietEnd", "end", 8, 30, page, pending, failure, settings))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .py_2()
                .child(text(move || text_key("settings.quietContinue")).text_sm().flex_1())
                .child(quiet_switch(
                    "continueFarm",
                    move || quiet_continue(&settings.get().unwrap_or(Value::Null)),
                    page,
                    pending,
                    failure,
                    settings,
                )),
        )
}

fn quiet_switch(
    key: &'static str,
    current: impl Fn() -> bool + Clone + 'static,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> nsh_base::ButtonElement {
    let current_label = current.clone();
    let current_selected = current.clone();
    button(
        move || text_key(if current_label() { "settings.on" } else { "settings.off" }),
        ButtonVariant::Secondary,
    )
    .selected(move || current_selected())
    .disabled(move || pending.get().is_some())
    .on_press(move || {
        let next = !current();
        save(page, pending, failure, settings, json!({ "friendQuietHours": { key: next } }));
    })
}

fn quiet_clock(
    label_key: &'static str,
    field: &'static str,
    default_hour: i64,
    default_minute: i64,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(text(move || text_key(label_key)).text_sm().flex_1())
        .child(clock_step(
            field,
            true,
            default_hour,
            default_minute,
            0,
            23,
            page,
            pending,
            failure,
            settings,
        ))
        .child(text(move || SharedString::from(":")).text_sm())
        .child(clock_step(
            field,
            false,
            default_hour,
            default_minute,
            0,
            59,
            page,
            pending,
            failure,
            settings,
        ))
}

fn clock_step(
    field: &'static str,
    hour: bool,
    default_hour: i64,
    default_minute: i64,
    min: i64,
    max: i64,
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
) -> View {
    let current = move || {
        let (clock_hour, clock_minute) = settings
            .get()
            .as_ref()
            .map(|value| quiet_hm(value, field, default_hour, default_minute))
            .unwrap_or((default_hour, default_minute));
        if hour {
            clock_hour
        } else {
            clock_minute
        }
    };
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            button(move || SharedString::from("−"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || current() <= min)
                .on_press(move || {
                    let (clock_hour, clock_minute) = settings
                        .get()
                        .as_ref()
                        .map(|value| quiet_hm(value, field, default_hour, default_minute))
                        .unwrap_or((default_hour, default_minute));
                    let next = if hour {
                        (clock_hour - 1).clamp(min, max)
                    } else {
                        (clock_minute - 1).clamp(min, max)
                    };
                    let text = if hour {
                        format!("{next:02}:{clock_minute:02}")
                    } else {
                        format!("{clock_hour:02}:{next:02}")
                    };
                    save(
                        page,
                        pending,
                        failure,
                        settings,
                        json!({ "friendQuietHours": { field: text } }),
                    );
                }),
        )
        .child(text(move || SharedString::from(format!("{:02}", current()))).text_sm())
        .child(
            button(move || SharedString::from("+"), ButtonVariant::Secondary)
                .disabled(move || pending.get().is_some() || current() >= max)
                .on_press(move || {
                    let (clock_hour, clock_minute) = settings
                        .get()
                        .as_ref()
                        .map(|value| quiet_hm(value, field, default_hour, default_minute))
                        .unwrap_or((default_hour, default_minute));
                    let next = if hour {
                        (clock_hour + 1).clamp(min, max)
                    } else {
                        (clock_minute + 1).clamp(min, max)
                    };
                    let text = if hour {
                        format!("{next:02}:{clock_minute:02}")
                    } else {
                        format!("{clock_hour:02}:{next:02}")
                    };
                    save(
                        page,
                        pending,
                        failure,
                        settings,
                        json!({ "friendQuietHours": { field: text } }),
                    );
                }),
        )
}

pub(crate) fn save(
    page: SettingsPage,
    pending: Signal<Option<String>>,
    failure: Signal<Option<String>>,
    settings: Resource<Value, String>,
    patch: Value,
) {
    if pending.get_untracked().is_some() {
        return;
    }
    let Some(id) = page.account_id.get_untracked() else {
        return;
    };
    pending.set(Some(id.clone()));
    failure.set(None);
    let context = session::session().context.clone();
    spawn(async move {
        let result = session::session()
            .runtime
            .spawn_blocking(move || qq_farm_app::settings::save_settings(&context, &id, patch))
            .await
            .map_err(|error| error.to_string())
            .and_then(|saved| saved.map_err(|error| error.to_string()));
        if let Err(message) = result {
            failure.set(Some(message));
        }
        pending.set(None);
        settings.refetch();
    });
}

#[derive(Clone, PartialEq, Eq)]
struct AnimalChoice {
    id: i64,
    name: String,
}

fn root_flag(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn toggle_locked(key: &str, value: &Value) -> bool {
    match key {
        "farm_push" => !automation_flag(value, "farm"),
        "mystery_shop_purchase_notify" => !automation_flag(value, "mystery_shop_auto_buy"),
        _ => false,
    }
}

fn automation_flag(value: &Value, key: &str) -> bool {
    value.pointer(&format!("/automation/{key}")).and_then(Value::as_bool).unwrap_or(false)
}

fn automation_number(value: &Value, key: &str, default: i64) -> i64 {
    value.pointer(&format!("/automation/{key}")).and_then(Value::as_i64).unwrap_or(default)
}

fn automation_text(value: &Value, key: &str, default: &str) -> String {
    value
        .pointer(&format!("/automation/{key}"))
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn strategy_of(value: &Value) -> String {
    automation_text(value, "ranch_strategy", "level")
}

fn wants_named_animal(value: &Value) -> bool {
    let strategy = strategy_of(value);
    let fallback = automation_text(value, "ranch_bag_fallback", "level");
    strategy == "preferred" || (strategy == "bag_priority" && fallback == "preferred")
}

fn stepped(current: i64, delta: i64, min: i64, max: i64) -> i64 {
    current.saturating_add(delta).clamp(min, max)
}

fn animal_rows(value: &Value, nest: bool) -> Vec<AnimalChoice> {
    value
        .get("ranchAnimals")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let id = row.get("id")?.as_i64()?;
                    let name = row.get("name")?.as_str()?.to_owned();
                    let is_nest = row.get("nest")?.as_bool()?;
                    (is_nest == nest).then_some(AnimalChoice { id, name })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn fertilizer_mode(value: &Value) -> String {
    value.pointer("/automation/fertilizer").and_then(Value::as_str).unwrap_or("none").to_owned()
}

fn selected_land_types(value: &Value) -> Vec<String> {
    match value.pointer("/automation/fertilizer_land_types").and_then(Value::as_array) {
        Some(rows) => LAND_TYPES
            .iter()
            .filter(|(id, _)| rows.iter().any(|item| item.as_str() == Some(*id)))
            .map(|(id, _)| (*id).to_owned())
            .collect(),
        None => LAND_TYPES
            .iter()
            .filter(|(id, _)| *id != "blue-crystal")
            .map(|(id, _)| (*id).to_owned())
            .collect(),
    }
}

fn toggled_land_types(current: &[String], id: &str) -> Vec<String> {
    LAND_TYPES
        .iter()
        .filter(|(item, _)| {
            let selected = current.iter().any(|have| have == item);
            if *item == id {
                !selected
            } else {
                selected
            }
        })
        .map(|(item, _)| (*item).to_owned())
        .collect()
}

fn quiet_enabled(value: &Value) -> bool {
    value.pointer("/friendQuietHours/enabled").and_then(Value::as_bool).unwrap_or(false)
}

fn quiet_continue(value: &Value) -> bool {
    value
        .pointer("/friendQuietHours/continueFarm")
        .or_else(|| value.pointer("/friendQuietHours/continue_farm"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn quiet_hm(value: &Value, field: &str, default_hour: i64, default_minute: i64) -> (i64, i64) {
    let text =
        value.pointer(&format!("/friendQuietHours/{field}")).and_then(Value::as_str).unwrap_or("");
    let Some((hour, minute)) = text.split_once(':') else {
        return (default_hour, default_minute);
    };
    let hour =
        hour.parse::<i64>().ok().filter(|hour| (0..24).contains(hour)).unwrap_or(default_hour);
    let minute = minute
        .parse::<i64>()
        .ok()
        .filter(|minute| (0..60).contains(minute))
        .unwrap_or(default_minute);
    (hour, minute)
}

fn interval_number(value: &Value, key: &str, default: i64) -> i64 {
    value.pointer(&format!("/intervals/{key}")).and_then(Value::as_i64).unwrap_or(default)
}

fn order_random(value: &Value) -> bool {
    value.get("plantOrderRandom").and_then(Value::as_bool).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        animal_rows, automation_flag, automation_number, fertilizer_mode, interval_number,
        order_random, quiet_continue, quiet_enabled, quiet_hm, root_flag, selected_land_types,
        stepped, toggled_land_types, wants_named_animal,
    };

    #[test]
    fn toggles_read_the_automation_object() {
        let value = json!({
            "automation": { "farm": true, "friend_steal": false, "fertilizer": "smart" },
            "friendQuietHours": { "enabled": true },
            "intervals": { "farm": 30, "helpMin": 10, "helpMax": 20, "stealMin": 5, "stealMax": 15 }
        });
        assert!(automation_flag(&value, "farm"));
        assert!(super::TOGGLES.iter().any(|(key, _)| *key == "farm_harvest_tiangong"));
        assert!(super::TOGGLES.iter().any(|(key, _)| *key == "farm_push"));
        assert!(super::TOGGLES.iter().any(|(key, _)| *key == "mystery_shop_allow_diamond"));
        assert!(super::TOGGLES
            .iter()
            .any(|(key, _)| *key == "friend_help_protect_dog_ignore_exp_limit"));
        assert!(!super::toggle_locked("farm_push", &value));
        assert!(super::toggle_locked("farm_push", &json!({ "automation": {} })));
        assert!(super::toggle_locked("mystery_shop_purchase_notify", &json!({ "automation": {} })));
        assert!(!super::toggle_locked(
            "mystery_shop_purchase_notify",
            &json!({ "automation": { "mystery_shop_auto_buy": true } })
        ));
        assert!(!automation_flag(&value, "friend_steal"));
        assert!(!automation_flag(&value, "missing"));
        assert_eq!(fertilizer_mode(&value), "smart");
        assert_eq!(
            selected_land_types(&value),
            vec!["purple-gold", "gold", "black", "red", "normal"]
        );
        let lands = json!({
            "automation": { "fertilizer_land_types": ["blue-crystal", "normal", "gold"] }
        });
        assert_eq!(selected_land_types(&lands), vec!["blue-crystal", "gold", "normal"]);
        assert_eq!(
            toggled_land_types(&selected_land_types(&lands), "gold"),
            vec!["blue-crystal", "normal"]
        );
        assert!(quiet_enabled(&value));
        assert!(quiet_continue(&value));
        assert_eq!(quiet_hm(&value, "start", 1, 0), (1, 0));
        let hours = json!({
            "friendQuietHours": { "start": "01:15", "end": "08:30", "continue_farm": false }
        });
        assert_eq!(quiet_hm(&hours, "start", 1, 0), (1, 15));
        assert_eq!(quiet_hm(&hours, "end", 8, 30), (8, 30));
        assert!(!quiet_continue(&hours));
        assert!(quiet_continue(&json!({ "friendQuietHours": { "continueFarm": true } })));
        assert_eq!(interval_number(&value, "farm", 2), 30);
        assert_eq!(interval_number(&value, "missing", 2), 2);
        assert!(!order_random(&value));
        assert!(root_flag(&json!({}), "autoAcceptHarvestStealEnabled", true));
        assert!(!root_flag(
            &json!({ "autoAcceptHarvestStealEnabled": false }),
            "autoAcceptHarvestStealEnabled",
            true
        ));
        assert!(root_flag(
            &json!({ "autoAcceptRequireOwnLevel": true }),
            "autoAcceptRequireOwnLevel",
            false
        ));
        let ranch = json!({
            "automation": {
                "ranch_check_minutes": 5,
                "ranch_strategy": "bag_priority",
                "ranch_bag_fallback": "preferred"
            },
            "ranchAnimals": [
                { "id": 60001, "name": "鸡", "nest": true },
                { "id": 60011, "name": "牛", "nest": false }
            ]
        });
        assert_eq!(automation_number(&ranch, "ranch_check_minutes", 5), 5);
        assert!(wants_named_animal(&ranch));
        assert_eq!(animal_rows(&ranch, true).len(), 1);
        assert_eq!(animal_rows(&ranch, false)[0].id, 60011);
        assert!(!wants_named_animal(&json!({ "automation": { "ranch_strategy": "level" } })));
        assert_eq!(stepped(60, 1, 1, 60), 60);
        assert_eq!(stepped(1, -1, 1, 60), 1);
        assert_eq!(stepped(5, 1, 1, 60), 6);
    }
}
