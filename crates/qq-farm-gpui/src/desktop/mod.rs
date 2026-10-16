use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use futures::{channel::mpsc, StreamExt};
use gpui_kit::base::{h_flex, Disableable, StyledExt};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{ActiveTheme, Icon};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::AppContext as GpuiContextExt;
use gpui_kit::*;
use qq_farm_app::runtime_lease::EngineLease;
use qq_farm_app::session::AppContext;

use crate::pages::{self, DisplayRow, Page};

mod commands;
#[cfg(feature = "ui-preview")]
mod preview;
#[cfg(feature = "ui-preview")]
pub(crate) use preview::render_previews;

gpui_kit::actions!(
    qq_farm_gpui,
    [
        RefreshPage,
        FocusSearch,
        ToggleSidebar,
        OpenSettings,
        SaveChanges,
        CheckForUpdates,
        QuitApp,
        CancelOverlay,
        NextPage,
        PreviousPage,
    ]
);

mod accounts;
mod activities;
mod analytics;
mod commerce;
mod dashboard;
mod friends;
mod game_config;
mod init;
mod land_grid;
mod navigation;
pub(crate) use navigation::{default_page_section, page_sections};
mod personal;
pub(crate) use friends::{
    friend_action_type_number, friend_matches_search, friend_pet_sync_progress,
    friend_status_localized, friend_value_number, has_friend_action, stealable_friends_matching,
};
pub(crate) use personal::bag_category_for_type;
mod runtime;
mod settings;
pub(in crate::desktop) use settings::{json_pointer, settings_text};
mod shell;
mod view;
pub(crate) use view::{
    page_items, render_asset_image, render_asset_preview, render_asset_preview_sized,
    render_record_asset_preview_sized, PageItem, TablePager, TablePagerTarget,
};

pub(crate) use accounts::{
    account_value, parse_account_login_input, upsert_and_maybe_start_account,
};
pub(crate) use activities::{
    activity_action_enabled, activity_section_for_record, activity_title_for_section,
    star_sand_purchase_feedback, value_to_action_string,
};

mod state;
mod toast;
pub(crate) use accounts::AccountsState;
pub(crate) use activities::ActivitiesState;
pub(crate) use analytics::AnalyticsState;
pub(crate) use commerce::CommerceState;
pub(crate) use dashboard::DashboardState;
pub(crate) use friends::{FriendFeedbackLevel, FriendsState};
pub(crate) use game_config::GameConfigState;
pub(crate) use personal::FarmState;
pub(crate) use settings::SettingsState;
pub use state::FarmDesktop;

pub(crate) enum DesktopEvent {
    Realtime(Vec<qq_farm_app::events::PanelRealtimeEvent>),
    Refresh,
}

#[cfg(test)]
mod tests;
