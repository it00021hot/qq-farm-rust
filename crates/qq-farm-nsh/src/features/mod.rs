mod account_gate;
mod account_toggle;
mod accounts;
mod activities;
mod analytics;
mod bag;
mod collections;
mod confirm;
mod dashboard;
mod farm;
mod friends;
mod game_config;
mod gifts;
mod illustrated;
mod interaction_items;
mod known_friends;
mod mall;
mod muge;
mod napcat_login;
mod offline_reminder;
mod outcome;
mod pets;
mod planting;
mod preferences;
mod ranch;
mod section_page;
mod settings;
mod system_runtime;
mod weather;

use nsh_core::prelude::*;

use accounts::AccountPage;
pub(crate) use accounts::{
    account_label, load_accounts, platform_label, selected_account, AccountRow,
};
use dashboard::DashboardPage;

use crate::navigation::Section;

#[derive(Clone, Copy)]
pub(crate) struct Workspace {
    pub section: Signal<Section>,
    pub account_id: Signal<Option<String>>,
    pub rows: Resource<Vec<accounts::AccountRow>, String>,
    pub pending: Signal<Option<String>>,
    pub command_failure: Signal<Option<String>>,
}

pub(crate) fn content(workspace: Workspace) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(0.0))
        .bg(move || nsh_base::theme().background)
        .child(when(
            move || workspace.section.get() == Section::Dashboard,
            move || {
                dashboard::page(DashboardPage {
                    section: workspace.section,
                    account_id: workspace.account_id,
                    rows: workspace.rows,
                    pending: workspace.pending,
                    command_failure: workspace.command_failure,
                })
            },
        ))
        .child(when(
            move || workspace.section.get() == Section::Accounts,
            move || {
                accounts::page(AccountPage {
                    rows: workspace.rows,
                    selected: workspace.account_id,
                    pending: workspace.pending,
                    failure: workspace.command_failure,
                })
            },
        ))
        .child(when(
            move || workspace.section.get() == Section::Farm,
            move || {
                farm::page(farm::FarmPage {
                    account_id: workspace.account_id,
                    rows: workspace.rows,
                })
            },
        ))
        .child(when(
            move || workspace.section.get() == Section::Friends,
            move || {
                friends::page(friends::FriendPage {
                    account_id: workspace.account_id,
                    rows: workspace.rows,
                })
            },
        ))
        .child(when(
            move || workspace.section.get() == Section::Analytics,
            move || analytics::page(analytics::AnalyticsPage { account_id: workspace.account_id }),
        ))
        .child(when(
            move || workspace.section.get() == Section::Mall,
            move || {
                mall::page(mall::MallPage {
                    account_id: workspace.account_id,
                    rows: workspace.rows,
                })
            },
        ))
        .child(when(
            move || workspace.section.get() == Section::Collections,
            move || {
                collections::page(collections::CollectionPage {
                    account_id: workspace.account_id,
                    rows: workspace.rows,
                })
            },
        ))
        .child(when(
            move || workspace.section.get() == Section::Activities,
            move || {
                activities::page(activities::ActivityPage {
                    account_id: workspace.account_id,
                    rows: workspace.rows,
                })
            },
        ))
        .child(when(
            move || workspace.section.get() == Section::Settings,
            move || settings::page(settings::SettingsPage { account_id: workspace.account_id }),
        ))
        .child(when(move || workspace.section.get() == Section::Config, game_config::page))
        .child(when(
            move || workspace.section.get() == Section::Ranch,
            move || {
                ranch::page(ranch::RanchPage {
                    account_id: workspace.account_id,
                    rows: workspace.rows,
                })
            },
        ))
        .child(each(
            move || pending_section(workspace.section.get()),
            |section: &Section| section.id().to_owned(),
            move |section, _| section_page::page(section.get_untracked()),
        ))
}

fn pending_section(section: Section) -> Vec<Section> {
    section.detail_key().is_some().then_some(section).into_iter().collect()
}
