use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Personal,
    Friends,
    Activities,
    Analytics,
    Mall,
    MysteryShop,
    Settings,
    GameConfig,
    Accounts,
}

impl Page {
    pub const ALL: [Self; 10] = [
        Self::Dashboard,
        Self::Personal,
        Self::Friends,
        Self::Activities,
        Self::Analytics,
        Self::Mall,
        Self::MysteryShop,
        Self::Settings,
        Self::GameConfig,
        Self::Accounts,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "总览",
            Self::Personal => "我的农场",
            Self::Friends => "好友",
            Self::Activities => "活动",
            Self::Analytics => "收益分析",
            Self::Mall => "商城",
            Self::MysteryShop => "神秘商店",
            Self::Settings => "设置",
            Self::GameConfig => "游戏配置",
            Self::Accounts => "账号",
        }
    }

    pub fn title_for(self, language: &str) -> &'static str {
        if language == "en-US" {
            match self {
                Self::Dashboard => "Overview",
                Self::Personal => "My Farm",
                Self::Friends => "Friends",
                Self::Activities => "Activities",
                Self::Analytics => "Earnings",
                Self::Mall => "Mall",
                Self::MysteryShop => "Mystery Shop",
                Self::Settings => "Settings",
                Self::GameConfig => "Game Data",
                Self::Accounts => "Accounts",
            }
        } else {
            self.title()
        }
    }

    pub fn section_title_for(self, section: &str, language: &str) -> Option<&'static str> {
        let chinese = match self {
            Self::Dashboard => match section {
                "all" => "全部",
                "logs" => "运行日志",
                "warnings" => "提醒",
                _ => return None,
            },
            Self::Personal => match section {
                "lands" => "我的农场",
                "bag" => "我的背包",
                "dailyGifts" => "我的任务",
                "pets" => "宠物",
                "illustrated" => "图鉴",
                "interactionItems" => "互动道具",
                _ => return None,
            },
            Self::Friends => match section {
                "friends" => "好友列表",
                "blacklist" => "好友黑名单",
                "records" => "最近访客",
                "interactionItems" => "互动道具",
                _ => return None,
            },
            Self::Activities => match section {
                "directory" => "活动目录",
                "season" => "赛季战令",
                "constellation" => "星座",
                "shop" => "星砂商店",
                "solarTerms" => "节气",
                "qingmei" => "青梅",
                "qixi" => "七夕",
                "charity" => "公益",
                "petDiary" => "萌宠日记",
                "autumn" => "秋日",
                "weather" => "天气",
                _ => return None,
            },
            Self::Analytics => match section {
                "strategy" => "种植策略",
                "plantBlacklist" => "种植黑名单",
                _ => return None,
            },
            Self::Mall => match section {
                "normal" => "普通商城",
                "svip" => "SVIP 商城",
                "mystery" => "神秘商人",
                _ => return None,
            },
            Self::MysteryShop => return None,
            Self::Settings => match section {
                "strategy" => "策略设置",
                "automation" => "自动控制",
                "offlineReminder" => "下线提醒",
                "systemConfig" => "运行环境",
                "ui" => "界面偏好",
                _ => return None,
            },
            Self::GameConfig => match section {
                "seeds" => "种子",
                "fruits" => "果实",
                "items" => "道具",
                _ => return None,
            },
            Self::Accounts => return None,
        };
        if language != "en-US" {
            return Some(chinese);
        }
        Some(match (self, section) {
            (Self::Dashboard, "all") => "All",
            (Self::Dashboard, "logs") => "Activity log",
            (Self::Dashboard, "warnings") => "Alerts",
            (Self::Personal, "lands") => "Farm",
            (Self::Personal, "bag") => "My Bag",
            (Self::Personal, "dailyGifts") => "My Tasks",
            (Self::Personal, "pets") => "Pets",
            (Self::Personal, "illustrated") => "Illustrated",
            (Self::Personal, "interactionItems") | (Self::Friends, "interactionItems") => {
                "Interaction"
            }
            (Self::Friends, "friends") => "Friends",
            (Self::Friends, "blacklist") => "Blacklist",
            (Self::Friends, "records") => "Recent Visitors",
            (Self::Activities, "directory") => "Activity directory",
            (Self::Activities, "season") => "Season pass",
            (Self::Activities, "constellation") => "Constellation",
            (Self::Activities, "shop") => "Star Sand Shop",
            (Self::Activities, "solarTerms") => "Solar terms",
            (Self::Activities, "qingmei") => "Green plum",
            (Self::Activities, "qixi") => "Qixi Festival",
            (Self::Activities, "charity") => "Charity",
            (Self::Activities, "petDiary") => "Pet diary",
            (Self::Activities, "autumn") => "Autumn",
            (Self::Activities, "weather") => "Weather",
            (Self::Analytics, "strategy") => "Planting strategy",
            (Self::Settings, "strategy") => "Strategy settings",
            (Self::Analytics, "plantBlacklist") => "Plant blacklist",
            (Self::Mall, "normal") => "General shop",
            (Self::Mall, "svip") => "SVIP shop",
            (Self::Mall, "mystery") => "Mystery merchant",
            (Self::Settings, "automation") => "Auto control",
            (Self::Settings, "friendBlacklist") => "Friend rules",
            (Self::Settings, "offlineReminder") => "Offline Alert",
            (Self::Settings, "systemConfig") => "Runtime",
            (Self::Settings, "ui") => "Interface preferences",
            (Self::GameConfig, "seeds") => "Seeds",
            (Self::GameConfig, "fruits") => "Fruits",
            (Self::GameConfig, "items") => "Items",
            _ => return None,
        })
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Personal => "personal",
            Self::Friends => "friends",
            Self::Activities => "activities",
            Self::Analytics => "analytics",
            Self::Mall => "mall",
            Self::MysteryShop => "mystery-shop",
            Self::Settings => "settings",
            Self::GameConfig => "game-config",
            Self::Accounts => "accounts",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "dashboard" => Some(Self::Dashboard),
            "personal" => Some(Self::Personal),
            "friends" => Some(Self::Friends),
            "activities" => Some(Self::Activities),
            "analytics" => Some(Self::Analytics),
            "mall" => Some(Self::Mall),
            "mystery-shop" => Some(Self::MysteryShop),
            "settings" => Some(Self::Settings),
            "game-config" => Some(Self::GameConfig),
            "accounts" => Some(Self::Accounts),
            _ => None,
        }
    }

    pub const fn icon(self) -> gpui_kit::assets::IconName {
        use gpui_kit::assets::IconName;
        match self {
            Self::Dashboard => IconName::House,
            Self::Personal => IconName::Wheat,
            Self::Friends => IconName::Users,
            Self::Activities => IconName::Activity,
            Self::Analytics => IconName::ChartNoAxesCombined,
            Self::Mall => IconName::ShoppingCart,
            Self::MysteryShop => IconName::Store,
            Self::Settings => IconName::Settings,
            Self::GameConfig => IconName::SlidersHorizontal,
            Self::Accounts => IconName::UserRound,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisplayRow {
    pub id: String,
    pub section: String,
    pub title: String,
    pub detail: String,
    pub state: String,
    pub record: Value,
}

#[derive(Debug, Clone)]
pub struct PageData {
    pub summary: String,
    pub rows: Vec<DisplayRow>,
    pub raw: Value,
}
