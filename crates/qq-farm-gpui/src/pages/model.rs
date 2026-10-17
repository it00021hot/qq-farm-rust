use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Personal,
    Activities,
    SolarCalendar,
    MilkTea,
    ShareReward,
    Friends,
    Analytics,
    Mall,
    MysteryShop,
    Settings,
    GameConfig,
    Accounts,
}

impl Page {
    pub const ALL: [Self; 13] = [
        Self::Dashboard,
        Self::Personal,
        Self::Activities,
        Self::SolarCalendar,
        Self::MilkTea,
        Self::ShareReward,
        Self::Friends,
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
            Self::Activities => "活动中心",
            Self::SolarCalendar => "节令日历",
            Self::MilkTea => "奶茶活动",
            Self::ShareReward => "快乐不独享",
            Self::Friends => "好友",
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
                Self::Activities => "Activity Center",
                Self::SolarCalendar => "Solar Calendar",
                Self::MilkTea => "Milk Tea",
                Self::ShareReward => "Share & Reward",
                Self::Friends => "Friends",
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
            Self::MysteryShop | Self::Activities => return None,
            Self::Settings => match section {
                "strategy" => "策略设置",
                "automation" => "自动控制",
                "offlineReminder" => "下线提醒",
                "systemConfig" => "运行环境",
                "ui" => "系统设置",
                _ => return None,
            },
            Self::GameConfig => match section {
                "seeds" => "种子",
                "fruits" => "果实",
                "items" => "道具",
                _ => return None,
            },
            Self::Accounts => return None,
            Self::SolarCalendar => return None,
            Self::MilkTea => match section {
                "shop" => "我的店铺",
                "workshop" => "奶茶工坊",
                "hireList" => "好友雇佣",
                "work" => "我的工作",
                "exchange" => "兑换商店",
                "fund" => "成长基金",
                "rank" => "排行榜",
                "logs" => "营业记录",
                _ => return None,
            },
            Self::ShareReward => return None,
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
            (Self::Settings, "ui") => "System settings",
            (Self::GameConfig, "seeds") => "Seeds",
            (Self::GameConfig, "fruits") => "Fruits",
            (Self::GameConfig, "items") => "Items",
            (Self::MilkTea, "shop") => "My shop",
            (Self::MilkTea, "workshop") => "Workshop",
            (Self::MilkTea, "hireList") => "Hire friends",
            (Self::MilkTea, "work") => "My work",
            (Self::MilkTea, "exchange") => "Exchange",
            (Self::MilkTea, "fund") => "Growth fund",
            (Self::MilkTea, "rank") => "Ranking",
            (Self::MilkTea, "logs") => "Sales log",
            _ => return None,
        })
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Personal => "personal",
            Self::Activities => "activities",
            Self::SolarCalendar => "solar-calendar",
            Self::MilkTea => "milk-tea",
            Self::ShareReward => "share-reward",
            Self::Friends => "friends",
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
            "activities" => Some(Self::Activities),
            "solar-calendar" => Some(Self::SolarCalendar),
            "milk-tea" => Some(Self::MilkTea),
            "share-reward" => Some(Self::ShareReward),
            "friends" => Some(Self::Friends),
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
            Self::Activities => IconName::Gift,
            Self::SolarCalendar => IconName::CalendarDays,
            Self::MilkTea => IconName::CupSoda,
            Self::ShareReward => IconName::Gift,
            Self::Friends => IconName::Users,
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
