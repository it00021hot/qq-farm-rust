use super::*;

fn initial_page_route(selected_page: &str) -> (Page, &'static str) {
    let page = Page::from_id(selected_page).unwrap_or(Page::Dashboard);
    (page, default_page_section(page))
}

impl FarmDesktop {
    pub fn new(
        app: Arc<AppContext>,
        tokio_runtime: Arc<tokio::runtime::Runtime>,
        lease: Arc<EngineLease>,
        tray: Option<crate::tray::TrayLifetime>,
        test_mode: bool,
        mut preferences: crate::runtime::UiPreferences,
        tray_events: mpsc::UnboundedReceiver<crate::tray::Action>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let accounts = accounts::read_accounts(&app);
        let selected_account = preferences
            .selected_account
            .clone()
            .filter(|selected| accounts.records.iter().any(|(id, _, _)| id == selected))
            .or_else(|| accounts.records.first().map(|(id, _, _)| id.clone()));
        let (page, initial_section) = initial_page_route(&preferences.selected_page);
        let migrate_page_preference = preferences.selected_page != page.id();
        preferences.selected_page = page.id().into();
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(super::navigation::search_placeholder_for(page, &preferences.language))
        });
        let mut commerce = CommerceState::new(&preferences.language, window, cx);
        if page == Page::Mall {
            commerce.mall_slot_type = match initial_section {
                "svip" => 4,
                "mystery" => -1,
                _ => 1,
            };
        } else if page == Page::MysteryShop {
            commerce.mall_slot_type = -1;
        }
        let accounts_state = AccountsState::new(accounts, &preferences.language, window, cx);
        let dashboard = DashboardState::new(&preferences.language, window, cx);
        let farm = FarmState::new(&preferences.language, window, cx);
        let friends = FriendsState::new(&preferences.language, window, cx);
        let analytics = AnalyticsState::new(
            &app,
            selected_account.as_deref(),
            &preferences.language,
            window,
            cx,
        );
        let game_config = GameConfigState::new(&preferences.language, window, cx);
        let solar_calendar = SolarCalendarState::default();
        let milk_tea = MilkTeaState::new(&preferences.language, window, cx);
        let share_reward = ShareRewardState::default();
        let settings = SettingsState::new(&preferences.language, window, cx);

        cx.observe(&search, |this, _, cx| {
            this.friends.page = 0;
            this.game_config.page = 0;
            cx.notify();
        })
        .detach();
        let (event_receiver, realtime_subscription) =
            runtime::subscribe_realtime(&app, &tokio_runtime, test_mode);
        runtime::observe_window(window, cx);
        let mut view = Self {
            app,
            tokio_runtime,
            _lease: lease,
            _tray: tray,
            test_mode,
            preferences: preferences.clone(),
            window_handle: None,
            focus_handle: cx.focus_handle(),
            sidebar_collapsed: preferences.sidebar_collapsed,
            sidebar_layout: cx.new(|_| gpui_kit::component::resizable::ResizableState::default()),
            page,
            page_section: initial_section.into(),
            selected_account,
            rows: Vec::new(),
            summary: "账号状态与近期运行记录".into(),
            loading: false,
            quitting: false,
            error: None,
            request_revision: 0,
            realtime: event_receiver,
            realtime_subscription,
            search,
            page_raw: serde_json::Value::Null,
            tray_events,
            update_checking: false,
            toast: None,
            toast_generation: 0,
            commerce,
            accounts: accounts_state,
            dashboard,
            farm,
            friends,
            analytics,
            solar_calendar,
            milk_tea,
            share_reward,
            game_config,
            settings,
        };
        if !test_mode {
            view.listen_events(cx);
            view.listen_tray(cx);
            if view.page == Page::Dashboard {
                view.start_dashboard_clock(cx);
            }
            if view.page == Page::SolarCalendar {
                view.start_solar_calendar_clock(cx);
            }
        }
        view.refresh(cx);
        if migrate_page_preference {
            view.persist_ui_preferences();
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::initial_page_route;
    use crate::pages::Page;

    #[test]
    fn mystery_shop_has_a_persistent_standalone_navigation_route() {
        assert_eq!(initial_page_route("mystery-shop"), (Page::MysteryShop, "all"));
        assert_eq!(initial_page_route("mall"), (Page::Mall, "normal"));
    }
}
