//! 游戏静态配置（1:1 翻译原 `core/src/config/gameConfig.ts`）。
//!
//! 加载 `assets/game_config/*.json`（编译时 embed 进二进制）。
//! 提供植物/物品/土地/等级表的查询 API。
//!
//! ## 阶段 1E-0 范围（本文件）
//!
//! - 4 个核心 struct：[`Plant`] / [`Item`] / [`Land`] / [`RoleLevel`]
//! - JSON 加载（编译时 `include_str!` + 运行时 `serde_json::from_str`）
//! - 全局 [`GameConfig`] 单例 + 多种查询函数
//!
//! ## 阶段 1E+ 范围（待办）
//!
//! - 完整 50+ 查询 API 1:1 翻译
//! - 运行时评分缓存（`runtimePlantScoreMap`）
//! - 活动积分学习（`learnActivityPlant`）

use std::sync::OnceLock;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

// ===== Plant =====

/// 植物详情
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plant {
    pub id: i64,
    pub name: String,
    pub seed_id: Option<i64>,
    pub fruit: Option<PlantFruit>,
    pub land_level_need: Option<i64>,
    pub seasons: Option<i64>,
    pub grow_phases: Option<String>,
    pub exp: Option<i64>,
    pub size: Option<i64>,
    pub exp_root: Option<f64>,
    pub exp_alter: Option<f64>,
    pub fruit_root: Option<f64>,
    pub fruit_alter: Option<f64>,
    pub harvest_animation: Option<String>,
    pub mature_effect: Option<String>,
    pub special_fruit: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlantFruit {
    pub id: i64,
    pub count: i64,
}

/// 种子信息（plant 中提取的子集）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeedInfo {
    pub seed_id: i64,
    pub name: String,
    pub required_level: i64,
    pub plant_id: i64,
}

// ===== Item =====

/// 物品详情
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Item {
    pub id: i64,
    #[serde(rename = "type")]
    pub item_type: i64,
    pub name: String,
    pub interaction_type: Option<String>,
    pub sells: Option<serde_json::Value>,
    pub level: Option<i64>,
    pub target_id: Option<i64>,
    pub asset_name: Option<String>,
    pub icon_res: Option<String>,
    pub max_count: Option<i64>,
    pub can_use: Option<i64>,
    pub desc: Option<String>,
    pub rarity: Option<i64>,
    #[serde(rename = "trait_id ")]
    pub trait_id: Option<i64>,
}

// ===== Land =====

/// 土地配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Land {
    pub id: i64,
    pub preceding_land_id: Option<i64>,
    pub unlocked_res: Option<String>,
    pub level_need: Option<i64>,
    pub gold_need: Option<i64>,
    pub grid_x: i64,
    pub grid_y: i64,
    pub grid_x2: i64,
    pub grid_y2: i64,
    pub can_share: Option<bool>,
}

// ===== RoleLevel =====

/// 等级经验表
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleLevel {
    pub level: i64,
    pub exp: i64,
}

// ===== GameConfig 单例 =====

/// 全局游戏配置（加载一次，到处用）
pub struct GameConfig {
    /// 植物列表（按 id 索引）
    plant_map: RwLock<Option<Vec<Plant>>>,
    /// 物品列表
    item_map: RwLock<Option<Vec<Item>>>,
    /// 土地配置
    land_map: RwLock<Option<Vec<Land>>>,
    /// 等级经验表
    role_level: RwLock<Option<Vec<RoleLevel>>>,
}

/// 默认 GameConfig（未加载）
impl Default for GameConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl GameConfig {
    /// 创建（未加载）
    #[must_use]
    pub fn new() -> Self {
        Self {
            plant_map: RwLock::new(None),
            item_map: RwLock::new(None),
            land_map: RwLock::new(None),
            role_level: RwLock::new(None),
        }
    }

    /// 加载所有 JSON
    pub fn load(&self) {
        self.load_plants();
        self.load_items();
        self.load_lands();
        self.load_role_levels();
    }

    fn load_plants(&self) {
        let json = include_str!("../../../../assets/game_config/Plant.json");
        let plants: Vec<Plant> = serde_json::from_str(json).expect("Plant.json parse failed");
        *self.plant_map.write() = Some(plants);
    }

    fn load_items(&self) {
        let json = include_str!("../../../../assets/game_config/ItemInfo.json");
        let items: Vec<Item> = serde_json::from_str(json).expect("ItemInfo.json parse failed");
        *self.item_map.write() = Some(items);
    }

    fn load_lands(&self) {
        let json = include_str!("../../../../assets/game_config/Land.json");
        let lands: Vec<Land> = serde_json::from_str(json).expect("Land.json parse failed");
        *self.land_map.write() = Some(lands);
    }

    fn load_role_levels(&self) {
        let json = include_str!("../../../../assets/game_config/RoleLevel.json");
        let levels: Vec<RoleLevel> =
            serde_json::from_str(json).expect("RoleLevel.json parse failed");
        *self.role_level.write() = Some(levels);
    }

    // ===== 查询 API =====

    /// 按 plant_id 查植物
    #[must_use]
    pub fn get_plant_by_id(&self, plant_id: i64) -> Option<Plant> {
        self.plant_map.read().as_ref().and_then(|p| p.iter().find(|x| x.id == plant_id).cloned())
    }

    /// 按 seed_id 查植物
    #[must_use]
    pub fn get_plant_by_seed_id(&self, seed_id: i64) -> Option<Plant> {
        self.plant_map
            .read()
            .as_ref()
            .and_then(|p| p.iter().find(|x| x.seed_id == Some(seed_id)).cloned())
    }

    /// 按 fruit_id 查植物
    #[must_use]
    pub fn get_plant_by_fruit_id(&self, fruit_id: i64) -> Option<Plant> {
        self.plant_map.read().as_ref().and_then(|p| {
            p.iter().find(|x| x.fruit.as_ref().map_or(false, |f| f.id == fruit_id)).cloned()
        })
    }

    /// 植物名
    #[must_use]
    pub fn get_plant_name(&self, plant_id: i64) -> String {
        self.get_plant_by_id(plant_id).map(|p| p.name).unwrap_or_default()
    }

    /// 按 seed_id 查植物名
    #[must_use]
    pub fn get_plant_name_by_seed_id(&self, seed_id: i64) -> String {
        self.get_plant_by_seed_id(seed_id).map(|p| p.name).unwrap_or_default()
    }

    /// 果实名
    #[must_use]
    pub fn get_fruit_name(&self, fruit_id: i64) -> String {
        // 果实也是 Item 的一种
        self.get_item_by_id(fruit_id).map(|i| i.name).unwrap_or_default()
    }

    /// 植物经验值
    #[must_use]
    pub fn get_plant_exp(&self, plant_id: i64) -> i64 {
        self.get_plant_by_id(plant_id).and_then(|p| p.exp).unwrap_or(0)
    }

    /// 植物生长时间（秒，解析 `grow_phases: "种子:30;发芽:30;成熟:0;"`）
    ///
    /// 求和所有阶段的秒数（成熟阶段一般是 0 终点）。
    #[must_use]
    pub fn get_plant_grow_time(&self, plant_id: i64) -> i64 {
        let Some(plant) = self.get_plant_by_id(plant_id) else {
            return 0;
        };
        let Some(phases) = plant.grow_phases.as_ref() else {
            return 0;
        };
        // 格式：`name:secs;name:secs;...`
        let mut total = 0i64;
        for part in phases.split(';') {
            if part.is_empty() {
                continue;
            }
            if let Some((_, secs_str)) = part.split_once(':') {
                if let Ok(secs) = secs_str.parse::<i64>() {
                    total += secs;
                }
            }
        }
        total
    }

    /// 格式化生长时间为人类可读字符串
    #[must_use]
    pub fn format_grow_time(&self, secs: i64) -> String {
        if secs <= 0 {
            return "0秒".to_string();
        }
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        if h > 0 {
            format!("{h}小时{m}分{s}秒")
        } else if m > 0 {
            format!("{m}分{s}秒")
        } else {
            format!("{s}秒")
        }
    }

    /// 全部种子（从 plant 中提取有 seed_id 的）
    #[must_use]
    pub fn get_all_seeds(&self) -> Vec<SeedInfo> {
        self.plant_map
            .read()
            .as_ref()
            .map(|p| {
                p.iter()
                    .filter_map(|x| {
                        x.seed_id.map(|sid| SeedInfo {
                            seed_id: sid,
                            name: x.name.clone(),
                            required_level: x.land_level_need.unwrap_or(1),
                            plant_id: x.id,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 全部植物
    #[must_use]
    pub fn get_all_plants(&self) -> Vec<Plant> {
        self.plant_map.read().clone().unwrap_or_default()
    }

    /// 按 item_id 查物品
    #[must_use]
    pub fn get_item_by_id(&self, item_id: i64) -> Option<Item> {
        self.item_map
            .read()
            .as_ref()
            .and_then(|items| items.iter().find(|i| i.id == item_id).cloned())
    }

    /// 全部物品
    #[must_use]
    pub fn get_all_items(&self) -> Vec<Item> {
        self.item_map.read().clone().unwrap_or_default()
    }

    /// 按 type 过滤物品
    #[must_use]
    pub fn get_items_by_type(&self, item_type: i64) -> Vec<Item> {
        self.item_map
            .read()
            .as_ref()
            .map(|items| items.iter().filter(|i| i.item_type == item_type).cloned().collect())
            .unwrap_or_default()
    }

    /// 按 land_id 查土地配置
    #[must_use]
    pub fn get_land_config_by_id(&self, land_id: i64) -> Option<Land> {
        self.land_map
            .read()
            .as_ref()
            .and_then(|lands| lands.iter().find(|l| l.id == land_id).cloned())
    }

    /// 全部土地配置
    #[must_use]
    pub fn get_all_land_configs(&self) -> Vec<Land> {
        self.land_map.read().clone().unwrap_or_default()
    }

    /// 经验表（按 level 索引）
    #[must_use]
    pub fn get_level_exp_table(&self) -> Vec<i64> {
        self.role_level
            .read()
            .as_ref()
            .map(|levels| {
                let mut sorted: Vec<RoleLevel> = levels.clone();
                sorted.sort_by_key(|x| x.level);
                sorted.iter().map(|x| x.exp).collect()
            })
            .unwrap_or_default()
    }

    /// 给定等级 + 总经验，返回当前级内进度
    ///
    /// 表语义：`table[i]` = 升到 level `i+1` 所需的累计经验。
    /// 因此：当前等级 `L` 已有 `table[L-1]`，下一等级需要 `table[L]`。
    /// 返回 `(当前级内经验, 本级总需经验)`。
    #[must_use]
    pub fn get_level_exp_progress(&self, level: i64, total_exp: i64) -> (i64, i64) {
        let table = self.get_level_exp_table();
        if table.is_empty() {
            return (0, 0);
        }
        // 升到当前 level 所需累计经验
        let cur = if level >= 1 && (level as usize) <= table.len() {
            table[(level as usize) - 1]
        } else {
            0
        };
        // 升到下一 level 所需累计经验
        let next = if (level as usize) < table.len() { table[level as usize] } else { cur };
        let needed = next - cur;
        (total_exp - cur, needed)
    }

    /// 种子商店价格
    #[must_use]
    pub fn get_seed_price(&self, seed_id: i64) -> i64 {
        // 简化：植物的 fruit.count * 10（占位）
        self.get_plant_by_seed_id(seed_id).and_then(|p| p.fruit).map(|f| f.count * 10).unwrap_or(0)
    }
}

// ===== 全局单例 =====

static GLOBAL: OnceLock<GameConfig> = OnceLock::new();

/// 获取全局 GameConfig（首次调用时自动 load）
pub fn global() -> &'static GameConfig {
    GLOBAL.get_or_init(|| {
        let gc = GameConfig::new();
        gc.load();
        gc
    })
}

/// 强制重新加载（仅测试用）
#[cfg(test)]
pub fn reload_for_test() -> GameConfig {
    let gc = GameConfig::new();
    gc.load();
    gc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_data() {
        let gc = reload_for_test();
        assert!(!gc.get_all_plants().is_empty());
        assert!(!gc.get_all_items().is_empty());
        assert!(!gc.get_all_land_configs().is_empty());
        assert!(!gc.get_level_exp_table().is_empty());
    }

    #[test]
    fn get_plant_by_id_works() {
        let gc = reload_for_test();
        // Plant.json 第一条 id=2020002 (白萝卜)
        let plant = gc.get_plant_by_id(2020002).expect("plant 2020002");
        assert_eq!(plant.name, "白萝卜");
        assert_eq!(plant.seed_id, Some(29999));
        assert_eq!(plant.land_level_need, Some(1));
    }

    #[test]
    fn get_plant_by_seed_id_works() {
        let gc = reload_for_test();
        let plant = gc.get_plant_by_seed_id(29999).expect("plant by seed_id 29999");
        assert_eq!(plant.id, 2020002);
    }

    #[test]
    fn get_plant_grow_time_parses_phases() {
        let gc = reload_for_test();
        // 白萝卜 grow_phases: "种子:30;发芽:30;成熟:0;"
        let secs = gc.get_plant_grow_time(2020002);
        assert_eq!(secs, 60); // 30+30，最后非零
    }

    #[test]
    fn get_all_seeds_extracts() {
        let gc = reload_for_test();
        let seeds = gc.get_all_seeds();
        assert!(!seeds.is_empty());
        assert!(seeds.iter().any(|s| s.seed_id == 29999));
    }

    #[test]
    fn get_item_by_id_works() {
        let gc = reload_for_test();
        let item = gc.get_item_by_id(10000).expect("item 10000");
        assert!(!item.name.is_empty());
    }

    #[test]
    fn get_level_exp_progress_works() {
        let gc = reload_for_test();
        let (current, needed) = gc.get_level_exp_progress(1, 50);
        // level 1 需 0 exp, level 2 需 100 exp
        assert_eq!(current, 50);
        assert_eq!(needed, 100);
    }

    #[test]
    fn get_land_config_by_id_works() {
        let gc = reload_for_test();
        let land = gc.get_land_config_by_id(1).expect("land 1");
        assert_eq!(land.grid_x, 0);
        assert_eq!(land.grid_y, 5);
    }

    #[test]
    fn format_grow_time_human() {
        let gc = reload_for_test();
        assert_eq!(gc.format_grow_time(0), "0秒");
        assert_eq!(gc.format_grow_time(30), "30秒");
        assert_eq!(gc.format_grow_time(60), "1分0秒");
        assert_eq!(gc.format_grow_time(3661), "1小时1分1秒");
    }

    #[test]
    fn global_singleton() {
        let g1 = global();
        let g2 = global();
        assert!(std::ptr::eq(g1, g2));
    }
}
