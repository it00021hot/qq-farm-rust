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

/// 种子信息（plant 中提取的子集，对齐 TS getAllSeeds）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedInfo {
    pub seed_id: i64,
    pub name: String,
    pub required_level: i64,
    pub plant_id: i64,
    #[serde(default)]
    pub price: i64,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub seasons: i64,
    #[serde(default)]
    pub exp: i64,
    #[serde(default)]
    pub grow_phases: String,
    #[serde(default)]
    pub grow_time: i64,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub harvest_count: i64,
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
    #[serde(default)]
    pub sell_cond: Option<serde_json::Value>,
    #[serde(default)]
    pub cond_sells: Option<serde_json::Value>,
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

    /// 加载所有 JSON（embed + 运行时 overlay）
    pub fn load(&self) {
        self.load_plants();
        self.load_items();
        self.load_lands();
        self.load_role_levels();
        self.apply_overlay();
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

    fn overlay_dir() -> std::path::PathBuf {
        crate::config::paths::get_data_dir().join("game_config")
    }

    fn apply_overlay(&self) {
        self.merge_overlay_plants();
        self.merge_overlay_items();
    }

    fn merge_overlay_plants(&self) {
        let path = Self::overlay_dir().join("Plant.json");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(overlay) = serde_json::from_str::<Vec<Plant>>(&text) else {
            return;
        };
        let mut guard = self.plant_map.write();
        let Some(plants) = guard.as_mut() else { return };
        for p in overlay {
            if let Some(existing) = plants.iter_mut().find(|x| x.id == p.id) {
                *existing = p;
            } else {
                plants.push(p);
            }
        }
    }

    fn merge_overlay_items(&self) {
        let path = Self::overlay_dir().join("ItemInfo.json");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(overlay) = serde_json::from_str::<Vec<Item>>(&text) else {
            return;
        };
        let mut guard = self.item_map.write();
        let Some(items) = guard.as_mut() else { return };
        for it in overlay {
            if let Some(existing) = items.iter_mut().find(|x| x.id == it.id) {
                *existing = it;
            } else {
                items.push(it);
            }
        }
    }

    fn write_json(path: &std::path::Path, value: &impl Serialize) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "[]".into());
        std::fs::write(path, text)
    }

    /// 写入植物 overlay 并热更新内存
    pub fn upsert_plant(&self, plant: Plant) -> std::io::Result<()> {
        let path = Self::overlay_dir().join("Plant.json");
        let mut overlay: Vec<Plant> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        overlay.retain(|p| p.id != plant.id);
        overlay.push(plant);
        Self::write_json(&path, &overlay)?;
        self.load_plants();
        self.merge_overlay_plants();
        Ok(())
    }

    /// 删除 overlay 植物并重载
    pub fn delete_plant_overlay(&self, plant_id: i64) -> std::io::Result<bool> {
        let path = Self::overlay_dir().join("Plant.json");
        let mut overlay: Vec<Plant> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let before = overlay.len();
        overlay.retain(|p| p.id != plant_id);
        Self::write_json(&path, &overlay)?;
        self.load_plants();
        self.merge_overlay_plants();
        Ok(overlay.len() < before)
    }

    /// 写入物品 overlay 并热更新内存
    pub fn upsert_item(&self, item: Item) -> std::io::Result<()> {
        let path = Self::overlay_dir().join("ItemInfo.json");
        let mut overlay: Vec<Item> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        overlay.retain(|i| i.id != item.id);
        overlay.push(item);
        Self::write_json(&path, &overlay)?;
        self.load_items();
        self.merge_overlay_items();
        Ok(())
    }

    /// 删除 overlay 物品并重载
    pub fn delete_item_overlay(&self, item_id: i64) -> std::io::Result<bool> {
        let path = Self::overlay_dir().join("ItemInfo.json");
        let mut overlay: Vec<Item> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let before = overlay.len();
        overlay.retain(|i| i.id != item_id);
        Self::write_json(&path, &overlay)?;
        self.load_items();
        self.merge_overlay_items();
        Ok(overlay.len() < before)
    }

    /// 保存配置图片（base64），返回相对 URL
    pub fn save_config_image_base64(name: &str, b64: &str) -> std::io::Result<String> {
        let trimmed = b64.split(',').next_back().unwrap_or(b64).trim();
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(trimmed)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        Self::save_config_image(name, &bytes)
    }

    /// 保存配置图片，返回相对 URL
    pub fn save_config_image(name: &str, bytes: &[u8]) -> std::io::Result<String> {
        let safe: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .collect();
        if safe.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid image name",
            ));
        }
        let dir = Self::overlay_dir().join("images");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(&safe), bytes)?;
        Ok(format!("/api/config/images/{safe}"))
    }

    /// 读取配置图片
    pub fn read_config_image(name: &str) -> Option<Vec<u8>> {
        let safe: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .collect();
        std::fs::read(Self::overlay_dir().join("images").join(safe)).ok()
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
        let plants = self.plant_map.read().clone().unwrap_or_default();
        plants
            .iter()
            .filter_map(|x| {
                x.seed_id.map(|sid| SeedInfo {
                    seed_id: sid,
                    name: x.name.clone(),
                    required_level: x.land_level_need.unwrap_or(1),
                    plant_id: x.id,
                    price: self.get_seed_price(sid),
                    image: mapped_item_image(sid),
                    seasons: x.seasons.unwrap_or(1),
                    exp: x.exp.unwrap_or(0),
                    grow_phases: x.grow_phases.clone().unwrap_or_default(),
                    grow_time: self.get_plant_grow_time(x.id),
                    size: x.size.unwrap_or(0),
                    harvest_count: x.fruit.as_ref().map(|f| f.count).unwrap_or(0),
                })
            })
            .collect()
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

    /// 按网格坐标查土地配置（对齐 TS `getLandConfigByCoordinate`）
    #[must_use]
    pub fn get_land_config_by_coordinate(&self, grid_x: i64, grid_y: i64) -> Option<Land> {
        self.land_map.read().as_ref().and_then(|lands| {
            lands.iter().find(|l| l.grid_x == grid_x && l.grid_y == grid_y).cloned()
        })
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

    /// 种子商店价格（读 ItemInfo `sells`，与果实同一套解析）
    #[must_use]
    pub fn get_seed_price(&self, seed_id: i64) -> i64 {
        if let Some(price) = self.parse_sells_price(seed_id) {
            return price;
        }
        self.get_plant_by_seed_id(seed_id).and_then(|p| p.fruit).map(|f| f.count * 10).unwrap_or(0)
    }

    /// 果实出售价格（从 ItemInfo.json 的 sells 字段读取）
    #[must_use]
    pub fn get_fruit_price(&self, fruit_id: i64) -> i64 {
        if let Some(price) = self.parse_sells_price(fruit_id) {
            return price;
        }
        self.get_item_by_id(fruit_id).and_then(|i| i.level).unwrap_or(1) * 8
    }

    fn parse_sells_price(&self, item_id: i64) -> Option<i64> {
        let item = self.get_item_by_id(item_id)?;
        let sells = item.sells.as_ref()?;
        let arr = sells.as_array()?;
        let obj = arr.first()?.as_object()?;
        obj.get("count").and_then(|v| v.as_i64())
    }

    /// 物品图标 URL（对齐 TS `/game-config/seed_images_named/{id}.png`）
    #[must_use]
    pub fn get_item_image_by_id(&self, item_id: i64) -> Option<String> {
        let url = mapped_item_image(item_id);
        if url.is_empty() {
            None
        } else {
            Some(url)
        }
    }

    /// 种子图标 URL（与物品同一路径规则）
    #[must_use]
    pub fn get_seed_image_by_seed_id(&self, seed_id: i64) -> Option<String> {
        self.get_item_image_by_id(seed_id)
    }

    /// 解析 sells 字符串（如 "1:100;1002:50"）→ [(currency_id, price)]
    #[must_use]
    pub fn parse_sells(&self, sells: &str) -> Vec<(i64, i64)> {
        if sells.is_empty() {
            return vec![];
        }
        sells
            .split(';')
            .filter_map(|part| {
                let mut iter = part.splitn(2, ':');
                let cid = iter.next()?.parse::<i64>().unwrap_or(0);
                let price = iter.next()?.parse::<i64>().unwrap_or(0);
                if cid == 0 && price == 0 {
                    None
                } else {
                    Some((cid, price))
                }
            })
            .collect()
    }

    /// 解析 ItemInfo.json 里的 sells / cond_sells（字符串或 null）
    #[must_use]
    pub fn parse_sells_value(&self, sells: Option<&serde_json::Value>) -> Vec<(i64, i64)> {
        let Some(value) = sells else {
            return vec![];
        };
        if let Some(s) = value.as_str() {
            return self.parse_sells(s);
        }
        vec![]
    }

    /// 对齐 TS `getEffectiveSellInfo`：条件满足时用 `cond_sells`。
    #[must_use]
    pub fn get_effective_sell_info(&self, item: &Item) -> EffectiveSellInfo {
        self.get_effective_sell_info_at(
            item,
            &crate::config::sell_conditions::SellConditionContext::now(
                crate::utils::time::get_server_time_secs(),
            ),
            0,
        )
    }

    /// 带过期时间与窗口上下文的出售判定。
    #[must_use]
    pub fn get_effective_sell_info_at(
        &self,
        item: &Item,
        ctx: &crate::config::sell_conditions::SellConditionContext,
        expire_time: i64,
    ) -> EffectiveSellInfo {
        let mut ctx = ctx.clone();
        ctx.expire_time = expire_time;
        let normal: Vec<(i64, i64)> = self
            .parse_sells_value(item.sells.as_ref())
            .into_iter()
            .filter(|(cid, price)| *cid > 0 && *price > 0)
            .collect();
        let condition = json_trimmed_string(item.sell_cond.as_ref());
        let conditional: Vec<(i64, i64)> = self
            .parse_sells_value(item.cond_sells.as_ref())
            .into_iter()
            .filter(|(cid, price)| *cid > 0 && *price > 0)
            .collect();
        if condition.as_ref().is_some_and(|s| !s.is_empty())
            && !conditional.is_empty()
            && crate::config::sell_conditions::is_sell_condition_satisfied(
                condition.as_deref().unwrap_or(""),
                &ctx,
            )
        {
            return EffectiveSellInfo {
                sellable: true,
                status: "available",
                condition,
                sells: conditional,
            };
        }
        if !normal.is_empty() {
            return EffectiveSellInfo {
                sellable: true,
                status: "available",
                condition,
                sells: normal,
            };
        }
        if condition.as_ref().is_some_and(|s| !s.is_empty()) && !conditional.is_empty() {
            return EffectiveSellInfo {
                sellable: false,
                status: "conditional",
                condition,
                sells: vec![],
            };
        }
        EffectiveSellInfo { sellable: false, status: "unavailable", condition, sells: vec![] }
    }

    /// 按物品 id 解析可售信息
    #[must_use]
    pub fn get_effective_sell_info_by_id(&self, item_id: i64) -> EffectiveSellInfo {
        self.get_effective_sell_info_by_id_at(item_id, 0)
    }

    /// 按物品 id + 堆叠过期时间解析可售信息。
    #[must_use]
    pub fn get_effective_sell_info_by_id_at(&self, item_id: i64, expire_time: i64) -> EffectiveSellInfo {
        match self.get_item_by_id(item_id) {
            Some(item) => self.get_effective_sell_info_at(
                &item,
                &crate::config::sell_conditions::SellConditionContext::now(
                    crate::utils::time::get_server_time_secs(),
                ),
                expire_time,
            ),
            None => EffectiveSellInfo {
                sellable: false,
                status: "unavailable",
                condition: None,
                sells: vec![],
            },
        }
    }
}

/// 对齐 TS `EffectiveSellInfo`
#[derive(Debug, Clone, Default)]
pub struct EffectiveSellInfo {
    pub sellable: bool,
    pub status: &'static str,
    pub condition: Option<String>,
    pub sells: Vec<(i64, i64)>,
}

fn json_trimmed_string(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    let text = if let Some(s) = value.as_str() {
        s.trim().to_string()
    } else if value.is_null() {
        String::new()
    } else {
        value.to_string()
    };
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

// ===== 全局单例 =====

/// 面板图标 URL，对齐 `gameConfig.ts` 的 `getItemImageById`。
#[must_use]
pub fn mapped_item_image(item_id: i64) -> String {
    if item_id <= 0 {
        String::new()
    } else {
        format!("/game-config/seed_images_named/{item_id}.png")
    }
}

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

/// 加载所有种子（便捷函数）
#[must_use]
pub fn load_seeds_config() -> Vec<SeedInfo> {
    let cfg = GameConfig::new();
    let _ = cfg.load();
    cfg.get_all_seeds()
}

/// 加载所有植物
#[must_use]
pub fn load_plants_config() -> Vec<Plant> {
    let cfg = GameConfig::new();
    let _ = cfg.load();
    cfg.get_all_plants()
}

/// 加载所有物品
#[must_use]
pub fn load_items_config() -> Vec<Item> {
    let cfg = GameConfig::new();
    let _ = cfg.load();
    cfg.get_all_items()
}

/// 按类型加载物品
#[must_use]
pub fn load_items_by_type_config(item_type: i64) -> Vec<Item> {
    let cfg = GameConfig::new();
    let _ = cfg.load();
    cfg.get_items_by_type(item_type)
}

/// 加载所有物品类型（type → count）
#[must_use]
pub fn load_item_types_config() -> std::collections::HashMap<i64, i64> {
    let cfg = GameConfig::new();
    let _ = cfg.load();
    let items = cfg.get_all_items();
    let mut out = std::collections::HashMap::new();
    for it in items {
        *out.entry(it.item_type).or_insert(0) += 1;
    }
    out
}

/// 加载所有果实（PlantFruit）
#[must_use]
pub fn load_fruits_config() -> Vec<PlantFruit> {
    let cfg = GameConfig::new();
    let _ = cfg.load();
    cfg.get_all_plants().into_iter().filter_map(|p| p.fruit).collect()
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
        let one = seeds.iter().find(|s| s.seed_id == 29999).unwrap();
        assert_eq!(one.image, "/game-config/seed_images_named/29999.png");
    }

    #[test]
    fn item_image_url_matches_panel_path() {
        assert_eq!(mapped_item_image(0), "");
        assert_eq!(mapped_item_image(1), "/game-config/seed_images_named/1.png");
        let gc = reload_for_test();
        assert_eq!(
            gc.get_item_image_by_id(10000).as_deref(),
            Some("/game-config/seed_images_named/10000.png")
        );
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
        let by_coord = gc.get_land_config_by_coordinate(land.grid_x, land.grid_y).expect("coord");
        assert_eq!(by_coord.id, 1);
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

    #[test]
    fn golden_fruit_sellable_from_sells_string() {
        let gc = reload_for_test();
        let item = gc
            .get_all_items()
            .into_iter()
            .find(|i| i.item_type == 17 && i.sells.as_ref().and_then(|v| v.as_str()).is_some())
            .expect("type 17 fruit with sells");
        let info = gc.get_effective_sell_info(&item);
        assert!(info.sellable, "{} should be sellable", item.name);
        assert_eq!(info.status, "available");
        assert!(!info.sells.is_empty());
    }

    #[test]
    fn conditional_sells_unlock_after_activity_ends() {
        use crate::config::activity_windows::{clear_activity_windows_for_test, set_activity_windows, ActivityWindow};
        use crate::config::sell_conditions::SellConditionContext;

        clear_activity_windows_for_test();
        set_activity_windows(vec![ActivityWindow {
            id: "2026081800".into(),
            name: "鹊桥寄情".into(),
            begin_time: 1,
            end_time: 50,
        }]);
        let mut item = Item::default();
        item.sells = None;
        item.sell_cond = Some(serde_json::Value::String("活动结束后:2026081800".into()));
        item.cond_sells = Some(serde_json::Value::String("1:100".into()));
        let gc = GameConfig::default();
        let before = gc.get_effective_sell_info_at(
            &item,
            &SellConditionContext { now_sec: 40, expire_time: 0, activity_windows_loaded: true },
            0,
        );
        assert!(!before.sellable);
        assert_eq!(before.status, "conditional");
        let after = gc.get_effective_sell_info_at(
            &item,
            &SellConditionContext { now_sec: 60, expire_time: 0, activity_windows_loaded: true },
            0,
        );
        assert!(after.sellable);
        assert_eq!(after.sells, vec![(1, 100)]);
        clear_activity_windows_for_test();
    }
}
