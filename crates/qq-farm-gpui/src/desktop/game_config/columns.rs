#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GameConfigColumnPin {
    Left,
    Right,
    Unfixed,
}

impl GameConfigColumnPin {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Unfixed => Self::Left,
            Self::Left => Self::Right,
            Self::Right => Self::Unfixed,
        }
    }

    pub(crate) fn label(self, language: &str) -> &'static str {
        match (language == "en-US", self) {
            (true, Self::Unfixed) => "Pin left",
            (true, Self::Left) => "Pin right",
            (true, Self::Right) => "Unpin",
            (false, Self::Unfixed) => "固定左侧",
            (false, Self::Left) => "固定右侧",
            (false, Self::Right) => "取消固定",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GameConfigColumnSetting {
    pub(crate) key: &'static str,
    pub(crate) checked: bool,
    pub(crate) pin: GameConfigColumnPin,
}

impl GameConfigColumnSetting {
    const fn new(key: &'static str) -> Self {
        Self { key, checked: true, pin: GameConfigColumnPin::Unfixed }
    }
}

pub(crate) fn default_game_config_columns(section: &str) -> Vec<GameConfigColumnSetting> {
    let keys: &[&str] = match section {
        "seeds" => &[
            "seed",
            "seedId",
            "requiredLevel",
            "seasons",
            "growTime",
            "harvestCount",
            "exp",
            "price",
            "operate",
        ],
        "fruits" => &["fruit", "fruitId", "plantName", "rarity", "price", "operate"],
        "items" => &["item", "itemId", "type", "rarity", "price", "operate"],
        _ => &[],
    };
    keys.iter().copied().map(GameConfigColumnSetting::new).collect()
}

pub(crate) fn game_config_column_width(key: &str) -> Option<f32> {
    match key {
        "seed" | "fruit" | "item" => Some(190.),
        "seedId" | "itemId" => Some(92.),
        "fruitId" => Some(100.),
        "requiredLevel" => Some(98.),
        "seasons" => Some(84.),
        "growTime" => Some(100.),
        "harvestCount" => Some(100.),
        "exp" => Some(74.),
        "plantName" => Some(145.),
        "type" => Some(145.),
        "rarity" => Some(116.),
        "price" => Some(132.),
        "operate" => Some(154.),
        _ => None,
    }
}

pub(crate) fn game_config_column_title(key: &str, language: &str) -> &'static str {
    let chinese = match key {
        "seed" => "种子",
        "seedId" => "种子ID",
        "requiredLevel" => "等级",
        "seasons" => "季节",
        "growTime" => "生长时间",
        "harvestCount" => "收获数",
        "exp" => "经验",
        "fruit" => "果实",
        "fruitId" => "果实ID",
        "plantName" => "植物",
        "item" => "道具",
        "itemId" => "物品ID",
        "type" => "类型",
        "rarity" => "稀有度",
        "price" => "价格",
        "operate" => "操作",
        _ => return "",
    };
    if language != "en-US" {
        return chinese;
    }
    match key {
        "seed" => "Seed",
        "seedId" => "Seed ID",
        "requiredLevel" => "Level",
        "seasons" => "Seasons",
        "growTime" => "Grow Time",
        "harvestCount" => "Harvest",
        "exp" => "Exp",
        "fruit" => "Fruit",
        "fruitId" => "Fruit ID",
        "plantName" => "Plant",
        "item" => "Item",
        "itemId" => "Item ID",
        "type" => "Type",
        "rarity" => "Rarity",
        "price" => "Price",
        "operate" => "Operate",
        _ => "",
    }
}

pub(crate) fn move_game_config_column(
    columns: &mut [GameConfigColumnSetting],
    index: usize,
    direction: isize,
) {
    let Some(target) = index.checked_add_signed(direction) else { return };
    if target < columns.len() {
        columns.swap(index, target);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_game_config_columns, game_config_column_title, move_game_config_column,
        GameConfigColumnPin,
    };

    #[test]
    fn column_defaults_match_vue_catalogs_and_support_reordering() {
        let mut seeds = default_game_config_columns("seeds");
        assert_eq!(seeds[0].key, "seed");
        assert_eq!(seeds.last().map(|column| column.key), Some("operate"));
        assert!(seeds.iter().all(|column| column.checked));
        move_game_config_column(&mut seeds, 1, -1);
        assert_eq!(seeds[0].key, "seedId");
        move_game_config_column(&mut seeds, 0, -1);
        assert_eq!(seeds[0].key, "seedId");
        assert_eq!(default_game_config_columns("fruits").len(), 6);
        assert_eq!(default_game_config_columns("items").len(), 6);
        assert!(default_game_config_columns("unknown").is_empty());
    }

    #[test]
    fn column_pin_cycle_and_titles_follow_vue_controls_in_both_locales() {
        assert_eq!(GameConfigColumnPin::Unfixed.next(), GameConfigColumnPin::Left);
        assert_eq!(GameConfigColumnPin::Left.next(), GameConfigColumnPin::Right);
        assert_eq!(GameConfigColumnPin::Right.next(), GameConfigColumnPin::Unfixed);
        assert_eq!(GameConfigColumnPin::Unfixed.label("en-US"), "Pin left");
        assert_eq!(game_config_column_title("seedId", "zh-CN"), "种子ID");
        assert_eq!(game_config_column_title("seedId", "en-US"), "Seed ID");
    }
}
