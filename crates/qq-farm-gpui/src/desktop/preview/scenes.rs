use super::*;

pub(super) struct PreviewCase {
    pub(super) name: &'static str,
    pub(super) mode: ThemeMode,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) scene: &'static str,
    pub(super) font_scale: f32,
}

pub(super) fn cases() -> Vec<PreviewCase> {
    let mut cases = [
        ("accounts-light", ThemeMode::Light, 1440., 900., "accounts", 1.),
        ("accounts-dark", ThemeMode::Dark, 1440., 900., "accounts", 1.),
        ("accounts-minimum", ThemeMode::Light, 1100., 720., "accounts", 1.),
        ("account-sheet-minimum", ThemeMode::Light, 1100., 720., "account-sheet", 1.),
        ("account-sheet-125", ThemeMode::Light, 1100., 720., "account-sheet", 1.25),
        ("account-sheet-150", ThemeMode::Dark, 1100., 720., "account-sheet", 1.5),
        ("dashboard-light", ThemeMode::Light, 1440., 900., "dashboard", 1.),
        ("dashboard-dark", ThemeMode::Dark, 1440., 900., "dashboard", 1.),
        ("activity-center-light", ThemeMode::Light, 1440., 900., "activity-center", 1.),
        ("activity-center-dark", ThemeMode::Dark, 1440., 900., "activity-center", 1.),
        ("solar-calendar-light", ThemeMode::Light, 1440., 900., "solar-calendar", 1.),
        ("solar-calendar-dark", ThemeMode::Dark, 1440., 900., "solar-calendar", 1.),
        ("milk-tea-light", ThemeMode::Light, 1440., 900., "milk-tea", 1.),
        ("milk-tea-dark", ThemeMode::Dark, 1440., 900., "milk-tea", 1.),
        ("milk-tea-shop-light", ThemeMode::Light, 1440., 900., "milk-tea-shop", 1.),
        ("milk-tea-shop-dark", ThemeMode::Dark, 1440., 900., "milk-tea-shop", 1.),
        ("milk-tea-workshop-light", ThemeMode::Light, 1440., 900., "milk-tea-workshop", 1.),
        ("milk-tea-workshop-dark", ThemeMode::Dark, 1440., 900., "milk-tea-workshop", 1.),
        ("milk-tea-hire-light", ThemeMode::Light, 1440., 900., "milk-tea-hire", 1.),
        ("milk-tea-hire-dark", ThemeMode::Dark, 1440., 900., "milk-tea-hire", 1.),
        ("milk-tea-work-light", ThemeMode::Light, 1440., 900., "milk-tea-work", 1.),
        ("milk-tea-work-dark", ThemeMode::Dark, 1440., 900., "milk-tea-work", 1.),
        ("milk-tea-exchange-light", ThemeMode::Light, 1440., 900., "milk-tea-exchange", 1.),
        ("milk-tea-exchange-dark", ThemeMode::Dark, 1440., 900., "milk-tea-exchange", 1.),
        ("milk-tea-fund-light", ThemeMode::Light, 1440., 900., "milk-tea-fund", 1.),
        ("milk-tea-fund-dark", ThemeMode::Dark, 1440., 900., "milk-tea-fund", 1.),
        ("milk-tea-rank-light", ThemeMode::Light, 1440., 900., "milk-tea-rank", 1.),
        ("milk-tea-rank-dark", ThemeMode::Dark, 1440., 900., "milk-tea-rank", 1.),
        ("milk-tea-logs-light", ThemeMode::Light, 1440., 900., "milk-tea-logs", 1.),
        ("milk-tea-logs-dark", ThemeMode::Dark, 1440., 900., "milk-tea-logs", 1.),
        ("share-reward-light", ThemeMode::Light, 1440., 900., "share-reward", 1.),
        ("share-reward-dark", ThemeMode::Dark, 1440., 900., "share-reward", 1.),
        ("farm-light", ThemeMode::Light, 1440., 900., "farm", 1.),
        ("farm-dark", ThemeMode::Dark, 1440., 900., "farm", 1.),
        ("bag-light", ThemeMode::Light, 1440., 900., "bag", 1.),
        ("bag-dark", ThemeMode::Dark, 1440., 900., "bag", 1.),
        ("daily-gifts-light", ThemeMode::Light, 1440., 900., "daily-gifts", 1.),
        ("daily-gifts-dark", ThemeMode::Dark, 1440., 900., "daily-gifts", 1.),
        ("illustrated-light", ThemeMode::Light, 1440., 900., "illustrated", 1.),
        ("illustrated-dark", ThemeMode::Dark, 1440., 900., "illustrated", 1.),
        ("illustrated-mutants-light", ThemeMode::Light, 1440., 900., "illustrated-mutants", 1.),
        ("interaction-light", ThemeMode::Light, 1440., 900., "interaction", 1.),
        ("interaction-dark", ThemeMode::Dark, 1440., 900., "interaction", 1.),
        ("pets-light", ThemeMode::Light, 1440., 900., "pets", 1.),
        ("pets-dark", ThemeMode::Dark, 1440., 900., "pets", 1.),
        ("friends-light", ThemeMode::Light, 1440., 900., "friends", 1.),
        ("friends-dark", ThemeMode::Dark, 1440., 900., "friends", 1.),
        ("friends-interaction-light", ThemeMode::Light, 1440., 900., "friends-interaction", 1.),
        ("friends-interaction-dark", ThemeMode::Dark, 1440., 900., "friends-interaction", 1.),
        ("friends-blacklist-light", ThemeMode::Light, 1440., 900., "friends-blacklist", 1.),
        ("friends-blacklist-dark", ThemeMode::Dark, 1440., 900., "friends-blacklist", 1.),
        ("friends-visitors-light", ThemeMode::Light, 1440., 900., "friends-visitors", 1.),
        ("friends-visitors-dark", ThemeMode::Dark, 1440., 900., "friends-visitors", 1.),
        ("analytics-light", ThemeMode::Light, 1440., 900., "analytics", 1.),
        ("analytics-dark", ThemeMode::Dark, 1440., 900., "analytics", 1.),
        ("analytics-blacklist-light", ThemeMode::Light, 1440., 900., "analytics-blacklist", 1.),
        ("analytics-blacklist-dark", ThemeMode::Dark, 1440., 900., "analytics-blacklist", 1.),
        ("mall-light", ThemeMode::Light, 1440., 900., "mall", 1.),
        ("mall-dark", ThemeMode::Dark, 1440., 900., "mall", 1.),
        ("mall-svip-light", ThemeMode::Light, 1440., 900., "mall-svip", 1.),
        ("mall-svip-dark", ThemeMode::Dark, 1440., 900., "mall-svip", 1.),
        ("mall-mystery-light", ThemeMode::Light, 1440., 900., "mall-mystery", 1.),
        ("mall-mystery-dark", ThemeMode::Dark, 1440., 900., "mall-mystery", 1.),
        ("mystery-shop-light", ThemeMode::Light, 1440., 900., "mystery-shop", 1.),
        ("mystery-shop-dark", ThemeMode::Dark, 1440., 900., "mystery-shop", 1.),
        ("mystery-shop-inactive-light", ThemeMode::Light, 1440., 900., "mystery-shop-inactive", 1.),
        ("mystery-shop-inactive-dark", ThemeMode::Dark, 1440., 900., "mystery-shop-inactive", 1.),
        ("mall-empty-light", ThemeMode::Light, 1440., 900., "mall-empty", 1.),
        ("mall-empty-dark", ThemeMode::Dark, 1440., 900., "mall-empty", 1.),
        ("bag-empty-light", ThemeMode::Light, 1440., 900., "bag-empty", 1.),
        ("daily-gifts-empty-light", ThemeMode::Light, 1440., 900., "daily-gifts-empty", 1.),
        ("pets-empty-light", ThemeMode::Light, 1440., 900., "pets-empty", 1.),
        ("friends-empty-light", ThemeMode::Light, 1440., 900., "friends-empty", 1.),
        ("game-config-empty-light", ThemeMode::Light, 1440., 900., "game-config-empty", 1.),
        ("accounts-empty-light", ThemeMode::Light, 1440., 900., "accounts-empty", 1.),
        ("dashboard-no-account-light", ThemeMode::Light, 1440., 900., "dashboard-no-account", 1.),
        ("analytics-no-account-light", ThemeMode::Light, 1440., 900., "analytics-no-account", 1.),
        ("mall-no-account-light", ThemeMode::Light, 1440., 900., "mall-no-account", 1.),
        ("friends-no-account-light", ThemeMode::Light, 1440., 900., "friends-no-account", 1.),
        ("friends-loading-light", ThemeMode::Light, 1440., 900., "friends-loading", 1.),
        ("friends-error-light", ThemeMode::Light, 1440., 900., "friends-error", 1.),
        ("friends-error-dark", ThemeMode::Dark, 1440., 900., "friends-error", 1.),
        ("friends-records-error-light", ThemeMode::Light, 1440., 900., "friends-records-error", 1.),
        ("friends-records-error-dark", ThemeMode::Dark, 1440., 900., "friends-records-error", 1.),
        ("farm-section-error-light", ThemeMode::Light, 1440., 900., "farm-section-error", 1.),
        ("farm-section-error-dark", ThemeMode::Dark, 1440., 900., "farm-section-error", 1.),
        ("mall-confirm-light", ThemeMode::Light, 1440., 900., "mall-confirm", 1.),
        ("mall-confirm-dark", ThemeMode::Dark, 1440., 900., "mall-confirm", 1.),
        ("mall-confirm-english", ThemeMode::Light, 1440., 900., "mall-confirm", 1.),
        ("mystery-confirm-light", ThemeMode::Light, 1440., 900., "mystery-confirm", 1.),
        ("mystery-confirm-dark", ThemeMode::Dark, 1440., 900., "mystery-confirm", 1.),
        ("game-config-editor-light", ThemeMode::Light, 1440., 900., "game-config-editor", 1.),
        ("game-config-editor-dark", ThemeMode::Dark, 1440., 900., "game-config-editor", 1.),
        ("pets-logs-light", ThemeMode::Light, 1440., 900., "pets-logs", 1.),
        ("pets-logs-dark", ThemeMode::Dark, 1440., 900., "pets-logs", 1.),
        (
            "account-delete-confirm-light",
            ThemeMode::Light,
            1440.,
            900.,
            "account-delete-confirm",
            1.,
        ),
        ("account-delete-confirm-dark", ThemeMode::Dark, 1440., 900., "account-delete-confirm", 1.),
        (
            "game-config-delete-confirm-light",
            ThemeMode::Light,
            1440.,
            900.,
            "game-config-delete-confirm",
            1.,
        ),
        (
            "game-config-delete-confirm-dark",
            ThemeMode::Dark,
            1440.,
            900.,
            "game-config-delete-confirm",
            1.,
        ),
        (
            "friends-delete-confirm-light",
            ThemeMode::Light,
            1440.,
            900.,
            "friends-delete-confirm",
            1.,
        ),
        ("friends-delete-confirm-dark", ThemeMode::Dark, 1440., 900., "friends-delete-confirm", 1.),
        ("accounts-selection-light", ThemeMode::Light, 1440., 900., "accounts-selection", 1.),
        ("accounts-selection-dark", ThemeMode::Dark, 1440., 900., "accounts-selection", 1.),
        ("game-config-selection-light", ThemeMode::Light, 1440., 900., "game-config-selection", 1.),
        ("game-config-selection-dark", ThemeMode::Dark, 1440., 900., "game-config-selection", 1.),
        ("account-sheet-edit-light", ThemeMode::Light, 1440., 900., "account-sheet-edit", 1.),
        (
            "game-config-editor-new-light",
            ThemeMode::Light,
            1440.,
            900.,
            "game-config-editor-new",
            1.,
        ),
        (
            "game-config-editor-fruit-light",
            ThemeMode::Light,
            1440.,
            900.,
            "game-config-editor-fruit",
            1.,
        ),
        (
            "game-config-editor-item-light",
            ThemeMode::Light,
            1440.,
            900.,
            "game-config-editor-item",
            1.,
        ),
        (
            "friends-unblacklist-confirm-light",
            ThemeMode::Light,
            1440.,
            900.,
            "friends-unblacklist-confirm",
            1.,
        ),
        ("accounts-filter-empty-light", ThemeMode::Light, 1440., 900., "accounts-filter-empty", 1.),
        (
            "game-config-filter-empty-light",
            ThemeMode::Light,
            1440.,
            900.,
            "game-config-filter-empty",
            1.,
        ),
        (
            "analytics-blacklist-empty-light",
            ThemeMode::Light,
            1440.,
            900.,
            "analytics-blacklist-empty",
            1.,
        ),
        (
            "game-config-select-open-light",
            ThemeMode::Light,
            1440.,
            900.,
            "game-config-select-open",
            1.,
        ),
        (
            "game-config-types-open-light",
            ThemeMode::Light,
            1440.,
            900.,
            "game-config-types-open",
            1.,
        ),
        (
            "settings-strategy-open-light",
            ThemeMode::Light,
            1440.,
            900.,
            "settings-strategy-open",
            1.,
        ),
        (
            "friends-blacklist-confirm-light",
            ThemeMode::Light,
            1440.,
            900.,
            "friends-blacklist-confirm",
            1.,
        ),
        ("mall-notice-light", ThemeMode::Light, 1440., 900., "mall-notice", 1.),
        ("friends-feedback-light", ThemeMode::Light, 1440., 900., "friends-feedback", 1.),
        ("bag-notice-light", ThemeMode::Light, 1440., 900., "bag-notice", 1.),
        ("friends-detail-error-light", ThemeMode::Light, 1440., 900., "friends-detail-error", 1.),
        ("account-sheet-wx-light", ThemeMode::Light, 1440., 900., "account-sheet-wx", 1.),
        ("account-sheet-wx-qr-light", ThemeMode::Light, 1440., 900., "account-sheet-wx-qr", 1.),
        (
            "account-sheet-wx-error-light",
            ThemeMode::Light,
            1440.,
            900.,
            "account-sheet-wx-error",
            1.,
        ),
        (
            "account-sheet-form-error-light",
            ThemeMode::Light,
            1440.,
            900.,
            "account-sheet-form-error",
            1.,
        ),
        ("dashboard-empty-logs-light", ThemeMode::Light, 1440., 900., "dashboard-empty-logs", 1.),
        (
            "dashboard-logs-expanded-light",
            ThemeMode::Light,
            1440.,
            900.,
            "dashboard-logs-expanded",
            1.,
        ),
        ("accounts-search-light", ThemeMode::Light, 1440., 900., "accounts-search", 1.),
        ("accounts-search-dark", ThemeMode::Dark, 1440., 900., "accounts-search", 1.),
        ("accounts-columns-light", ThemeMode::Light, 1440., 900., "accounts-columns", 1.),
        ("game-config-search-light", ThemeMode::Light, 1440., 900., "game-config-search", 1.),
        ("game-config-search-dark", ThemeMode::Dark, 1440., 900., "game-config-search", 1.),
        ("game-config-columns-light", ThemeMode::Light, 1440., 900., "game-config-columns", 1.),
        ("mall-non-svip-light", ThemeMode::Light, 1440., 900., "mall-non-svip", 1.),
        ("mall-svip-locked-light", ThemeMode::Light, 1440., 900., "mall-svip-locked", 1.),
        ("accounts-pagination-light", ThemeMode::Light, 1440., 900., "accounts-pagination", 1.),
        ("bag-selection-light", ThemeMode::Light, 1440., 900., "bag-selection", 1.),
        ("dashboard-logs-light", ThemeMode::Light, 1440., 900., "dashboard-logs", 1.),
        ("mall-confirm-pending-light", ThemeMode::Light, 1440., 900., "mall-confirm-pending", 1.),
        ("settings-light", ThemeMode::Light, 1440., 900., "settings", 1.),
        ("settings-dark", ThemeMode::Dark, 1440., 900., "settings", 1.),
        ("settings-automation-light", ThemeMode::Light, 1440., 900., "settings-automation", 1.),
        ("settings-automation-dark", ThemeMode::Dark, 1440., 900., "settings-automation", 1.),
        ("settings-offline-light", ThemeMode::Light, 1440., 900., "settings-offline", 1.),
        ("settings-offline-dark", ThemeMode::Dark, 1440., 900., "settings-offline", 1.),
        ("settings-dingtalk-light", ThemeMode::Light, 1440., 900., "settings-dingtalk", 1.),
        ("settings-dingtalk-dark", ThemeMode::Dark, 1440., 900., "settings-dingtalk", 1.),
        (
            "settings-offline-bound-light",
            ThemeMode::Light,
            1440.,
            900.,
            "settings-offline-bound",
            1.,
        ),
        ("settings-offline-bound-dark", ThemeMode::Dark, 1440., 900., "settings-offline-bound", 1.),
        ("settings-system-light", ThemeMode::Light, 1440., 900., "settings-system", 1.),
        ("settings-system-dark", ThemeMode::Dark, 1440., 900., "settings-system", 1.),
        ("settings-ui-light", ThemeMode::Light, 1440., 900., "settings-ui", 1.),
        ("settings-ui-dark", ThemeMode::Dark, 1440., 900., "settings-ui", 1.),
        ("game-config-light", ThemeMode::Light, 1440., 900., "game-config", 1.),
        ("game-config-dark", ThemeMode::Dark, 1440., 900., "game-config", 1.),
        ("game-config-fruits-light", ThemeMode::Light, 1440., 900., "game-config-fruits", 1.),
        ("game-config-fruits-dark", ThemeMode::Dark, 1440., 900., "game-config-fruits", 1.),
        ("game-config-items-light", ThemeMode::Light, 1440., 900., "game-config-items", 1.),
        ("game-config-items-dark", ThemeMode::Dark, 1440., 900., "game-config-items", 1.),
        ("dashboard-minimum", ThemeMode::Light, 1100., 720., "dashboard", 1.),
        ("farm-minimum", ThemeMode::Light, 1100., 720., "farm", 1.),
        ("friends-minimum", ThemeMode::Light, 1100., 720., "friends", 1.),
        ("analytics-minimum", ThemeMode::Light, 1100., 720., "analytics", 1.),
        ("mall-minimum", ThemeMode::Light, 1100., 720., "mall", 1.),
        ("settings-minimum", ThemeMode::Light, 1100., 720., "settings", 1.),
        ("game-config-minimum", ThemeMode::Light, 1100., 720., "game-config", 1.),
        ("bag-minimum", ThemeMode::Light, 1100., 720., "bag", 1.),
        ("daily-gifts-minimum", ThemeMode::Light, 1100., 720., "daily-gifts", 1.),
        ("pets-minimum", ThemeMode::Light, 1100., 720., "pets", 1.),
        ("illustrated-minimum", ThemeMode::Light, 1100., 720., "illustrated", 1.),
        ("interaction-minimum", ThemeMode::Light, 1100., 720., "interaction", 1.),
        ("mystery-shop-minimum", ThemeMode::Light, 1100., 720., "mystery-shop", 1.),
        ("accounts-150", ThemeMode::Light, 1440., 900., "accounts", 1.5),
        ("game-config-150", ThemeMode::Dark, 1440., 900., "game-config", 1.5),
        ("dashboard-150", ThemeMode::Light, 1440., 900., "dashboard", 1.5),
        ("friends-150", ThemeMode::Light, 1440., 900., "friends", 1.5),
        ("settings-150", ThemeMode::Light, 1440., 900., "settings", 1.5),
        ("mall-150", ThemeMode::Light, 1440., 900., "mall", 1.5),
        ("bag-150", ThemeMode::Light, 1440., 900., "bag", 1.5),
        ("pets-150", ThemeMode::Light, 1440., 900., "pets", 1.5),
    ]
    .into_iter()
    .map(|(name, mode, width, height, scene, font_scale)| PreviewCase {
        name,
        mode,
        width,
        height,
        scene,
        font_scale,
    })
    .collect::<Vec<_>>();
    for (name, scene) in [
        ("accounts-english", "accounts"),
        ("dashboard-english", "dashboard"),
        ("activity-center-english", "activity-center"),
        ("solar-calendar-english", "solar-calendar"),
        ("milk-tea-english", "milk-tea"),
        ("milk-tea-shop-english", "milk-tea-shop"),
        ("milk-tea-workshop-english", "milk-tea-workshop"),
        ("milk-tea-hire-english", "milk-tea-hire"),
        ("milk-tea-work-english", "milk-tea-work"),
        ("milk-tea-exchange-english", "milk-tea-exchange"),
        ("milk-tea-fund-english", "milk-tea-fund"),
        ("milk-tea-rank-english", "milk-tea-rank"),
        ("milk-tea-logs-english", "milk-tea-logs"),
        ("share-reward-english", "share-reward"),
        ("farm-english", "farm"),
        ("bag-english", "bag"),
        ("daily-gifts-english", "daily-gifts"),
        ("illustrated-english", "illustrated"),
        ("interaction-english", "interaction"),
        ("pets-english", "pets"),
        ("friends-english", "friends"),
        ("friends-interaction-english", "friends-interaction"),
        ("friends-blacklist-english", "friends-blacklist"),
        ("friends-visitors-english", "friends-visitors"),
        ("friends-delete-confirm-english", "friends-delete-confirm"),
        ("analytics-english", "analytics"),
        ("analytics-blacklist-english", "analytics-blacklist"),
        ("mall-english", "mall"),
        ("mall-svip-english", "mall-svip"),
        ("mall-mystery-english", "mall-mystery"),
        ("mystery-shop-english", "mystery-shop"),
        ("settings-english", "settings"),
        ("settings-automation-english", "settings-automation"),
        ("settings-offline-english", "settings-offline"),
        ("settings-dingtalk-english", "settings-dingtalk"),
        ("settings-offline-bound-english", "settings-offline-bound"),
        ("settings-system-english", "settings-system"),
        ("settings-ui-english", "settings-ui"),
        ("game-config-english", "game-config"),
        ("game-config-fruits-english", "game-config-fruits"),
        ("game-config-items-english", "game-config-items"),
    ] {
        cases.push(PreviewCase {
            name,
            mode: ThemeMode::Light,
            width: 1440.,
            height: 900.,
            scene,
            font_scale: 1.,
        });
    }
    cases
}

fn page_for_scene(scene: &str) -> Page {
    match scene {
        "dashboard" => Page::Dashboard,
        "dashboard-no-account" => Page::Dashboard,
        "dashboard-logs" => Page::Dashboard,
        "dashboard-empty-logs" => Page::Dashboard,
        "dashboard-logs-expanded" => Page::Dashboard,
        "farm" => Page::Personal,
        "farm-section-error" => Page::Personal,
        "bag" => Page::Personal,
        "bag-empty" => Page::Personal,
        "bag-selection" => Page::Personal,
        "bag-notice" => Page::Personal,
        "daily-gifts" => Page::Personal,
        "daily-gifts-empty" => Page::Personal,
        "illustrated" => Page::Personal,
        "illustrated-mutants" => Page::Personal,
        "interaction" => Page::Personal,
        "pets" => Page::Personal,
        "pets-empty" => Page::Personal,
        "pets-logs" => Page::Personal,
        "friends" => Page::Friends,
        "friends-no-account" => Page::Friends,
        "friends-loading" => Page::Friends,
        "friends-error" => Page::Friends,
        "friends-records-error" => Page::Friends,
        "friends-delete-confirm" => Page::Friends,
        "friends-unblacklist-confirm" => Page::Friends,
        "friends-blacklist-confirm" => Page::Friends,
        "friends-feedback" => Page::Friends,
        "friends-detail-error" => Page::Friends,
        "friends-empty" => Page::Friends,
        "friends-interaction" => Page::Friends,
        "friends-blacklist" => Page::Friends,
        "friends-visitors" => Page::Friends,
        "analytics" => Page::Analytics,
        "analytics-no-account" => Page::Analytics,
        "analytics-blacklist" => Page::Analytics,
        "analytics-blacklist-empty" => Page::Analytics,
        "mall" => Page::Mall,
        "mall-no-account" => Page::Mall,
        "mall-empty" => Page::Mall,
        "mall-svip" => Page::Mall,
        "mall-mystery" => Page::Mall,
        "mall-confirm" => Page::Mall,
        "mall-confirm-pending" => Page::Mall,
        "mall-non-svip" => Page::Mall,
        "mall-svip-locked" => Page::Mall,
        "mall-search-empty" => Page::Mall,
        "mall-notice" => Page::Mall,
        "mystery-shop" => Page::MysteryShop,
        "mystery-shop-inactive" => Page::MysteryShop,
        "mystery-confirm" => Page::MysteryShop,
        "settings" => Page::Settings,
        "settings-automation" => Page::Settings,
        "settings-offline" => Page::Settings,
        "settings-dingtalk" => Page::Settings,
        "settings-offline-bound" => Page::Settings,
        "settings-system" => Page::Settings,
        "settings-ui" => Page::Settings,
        "settings-strategy-open" => Page::Settings,
        "solar-calendar" => Page::SolarCalendar,
        "activity-center" => Page::Activities,
        "milk-tea" => Page::MilkTea,
        "milk-tea-shop" | "milk-tea-workshop" | "milk-tea-hire" | "milk-tea-work"
        | "milk-tea-exchange" | "milk-tea-fund" | "milk-tea-rank" | "milk-tea-logs" => {
            Page::MilkTea
        }
        "share-reward" => Page::ShareReward,
        "game-config" => Page::GameConfig,
        "game-config-empty" => Page::GameConfig,
        "game-config-fruits" => Page::GameConfig,
        "game-config-items" => Page::GameConfig,
        "game-config-editor" => Page::GameConfig,
        "game-config-delete-confirm" => Page::GameConfig,
        "game-config-selection" => Page::GameConfig,
        "game-config-editor-new" => Page::GameConfig,
        "game-config-editor-fruit" => Page::GameConfig,
        "game-config-editor-item" => Page::GameConfig,
        "game-config-filter-empty" => Page::GameConfig,
        "game-config-search" => Page::GameConfig,
        "game-config-columns" => Page::GameConfig,
        "game-config-select-open" => Page::GameConfig,
        "game-config-types-open" => Page::GameConfig,
        "account-delete-confirm" => Page::Accounts,
        "account-sheet-edit" => Page::Accounts,
        "account-sheet-wx" => Page::Accounts,
        "account-sheet-wx-qr" => Page::Accounts,
        "account-sheet-wx-error" => Page::Accounts,
        "account-sheet-form-error" => Page::Accounts,
        "accounts-selection" => Page::Accounts,
        "accounts-search" => Page::Accounts,
        "accounts-columns" => Page::Accounts,
        "accounts-pagination" => Page::Accounts,
        "accounts-filter-empty" => Page::Accounts,
        _ => Page::Accounts,
    }
}

fn section_for_scene(scene: &str) -> &'static str {
    match scene {
        "friends" => "friends",
        "friends-no-account" => "friends",
        "friends-loading" => "friends",
        "friends-error" => "friends",
        "friends-records-error" => "records",
        "friends-delete-confirm" => "friends",
        "friends-unblacklist-confirm" => "blacklist",
        "friends-blacklist-confirm" => "friends",
        "friends-feedback" => "friends",
        "friends-detail-error" => "friends",
        "friends-empty" => "friends",
        "friends-interaction" => "interactionItems",
        "friends-blacklist" => "blacklist",
        "friends-visitors" => "records",
        "analytics" => "strategy",
        "analytics-no-account" => "strategy",
        "analytics-blacklist" => "plantBlacklist",
        "analytics-blacklist-empty" => "plantBlacklist",
        "mall" => "normal",
        "mall-no-account" => "normal",
        "mall-empty" => "normal",
        "mall-svip" => "svip",
        "mall-mystery" => "mystery",
        "mall-confirm" => "normal",
        "mall-confirm-pending" => "normal",
        "mall-non-svip" => "normal",
        "mall-svip-locked" => "svip",
        "mystery-shop" => "mystery",
        "mystery-shop-inactive" => "mystery",
        "mystery-confirm" => "mystery",
        "settings" => "strategy",
        "settings-automation" => "automation",
        "settings-offline" => "offlineReminder",
        "settings-dingtalk" => "offlineReminder",
        "settings-offline-bound" => "offlineReminder",
        "settings-system" => "systemConfig",
        "settings-ui" => "ui",
        "settings-strategy-open" => "strategy",
        "game-config" => "seeds",
        "game-config-empty" => "seeds",
        "game-config-fruits" => "fruits",
        "game-config-items" => "items",
        "game-config-editor" => "seeds",
        "game-config-delete-confirm" => "seeds",
        "game-config-selection" => "seeds",
        "game-config-editor-new" => "seeds",
        "game-config-editor-fruit" => "fruits",
        "game-config-editor-item" => "items",
        "game-config-filter-empty" => "seeds",
        "game-config-select-open" => "seeds",
        "game-config-types-open" => "items",
        "game-config-search" => "seeds",
        "game-config-columns" => "seeds",
        "farm" => "lands",
        "farm-section-error" => "lands",
        "bag" => "bag",
        "bag-empty" => "bag",
        "bag-selection" => "bag",
        "bag-notice" => "bag",
        "daily-gifts" => "dailyGifts",
        "daily-gifts-empty" => "dailyGifts",
        "illustrated" => "illustrated",
        "milk-tea-shop" => "shop",
        "milk-tea-workshop" => "workshop",
        "milk-tea-hire" => "hireList",
        "milk-tea-work" => "work",
        "milk-tea-exchange" => "exchange",
        "milk-tea-fund" => "fund",
        "milk-tea-rank" => "rank",
        "milk-tea-logs" => "logs",
        "illustrated-mutants" => "illustrated",
        "interaction" => "interactionItems",
        "pets" => "pets",
        "pets-empty" => "pets",
        "pets-logs" => "pets",
        _ => "",
    }
}

pub(super) fn seed(
    view: &Entity<FarmDesktop>,
    handle: AnyWindowHandle,
    case: &PreviewCase,
    window: &mut Window,
    cx: &mut App,
) {
    view.update(cx, |this, cx| {
        this.window_handle = Some(handle);
        this.accounts.records = vec![
            ("1".into(), "主账号".into(), true),
            ("2".into(), "日常农场".into(), false),
            ("3".into(), "长名称账号 · 微信授权".into(), false),
        ];
        this.selected_account = Some("1".into());
        this.page = page_for_scene(case.scene);
        this.page_section = section_for_scene(case.scene).into();
        this.summary = if this.preferences.language == "en-US" {
            "UI preview · sample snapshot".into()
        } else {
            "界面预览 · 展示用快照".into()
        };
        this.page_raw = match case.scene {
            "dashboard" => super::fixtures::dashboard_snapshot(),
            "dashboard-no-account" => super::fixtures::dashboard_snapshot(),
            "dashboard-logs" => super::fixtures::dashboard_many_logs_snapshot(),
            "dashboard-empty-logs" => super::fixtures::dashboard_empty_logs_snapshot(),
            "dashboard-logs-expanded" => super::fixtures::dashboard_many_logs_snapshot(),
            "solar-calendar" => super::fixtures::solar_calendar_snapshot(),
            "milk-tea" => super::fixtures::milk_tea_snapshot("shop"),
            "milk-tea-shop" => super::fixtures::milk_tea_snapshot("shop"),
            "milk-tea-workshop" => super::fixtures::milk_tea_snapshot("workshop"),
            "milk-tea-hire" => super::fixtures::milk_tea_snapshot("hireList"),
            "milk-tea-work" => super::fixtures::milk_tea_snapshot("work"),
            "milk-tea-exchange" => super::fixtures::milk_tea_snapshot("exchange"),
            "milk-tea-fund" => super::fixtures::milk_tea_snapshot("fund"),
            "milk-tea-rank" => super::fixtures::milk_tea_snapshot("rank"),
            "milk-tea-logs" => super::fixtures::milk_tea_snapshot("logs"),
            "share-reward" => super::fixtures::share_reward_snapshot(),
            "farm" => super::fixtures::farm_snapshot(),
            "farm-section-error" => super::fixtures::farm_snapshot(),
            "bag" => super::fixtures::bag_snapshot(),
            "bag-empty" => super::fixtures::bag_empty_snapshot(),
            "bag-selection" => super::fixtures::bag_selection_snapshot(),
            "bag-notice" => super::fixtures::bag_snapshot(),
            "daily-gifts" => super::fixtures::daily_gifts_snapshot(),
            "daily-gifts-empty" => super::fixtures::daily_gifts_empty_snapshot(),
            "illustrated" => super::fixtures::illustrated_snapshot(),
            "illustrated-mutants" => super::fixtures::illustrated_snapshot(),
            "interaction" => super::fixtures::interaction_items_snapshot(),
            "pets" => super::fixtures::pets_snapshot(),
            "pets-empty" => super::fixtures::pets_empty_snapshot(),
            "pets-logs" => super::fixtures::pets_snapshot(),
            "friends" => super::fixtures::friends_snapshot(),
            "friends-no-account" => super::fixtures::friends_snapshot(),
            "friends-loading" => super::fixtures::friends_snapshot(),
            "friends-error" => super::fixtures::friends_snapshot(),
            "friends-records-error" => super::fixtures::friends_visitors_snapshot(),
            "friends-delete-confirm" => super::fixtures::friends_snapshot(),
            "friends-unblacklist-confirm" => super::fixtures::friends_unblacklist_snapshot(),
            "friends-blacklist-confirm" => super::fixtures::friends_snapshot(),
            "friends-feedback" => super::fixtures::friends_snapshot(),
            "friends-detail-error" => super::fixtures::friends_snapshot(),
            "friends-empty" => super::fixtures::friends_empty_snapshot(),
            "friends-interaction" => super::fixtures::friends_interaction_snapshot(),
            "friends-blacklist" => super::fixtures::friends_blacklist_snapshot(),
            "friends-visitors" => super::fixtures::friends_visitors_snapshot(),
            "analytics" => super::fixtures::analytics_snapshot(),
            "analytics-no-account" => super::fixtures::analytics_snapshot(),
            "analytics-blacklist" => super::fixtures::analytics_snapshot(),
            "analytics-blacklist-empty" => super::fixtures::analytics_blacklist_empty_snapshot(),
            "mall" => super::fixtures::mall_snapshot(),
            "mall-no-account" => super::fixtures::mall_snapshot(),
            "mall-empty" => super::fixtures::mall_empty_snapshot(),
            "mall-svip" => super::fixtures::mall_svip_snapshot(),
            "mall-mystery" => super::fixtures::mystery_shop_snapshot(),
            "mall-confirm" => super::fixtures::mall_snapshot(),
            "mall-confirm-pending" => super::fixtures::mall_snapshot(),
            "mall-non-svip" => super::fixtures::mall_non_svip_snapshot(),
            "mall-svip-locked" => super::fixtures::mall_svip_non_member_snapshot(),
            "mall-notice" => super::fixtures::mall_snapshot(),
            "mystery-shop" => super::fixtures::mystery_shop_snapshot(),
            "mystery-shop-inactive" => super::fixtures::mystery_shop_inactive_snapshot(),
            "mystery-confirm" => super::fixtures::mystery_shop_snapshot(),
            "settings" => super::fixtures::settings_snapshot(),
            "settings-automation" => super::fixtures::settings_snapshot(),
            "settings-offline" => super::fixtures::settings_snapshot(),
            "settings-dingtalk" => super::fixtures::settings_dingtalk_snapshot(),
            "settings-offline-bound" => super::fixtures::settings_qq_bot_bound_snapshot(),
            "settings-system" => super::fixtures::settings_snapshot(),
            "settings-ui" => super::fixtures::settings_snapshot(),
            "settings-strategy-open" => super::fixtures::settings_snapshot(),
            "game-config" => super::fixtures::game_config_snapshot(),
            "game-config-empty" => super::fixtures::game_config_empty_snapshot(),
            "game-config-fruits" => super::fixtures::game_config_snapshot(),
            "game-config-items" => super::fixtures::game_config_snapshot(),
            "game-config-editor" => super::fixtures::game_config_snapshot(),
            "game-config-delete-confirm" => super::fixtures::game_config_snapshot(),
            "game-config-selection" => super::fixtures::game_config_snapshot(),
            "game-config-editor-new" => super::fixtures::game_config_snapshot(),
            "game-config-editor-fruit" => super::fixtures::game_config_snapshot(),
            "game-config-editor-item" => super::fixtures::game_config_snapshot(),
            "game-config-filter-empty" => super::fixtures::game_config_snapshot(),
            "game-config-select-open" => super::fixtures::game_config_snapshot(),
            "game-config-types-open" => super::fixtures::game_config_snapshot(),
            "game-config-search" => super::fixtures::game_config_snapshot(),
            "game-config-columns" => super::fixtures::game_config_snapshot(),
            "accounts-empty" => super::fixtures::accounts_empty_snapshot(),
            "accounts-pagination" => super::fixtures::accounts_many_snapshot(),
            _ => super::fixtures::account_snapshot(),
        };
        if case.scene == "mystery-shop" || case.scene == "mall-mystery" {
            this.commerce.mall_slot_type = -1;
            this.page_raw["slotType"] = serde_json::json!(-1);
        }
        if case.scene.ends_with("-no-account") {
            this.selected_account = None;
        }
        if case.scene == "friends-loading" {
            this.loading = true;
        }
        if case.scene == "friends-error" {
            this.page_raw = serde_json::json!({"friends": []});
            this.error = Some("加载页面失败：网络请求超时".into());
        }
        if case.scene == "friends-records-error" {
            this.page_raw["records"] = serde_json::json!([]);
            this.page_raw["recordsError"] = serde_json::json!("访客服务暂不可用");
        }
        if case.scene == "farm-section-error" {
            this.page_raw["sectionErrors"] = serde_json::json!({"lands": "地块服务暂不可用"});
        }
        if this.page != Page::Accounts {
            this.rows = crate::pages::page_data(this.page, this.page_raw.clone()).rows;
        }
        if this.page == Page::SolarCalendar {
            let calendar = this.page_raw.clone();
            this.apply_solar_calendar(calendar);
        }
        if this.page == Page::Analytics && this.selected_account.is_some() {
            // Snapshots are injected directly here; run the same level sync the
            // real page-load path performs so the reference level matches the
            // account level in the preview.
            this.sync_analytics_level(window, cx);
        }
        if case.scene == "dashboard" {
            this.accounts.records = vec![("1".into(), "主账号".into(), true)];
        }
        if case.scene == "friends" {
            this.friends.detail = Some((
                102,
                serde_json::json!({
                    "career": {"harvest": 12000, "steal": 3000, "level": 18},
                    "lands": [
                        {"id":1,"status":"growing","plantName":"草莓","matureInSec":420,"totalGrowTime":900,"level":3,"currentSeason":1,"totalSeason":2},
                        {"id":2,"status":"empty"},
                        {"id":3,"status":"harvestable","plantName":"向日葵","matureInSec":0,"totalGrowTime":1200,"level":2,"currentSeason":2,"totalSeason":2}
                    ]
                }),
            ));
            this.friends.detail_loaded_at = 1_790_000_000;
            this.friends.countdown_now_sec = 1_790_000_000;
        }
        if matches!(
            case.scene,
            "settings"
                | "settings-automation"
                | "settings-offline"
                | "settings-dingtalk"
                | "settings-offline-bound"
                | "settings-system"
                | "settings-ui"
                | "settings-strategy-open"
        ) {
            this.settings.draft = match case.scene {
                "settings-dingtalk" => super::fixtures::settings_dingtalk_snapshot(),
                "settings-offline-bound" => super::fixtures::settings_qq_bot_bound_snapshot(),
                _ => super::fixtures::settings_snapshot(),
            };
            this.page_raw = this.settings.draft.clone();
            if case.scene == "settings" {
                this.settings.strategy_preview = Some(crate::desktop::settings::StrategyPreview::Seed {
                    required_level: 18,
                    name: "草莓".into(),
                });
            }
            if matches!(case.scene, "settings-offline" | "settings-offline-bound") {
                this.settings.qq_bot_app_id.update(cx, |state, cx| {
                    state.set_value("1900000000", window, cx);
                });
                this.settings.qq_bot_secret.update(cx, |state, cx| {
                    state.set_value("secret-value", window, cx);
                });
            }
            if case.scene == "settings-dingtalk" {
                this.settings.offline_endpoint.update(cx, |state, cx| {
                    state.set_value(
                        "https://oapi.dingtalk.com/robot/send?access_token=demo",
                        window,
                        cx,
                    );
                });
                this.settings.offline_secret.update(cx, |state, cx| {
                    state.set_value("SECdemo", window, cx);
                });
            }
            if case.scene == "settings-system" {
                this.page_raw["devicePresets"] = serde_json::json!([
                    {"id": "pc_default", "name": "默认 PC",
                        "description": "Windows 10 x64 桌面端指纹"},
                    {"id": "iphone_test", "name": "iPhone 测试",
                        "description": "iOS 17 移动端指纹"}
                ]);
            }
            this.update_setting_selector_language(window, cx);
            this.sync_system_config_inputs(window, cx);
        }
        match case.scene {
            "mall" | "mall-empty" | "mall-confirm" | "mall-confirm-pending"
            | "mall-non-svip" | "mall-notice" => {
                this.commerce.mall_slot_type = 1
            }
            "mall-svip" | "mall-svip-locked" => this.commerce.mall_slot_type = 4,
            _ => {}
        }
        if case.scene == "account-sheet" {
            this.open_account_form(None, window, cx);
        }
        if case.scene == "account-sheet-edit" {
            let record = super::fixtures::account_snapshot()["accounts"][0].clone();
            this.open_account_form(Some(record), window, cx);
        }
        if case.scene == "account-sheet-wx" {
            this.open_account_form(None, window, cx);
            this.accounts.login_tab = "wx".into();
        }
        if case.scene == "account-sheet-wx-qr" {
            this.open_account_form(None, window, cx);
            this.accounts.login_tab = "wx".into();
            this.accounts.wx_mode = "qr".into();
        }
        if case.scene == "account-sheet-wx-error" {
            this.open_account_form(None, window, cx);
            this.accounts.login_tab = "wx".into();
            this.accounts.wx_mode = "qr".into();
            this.accounts.wx_status = "二维码获取失败".into();
            this.accounts.wx_error = Some("二维码任务失败：timeout".into());
        }
        if case.scene == "account-sheet-form-error" {
            this.open_account_form(None, window, cx);
            this.accounts.form_error = Some("请输入登录 code 或粘贴完整登录链接".into());
        }
        if case.scene == "game-config-editor" {
            let record = super::fixtures::game_config_snapshot()["seeds"][0].clone();
            this.open_game_config_form(Some(record), window, cx);
        }
        if case.scene == "game-config-editor-new" {
            this.open_game_config_form(None, window, cx);
        }
        if case.scene == "game-config-editor-fruit" {
            let record = super::fixtures::game_config_snapshot()["fruits"][0].clone();
            this.open_game_config_form(Some(record), window, cx);
        }
        if case.scene == "game-config-editor-item" {
            let record = super::fixtures::game_config_snapshot()["items"][0].clone();
            this.open_game_config_form(Some(record), window, cx);
        }
        if case.scene == "accounts-filter-empty" {
            this.accounts.keyword_applied = "不存在的账号".into();
        }
        if case.scene == "game-config-filter-empty" {
            this.game_config.applied_keyword = "不存在的种子".into();
        }
        if case.scene == "game-config-select-open" {
            this.game_config.search_open = true;
        }
        if case.scene == "game-config-types-open" {
            this.game_config.search_open = true;
        }
        if case.scene == "mall-notice" {
            this.commerce.notice = Some(("购买成功".into(), true));
        }
        if case.scene == "friends-feedback" {
            this.friends.feedback =
                Some(("黑名单已更新".into(), crate::desktop::FriendFeedbackLevel::Success));
        }
        if case.scene == "bag-notice" {
            this.farm.bag.notice = Some(("出售成功".into(), true));
        }
        if case.scene == "friends-detail-error" {
            this.friends.detail_error = Some((101, "后台任务已结束：连接已关闭".into()));
        }
        if case.scene == "game-config-search" {
            this.game_config.search_open = true;
        }
        if case.scene == "game-config-columns" {
            this.game_config.column_picker_open = true;
        }
        if case.scene == "accounts-search" {
            this.accounts.search_open = true;
        }
        if case.scene == "accounts-columns" {
            this.accounts.column_picker_open = true;
        }
        if case.scene == "mall-confirm" {
            let goods = super::fixtures::mall_snapshot()["catalog"]["goods"][0].clone();
            this.open_purchase_confirmation(goods, window, cx);
        }
        if case.scene == "mall-confirm-pending" {
            let goods = super::fixtures::mall_snapshot()["catalog"]["goods"][0].clone();
            this.open_purchase_confirmation(goods, window, cx);
            this.commerce.purchase_pending = true;
        }
        if case.scene == "bag-selection" {
            this.farm.bag.batch_mode = true;
            this.farm.bag.selected_ids = [1020001_i64, 1020145_i64].into_iter().collect();
        }
        if case.scene == "mystery-confirm" {
            let goods = super::fixtures::mystery_shop_snapshot()["npc"].clone();
            this.open_purchase_confirmation(goods, window, cx);
        }
        if case.scene == "pets-logs" {
            this.farm.pets.logs = vec![
                serde_json::json!({"friendName": "偷菜小能手", "timestamp": 1_790_000_000_i64,
                    "stolenCount": 2, "protectedGold": 100}),
                serde_json::json!({"friendName": "热心邻居", "timestamp": 1_789_996_400_i64,
                    "stolenCount": 1, "protectedGold": 45}),
            ];
            this.show_personal_pet_logs(window, cx);
        }
        if case.scene == "illustrated-mutants" {
            this.farm.illustrated.book = "mutant".into();
        }
        if case.scene == "interaction" {
            this.sync_personal_interaction_picker(window, cx);
        }
        if case.scene == "pets" || case.scene == "pets-logs" {
            this.sync_personal_pet_food_inputs(window, cx);
        }
        if case.scene == "friends-interaction" {
            this.sync_friend_interaction_picker(window, cx);
        }
        this.sync_search_placeholder(window, cx);
        cx.notify();
    });
}

pub(super) fn arrange(
    case: &PreviewCase,
    view: &Entity<FarmDesktop>,
    window: &mut Window,
    cx: &mut App,
) {
    if case.scene == "dashboard-logs" {
        view.update(cx, |this, _| {
            this.dashboard.logs_scroll.scroll_to_bottom();
        });
    }
    if case.scene == "dashboard-logs-expanded" {
        view.update(cx, |this, _| {
            this.dashboard.logs_scroll.scroll_to_bottom();
        });
        window.click("dashboard-show-older-logs", cx);
        view.update(cx, |this, _| {
            this.dashboard.logs_scroll.scroll_to_bottom();
        });
    }
    if case.scene == "game-config-select-open" {
        window.within("game-config-season-select").click("input", cx);
    }
    if case.scene == "game-config-types-open" {
        window.within("game-config-item-type-select").click("input", cx);
    }
    if case.scene == "settings-strategy-open" {
        window.within("planting-strategy-select").click("input", cx);
    }
    if case.scene == "friends-blacklist-confirm" {
        window.click(("friend-block", 0_usize), cx);
    }
    if case.scene == "friends-unblacklist-confirm" {
        window.click(("friend-unblock", 0_usize), cx);
    }
    if case.scene == "accounts-selection" {
        window.click("account-select-1", cx);
        window.click("account-select-3", cx);
    }
    if case.scene == "game-config-selection" {
        window.click("game-config-select-0", cx);
        window.click("game-config-select-1", cx);
    }
    if case.scene == "account-delete-confirm" {
        window.click("account-delete-1", cx);
    }
    if case.scene == "game-config-delete-confirm" {
        window.click(("config-delete", 0_usize), cx);
    }
    if case.scene == "friends-delete-confirm" {
        window.click(("friend-delete", 0_usize), cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{cases, page_for_scene};
    use crate::pages::Page;

    #[test]
    fn preview_scenes_cover_every_primary_page_in_both_themes() {
        let cases = cases();
        for page in Page::ALL {
            for mode in ["light", "dark"] {
                assert!(
                    cases.iter().any(|case| {
                        page_for_scene(case.scene) == page && case.mode.name() == mode
                    }),
                    "missing {mode} preview for {page:?}"
                );
            }
        }
    }

    #[test]
    fn preview_scenes_cover_every_primary_page_in_english() {
        let cases = cases();
        for page in Page::ALL {
            assert!(
                cases.iter().any(|case| {
                    case.name.ends_with("-english") && page_for_scene(case.scene) == page
                }),
                "missing English preview for {page:?}"
            );
        }
    }

    #[test]
    fn preview_scenes_cover_every_visible_page_section_in_all_conditions() {
        let cases = cases();
        for page in Page::ALL {
            if matches!(page, Page::Accounts | Page::Dashboard) {
                // These pages hide the section tabs.
                continue;
            }
            for (section, _) in crate::desktop::page_sections(page) {
                for suffix in ["-light", "-dark", "-english"] {
                    assert!(
                        cases.iter().any(|case| {
                            case.name.ends_with(suffix)
                                && page_for_scene(case.scene) == page
                                && super::section_for_scene(case.scene) == *section
                        }),
                        "missing {suffix} preview for {page:?}/{section}"
                    );
                }
            }
        }
    }
}
