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
    /// 变异展示植株映射链（格式 `5:1120112:1;5_6:1129001:1`）
    #[serde(default)]
    pub mutant_effect_plant: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlantFruit {
    pub id: i64,
    pub count: i64,
}

/// 生长阶段配置项（对齐 bot `PlantGrowPhase`）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlantGrowPhase {
    pub index: usize,
    pub name: String,
    pub duration: i64,
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
    /// 资源所在 bundle（bot 46bb710 图片映射键的一部分）
    #[serde(default)]
    pub bundle_name: Option<String>,
    pub max_count: Option<i64>,
    pub can_use: Option<i64>,
    pub desc: Option<String>,
    #[serde(rename = "effectDesc")]
    pub effect_desc: Option<String>,
    pub rarity: Option<i64>,
    #[serde(rename = "trait_id ")]
    pub trait_id: Option<i64>,
    #[serde(default)]
    pub activity_id: Option<i64>,
    #[serde(default)]
    pub to_see: Option<i64>,
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

/// 图鉴条目（Illustrated.json）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IllustratedEntry {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub illustrated_type: String,
    pub param: i64,
    #[serde(default)]
    pub sort: i64,
}

/// 超变升级 buff 配置（BuffCfg.json）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuffConfigItem {
    pub id: i64,
    pub source_type: String,
    pub source_param: i64,
    #[serde(deserialize_with = "string_or_number_to_string")]
    pub attr_id: String,
    pub attr_value: i64,
}

/// 官方配置里大量字段会显式写 `null`（serde `default` 只兜缺失键，不兜 null）
fn null_to_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    let value: Option<T> = Option::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

/// 官方 BuffCfg 新批次的「活动加成」条目 attr_id 直接写数字 ID（「超变升级」条目仍是字符串
/// 名），bot 侧以 `String(entry.attr_id || '')` 归一，这里对齐为 string|number|null 三容忍。
fn string_or_number_to_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        serde_json::Value::Null => Ok(String::new()),
        other => Err(serde::de::Error::custom(format!("unexpected attr_id value: {other:?}"))),
    }
}

/// 变异效果原始配置（MutantEffect.json；可空字段一律 null 容错）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MutantEffectEntry {
    pub id: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub effect_name: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub icon: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub description: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub tips: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub tag: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub activity_id: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub effect_type: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub hide_ui: bool,
}

/// 变异效果展示 DTO（对齐 bot `toMutantEffectDto`，camelCase 序列化）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutantEffectDto {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub tag: String,
    pub activity_id: i64,
}

/// 运行时 MutantEffect 未下发 tips/description 的补充说明（对齐 bot）
const MUTANT_EFFECT_DESCRIPTION_FALLBACKS: &[(i64, &str)] =
    &[(13, "特殊活动变异，收获时可额外获得鹊羽。")];

/// 图标取路径末段（`gui/.../crystal/spriteFrame` → `crystal`）
fn normalize_mutant_icon_name(value: &str) -> String {
    let raw = value.trim().trim_end_matches("/spriteFrame");
    raw.split('/').rfind(|s| !s.is_empty()).unwrap_or("").to_string()
}

fn to_mutant_effect_dto(effect: Option<&MutantEffectEntry>, id: i64) -> MutantEffectDto {
    let numeric_id = if id != 0 { id } else { effect.map(|e| e.id).unwrap_or(0) };
    let Some(effect) = effect else {
        return MutantEffectDto {
            id: numeric_id,
            name: if numeric_id > 0 {
                format!("变异 #{numeric_id}")
            } else {
                "变异".to_string()
            },
            icon: String::new(),
            description: String::new(),
            tag: String::new(),
            activity_id: 0,
        };
    };
    // 名称以官方运行时配置的 effect_name 为准；name 可能是“喜鹊事件”等内部事件名
    let name = if !effect.effect_name.is_empty() {
        effect.effect_name.clone()
    } else if !effect.name.is_empty() {
        effect.name.clone()
    } else if numeric_id > 0 {
        format!("变异 #{numeric_id}")
    } else {
        "变异".to_string()
    };
    let description = if !effect.description.is_empty() {
        effect.description.clone()
    } else if !effect.tips.is_empty() {
        effect.tips.clone()
    } else {
        MUTANT_EFFECT_DESCRIPTION_FALLBACKS
            .iter()
            .find(|(fid, _)| *fid == numeric_id)
            .map(|(_, text)| (*text).to_string())
            .unwrap_or_default()
    };
    MutantEffectDto {
        id: numeric_id,
        name,
        icon: normalize_mutant_icon_name(&effect.icon),
        description,
        tag: effect.tag.clone(),
        activity_id: effect.activity_id,
    }
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
    /// 图鉴条目（按 param 索引）
    illustrated: RwLock<Option<Vec<IllustratedEntry>>>,
    /// 超变 buff 配置
    buffs: RwLock<Option<Vec<BuffConfigItem>>>,
    /// 变异效果配置（MutantEffect.json）
    mutant_effects: RwLock<Option<Vec<MutantEffectEntry>>>,
    /// 物品图片索引（bot 46bb710：磁盘存在才发 URL，icon_res/asset_name 同键复用）
    item_image_index: RwLock<ItemImageIndex>,
}

/// 物品图片索引（对齐 bot `gameConfig.ts` 46bb710 的三个集合）。
#[derive(Default, Debug, Clone)]
struct ItemImageIndex {
    /// 磁盘上存在图片文件的物品 ID → 扩展名（rust 资源统一 webp，同步工具可能带 png）
    file_ext: std::collections::HashMap<i64, &'static str>,
    /// `bundle:icon_res` → 拥有该图标的物品 ID（仅登记自身图片存在的物品）
    by_icon: std::collections::HashMap<String, i64>,
    /// `bundle:asset_name` → 拥有该模型的物品 ID（无 icon_res 才登记）
    by_asset: std::collections::HashMap<String, i64>,
}

/// bot `getItemImageKey`：`bundle:resource`（去尾部 `/spriteFrame`，resource 为空则无键）。
fn item_image_key(item: &Item, resource: Option<&str>) -> Option<String> {
    let resource = resource?.trim();
    let resource = resource.strip_suffix("/spriteFrame").unwrap_or(resource);
    if resource.is_empty() {
        return None;
    }
    Some(format!("{}:{resource}", item.bundle_name.as_deref().unwrap_or("").trim()))
}

/// 扫描图片目录构建索引；目录不可读时索引为空（记 warn，与 bot 同为降级）。
fn scan_item_image_index(items: &[Item]) -> ItemImageIndex {
    let mut index = ItemImageIndex::default();
    let dir = crate::config::paths::game_config_static_dir()
        .join("seed_images_named")
        .join("seed_images");
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(dir = %dir.display(), error = %e, "物品图片目录读取失败，物品图标将不可见");
            return index;
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        for (ext, parsed) in
            [("webp", name.strip_suffix(".webp")), ("png", name.strip_suffix(".png"))]
        {
            if let Some(id) = parsed.and_then(|base| base.parse::<i64>().ok()).filter(|id| *id > 0)
            {
                // webp 优先：同 ID 双文件时以 rust 惯用格式为准
                index.file_ext.entry(id).or_insert(ext);
            }
        }
    }
    // bot：按 ItemInfo 顺序登记 icon_res / asset_name 键（仅自身图片存在的物品）
    for item in items {
        if !index.file_ext.contains_key(&item.id) {
            continue;
        }
        if let Some(key) = item_image_key(item, item.icon_res.as_deref()) {
            index.by_icon.entry(key).or_insert(item.id);
        } else if let Some(key) = item_image_key(item, item.asset_name.as_deref()) {
            index.by_asset.entry(key).or_insert(item.id);
        }
    }
    index
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
            illustrated: RwLock::new(None),
            buffs: RwLock::new(None),
            mutant_effects: RwLock::new(None),
            item_image_index: RwLock::new(ItemImageIndex::default()),
        }
    }

    /// 加载所有 JSON（embed + 运行时 overlay）
    pub fn load(&self) {
        self.load_plants();
        self.load_items();
        self.load_lands();
        self.load_role_levels();
        self.load_illustrated();
        self.load_buffs();
        self.load_mutant_effects();
        self.apply_overlay();
        // 图片目录只在加载配置时扫描一次（bot 46bb710：避免列表请求反复访问磁盘）
        let items = self.item_map.read().clone().unwrap_or_default();
        *self.item_image_index.write() = scan_item_image_index(&items);
    }

    fn load_plants(&self) {
        let json = include_str!("../../../../assets/game_config/Plant.json");
        match serde_json::from_str::<Vec<Plant>>(json) {
            Ok(plants) => *self.plant_map.write() = Some(plants),
            Err(e) => {
                tracing::error!(error = %e, "Plant.json parse failed; using empty list");
                *self.plant_map.write() = Some(Vec::new());
            }
        }
    }

    fn load_items(&self) {
        let json = include_str!("../../../../assets/game_config/ItemInfo.json");
        match serde_json::from_str::<Vec<Item>>(json) {
            Ok(items) => *self.item_map.write() = Some(items),
            Err(e) => {
                tracing::error!(error = %e, "ItemInfo.json parse failed; using empty list");
                *self.item_map.write() = Some(Vec::new());
            }
        }
    }

    fn load_lands(&self) {
        let json = include_str!("../../../../assets/game_config/Land.json");
        match serde_json::from_str::<Vec<Land>>(json) {
            Ok(lands) => *self.land_map.write() = Some(lands),
            Err(e) => {
                tracing::error!(error = %e, "Land.json parse failed; using empty list");
                *self.land_map.write() = Some(Vec::new());
            }
        }
    }

    fn load_role_levels(&self) {
        let json = include_str!("../../../../assets/game_config/RoleLevel.json");
        match serde_json::from_str::<Vec<RoleLevel>>(json) {
            Ok(levels) => *self.role_level.write() = Some(levels),
            Err(e) => {
                tracing::error!(error = %e, "RoleLevel.json parse failed; using empty list");
                *self.role_level.write() = Some(Vec::new());
            }
        }
    }

    fn load_illustrated(&self) {
        let json = include_str!("../../../../assets/game_config/Illustrated.json");
        match serde_json::from_str::<Vec<IllustratedEntry>>(json) {
            Ok(entries) => *self.illustrated.write() = Some(entries),
            Err(e) => {
                tracing::error!(error = %e, "Illustrated.json parse failed; using empty list");
                *self.illustrated.write() = Some(Vec::new());
            }
        }
    }

    fn load_buffs(&self) {
        let json = include_str!("../../../../assets/game_config/BuffCfg.json");
        match serde_json::from_str::<Vec<BuffConfigItem>>(json) {
            Ok(items) => *self.buffs.write() = Some(items),
            Err(e) => {
                tracing::error!(error = %e, "BuffCfg.json parse failed; using empty list");
                *self.buffs.write() = Some(Vec::new());
            }
        }
    }

    fn load_mutant_effects(&self) {
        let json = include_str!("../../../../assets/game_config/MutantEffect.json");
        match serde_json::from_str::<Vec<MutantEffectEntry>>(json) {
            Ok(items) => {
                tracing::info!(count = items.len(), "已加载变异效果配置");
                *self.mutant_effects.write() = Some(items);
            }
            Err(e) => {
                tracing::error!(error = %e, "MutantEffect.json parse failed; using empty list");
                *self.mutant_effects.write() = Some(Vec::new());
            }
        }
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
            p.iter().find(|x| x.fruit.as_ref().is_some_and(|f| f.id == fruit_id)).cloned()
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
        self.get_plant_grow_phases(plant_id).iter().map(|p| p.duration).sum()
    }

    /// 解析官方 `Plant.grow_phases`（对齐 bot `getPlantGrowPhases`）。
    ///
    /// 官方客户端把每项转换成内部 phase_id；服务端响应的 phases
    /// 是从当前阶段开始的配置后缀。配置缺失或为空时返回空 Vec。
    #[must_use]
    pub fn get_plant_grow_phases(&self, plant_id: i64) -> Vec<PlantGrowPhase> {
        let Some(plant) = self.get_plant_by_id(plant_id) else {
            return Vec::new();
        };
        let Some(phases) = plant.grow_phases.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (index, part) in phases.split(';').filter(|p| !p.is_empty()).enumerate() {
            // 名称里可能含 ':'，以最后一个冒号分隔（对齐 TS lastIndexOf）
            let (name, duration) = match part.rsplit_once(':') {
                Some((name, dur)) => (name.to_string(), dur.parse::<i64>().unwrap_or(0)),
                None => (part.to_string(), 0),
            };
            out.push(PlantGrowPhase { index, name, duration });
        }
        out
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
                    // Plant.json 的 land_level_need 全为 1；等级以 ItemInfo.level 为准（对齐 TS getAllSeeds）
                    required_level: self
                        .get_item_by_id(sid)
                        .and_then(|i| i.level)
                        .filter(|l| *l > 0)
                        .or(x.land_level_need)
                        .unwrap_or(0),
                    plant_id: x.id,
                    price: self.get_seed_price(sid),
                    // bot 46bb710：getMappedSeedImage 走 getItemImageById（存在性检查 + 映射）
                    image: self.get_item_image_by_id(sid).unwrap_or_default(),
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

    /// 按变异 ID 查效果 DTO
    #[must_use]
    pub fn get_mutant_effect_by_id(&self, mutant_id: i64) -> Option<MutantEffectDto> {
        if mutant_id <= 0 {
            return None;
        }
        let entry =
            self.mutant_effects.read().as_ref()?.iter().find(|e| e.id == mutant_id).cloned();
        Some(to_mutant_effect_dto(entry.as_ref(), mutant_id))
    }

    /// 批量查变异效果（去重、保序）
    #[must_use]
    pub fn get_mutant_effects_by_ids(&self, ids: &[i64]) -> Vec<MutantEffectDto> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for id in ids {
            if *id <= 0 || !seen.insert(*id) {
                continue;
            }
            if let Some(dto) = self.get_mutant_effect_by_id(*id) {
                result.push(dto);
            }
        }
        result
    }

    /// 变异类型名称列表（背包等展示用）
    #[must_use]
    pub fn get_mutant_type_names(&self, ids: &[i64]) -> Vec<String> {
        self.get_mutant_effects_by_ids(ids).into_iter().map(|e| e.name).collect()
    }

    /// 变异图标 URL（`seed_images_named/mutant/{id}.webp`）
    #[must_use]
    pub fn get_mutant_image_by_id(&self, mutant_id: i64) -> String {
        if mutant_id <= 0 {
            String::new()
        } else {
            format!("/game-config/seed_images_named/mutant/{mutant_id}.webp")
        }
    }

    /// 根据当前变异组合解析应展示的植物 ID（对齐 bot `getMutantDisplayPlantId`）。
    ///
    /// `mutant_effect_plant` 形如 `5:1120112:1;5_6:1129001:1`；多效果组合优先于
    /// 单效果，避免黄金+活动变异退化成普通黄金作物；最多 4 层映射。
    #[must_use]
    pub fn get_mutant_display_plant_id(&self, plant_id: i64, mutant_ids: &[i64]) -> i64 {
        if mutant_ids.is_empty() {
            return plant_id;
        }
        let active: std::collections::HashSet<i64> =
            mutant_ids.iter().copied().filter(|id| *id > 0).collect();
        if active.is_empty() {
            return plant_id;
        }
        let mut current = plant_id;
        let mut visited = std::collections::HashSet::from([current]);
        for _ in 0..4 {
            let Some(plant) = self.get_plant_by_id(current) else { break };
            let Some(mapping) = plant.mutant_effect_plant.as_deref().map(str::trim) else { break };
            if mapping.is_empty() {
                break;
            }
            let mut best: Option<(usize, i64)> = None;
            for entry in mapping.split(';') {
                let Some((effect_key, target_text)) = entry.split_once(':') else { continue };
                let effect_ids: Vec<i64> = effect_key
                    .split('_')
                    .filter_map(|s| s.trim().parse::<i64>().ok())
                    .filter(|id| *id > 0)
                    .collect();
                let Ok(target) = target_text.trim().parse::<i64>() else { continue };
                if target <= 0 || effect_ids.is_empty() {
                    continue;
                }
                if !effect_ids.iter().all(|id| active.contains(id)) {
                    continue;
                }
                if best.as_ref().is_none_or(|(count, _)| effect_ids.len() > *count) {
                    best = Some((effect_ids.len(), target));
                }
            }
            let Some((_, target)) = best else { break };
            if visited.contains(&target) {
                break;
            }
            current = target;
            visited.insert(current);
        }
        current
    }

    /// 按 param（种子/果实参数）查图鉴条目
    #[must_use]
    pub fn get_illustrated_by_param(&self, param: i64) -> Option<IllustratedEntry> {
        self.illustrated.read().as_ref()?.iter().find(|e| e.param == param).cloned()
    }

    /// 图鉴分组：装扮果实 decoration / 活动果实 activity / 其余 gold
    #[must_use]
    pub fn illustrated_mutant_group(&self, seed_id: i64) -> &'static str {
        match self.get_illustrated_by_param(seed_id).map(|e| e.kind).as_deref() {
            Some("装扮果实") => "decoration",
            Some("活动果实") => "activity",
            _ => "gold",
        }
    }

    /// 指定来源的 buff。藏品积分奖励使用「藏品属性」。
    #[must_use]
    pub fn buffs_by_source(&self, source_type: &str) -> Vec<BuffConfigItem> {
        self.buffs
            .read()
            .as_ref()
            .map(|items| {
                items.iter().filter(|buff| buff.source_type == source_type).cloned().collect()
            })
            .unwrap_or_default()
    }

    /// 超变升级 buff 全表（source_type = 超变升级）
    #[must_use]
    pub fn illustrated_buffs(&self) -> Vec<BuffConfigItem> {
        self.buffs
            .read()
            .as_ref()
            .map(|items| {
                let mut list: Vec<BuffConfigItem> =
                    items.iter().filter(|b| b.source_type == "超变升级").cloned().collect();
                list.sort_by_key(|b| b.source_param);
                list
            })
            .unwrap_or_default()
    }

    /// 当前等级生效的超变 buff（按 attr 取最高等级条目）
    #[must_use]
    pub fn illustrated_buffs_by_level(&self, level: i64) -> Vec<BuffConfigItem> {
        let mut latest: std::collections::HashMap<String, BuffConfigItem> = Default::default();
        for buff in self.illustrated_buffs() {
            if buff.source_param <= level.max(0) && !buff.attr_id.is_empty() {
                latest.insert(buff.attr_id.clone(), buff);
            }
        }
        let mut list: Vec<BuffConfigItem> = latest.into_values().collect();
        list.sort_by_key(|b| b.source_param);
        list
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

    /// 物品图标 URL（对齐 bot `getItemImageById`，46bb710）：
    /// 自身图片文件存在 → 直接用；否则按 `icon_res` / `asset_name` 键复用
    /// 同一资源的其它物品图（解锁卡用对应作物模型图）；都没有 → None。
    #[must_use]
    pub fn get_item_image_by_id(&self, item_id: i64) -> Option<String> {
        if item_id <= 0 {
            return None;
        }
        let index = self.item_image_index.read();
        let image_id = match index.file_ext.get(&item_id) {
            Some(_) => item_id,
            None => {
                let item = self.get_item_by_id(item_id)?;
                item_image_key(&item, item.icon_res.as_deref())
                    .and_then(|key| index.by_icon.get(&key).copied())
                    .or_else(|| {
                        item_image_key(&item, item.asset_name.as_deref())
                            .and_then(|key| index.by_asset.get(&key).copied())
                    })?
            }
        };
        let ext = index.file_ext.get(&image_id).copied().unwrap_or("webp");
        Some(format!("/game-config/seed_images_named/seed_images/{image_id}.{ext}"))
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
    pub fn get_effective_sell_info_by_id_at(
        &self,
        item_id: i64,
        expire_time: i64,
    ) -> EffectiveSellInfo {
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
    cfg.load();
    cfg.get_all_seeds()
}

/// 加载所有植物
#[must_use]
pub fn load_plants_config() -> Vec<Plant> {
    let cfg = GameConfig::new();
    cfg.load();
    cfg.get_all_plants()
}

/// 加载所有物品
#[must_use]
pub fn load_items_config() -> Vec<Item> {
    let cfg = GameConfig::new();
    cfg.load();
    cfg.get_all_items()
}

/// 按类型加载物品
#[must_use]
pub fn load_items_by_type_config(item_type: i64) -> Vec<Item> {
    let cfg = GameConfig::new();
    cfg.load();
    cfg.get_items_by_type(item_type)
}

/// 加载所有物品类型（type → count）
#[must_use]
pub fn load_item_types_config() -> std::collections::HashMap<i64, i64> {
    let cfg = GameConfig::new();
    cfg.load();
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
    cfg.load();
    cfg.get_all_plants().into_iter().filter_map(|p| p.fruit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isolated_config() -> GameConfig {
        let _data = crate::config::paths::TestDataDir::enter();
        reload_for_test()
    }

    #[test]
    fn loads_all_data() {
        let gc = isolated_config();
        assert!(!gc.get_all_plants().is_empty());
        assert_eq!(gc.get_plant_by_id(1020381).expect("plant").name, "鸡蛋花");
        assert!(!gc.get_all_items().is_empty());
        assert_eq!(gc.get_item_by_id(60002).expect("cub").name, "鸭");
        assert_eq!(gc.get_item_by_id(710001).expect("collection").name, "菇菇氧");
        assert!(!gc.get_all_land_configs().is_empty());
        let levels = gc.get_level_exp_table();
        assert_eq!(levels.len(), 280);
        assert_eq!(levels[200], 169_925_100);
        assert!(gc.illustrated.read().as_ref().is_some_and(|rows| {
            rows.iter().any(|row| row.id == 40_381_001 && row.param == 40_381)
        }));
        assert!(!gc.get_level_exp_table().is_empty());
    }

    #[test]
    fn buff_cfg_parses_with_numeric_attr_id() {
        // 官方新批次「活动加成」条目 attr_id 写数字 ID，「超变升级」条目仍是字符串名，
        // 两种形态（外加 null）都必须能整表解析——回归 2026-10-26 BuffCfg 空表事故。
        let json = r#"[
            {"id":1,"source_type":"超变升级","source_param":1,"attr_id":"闪电变异概率提升","attr_value":60},
            {"id":1005,"source_type":"活动加成","source_param":2026092201,"attr_id":1005,"attr_value":5000},
            {"id":1007,"source_type":"活动加成","source_param":2026092201,"attr_id":null,"attr_value":10000}
        ]"#;
        let items: Vec<BuffConfigItem> = serde_json::from_str(json).expect("parse BuffConfigItem");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].attr_id, "闪电变异概率提升");
        assert_eq!(items[1].attr_id, "1005");
        assert_eq!(items[2].attr_id, "");
    }

    #[test]
    fn embedded_buff_cfg_yields_non_empty_upgrade_buffs() {
        let gc = isolated_config();
        assert!(
            !gc.illustrated_buffs().is_empty(),
            "内嵌 BuffCfg.json 必须能整表解析出超变升级 buff"
        );
        assert!(gc.buffs_by_source("藏品属性").iter().any(|buff| buff.attr_id == "节省10%肥料"));
    }

    #[test]
    fn get_plant_by_id_works() {
        let gc = isolated_config();
        // Plant.json 第一条 id=2020002 (白萝卜)
        let plant = gc.get_plant_by_id(2020002).expect("plant 2020002");
        assert_eq!(plant.name, "白萝卜");
        assert_eq!(plant.seed_id, Some(29999));
        assert_eq!(plant.land_level_need, Some(1));
    }

    #[test]
    fn get_plant_by_seed_id_works() {
        let gc = isolated_config();
        let plant = gc.get_plant_by_seed_id(29999).expect("plant by seed_id 29999");
        assert_eq!(plant.id, 2020002);
    }

    #[test]
    fn get_plant_grow_time_parses_phases() {
        let gc = isolated_config();
        // 白萝卜 grow_phases 随官方配置更新为 "种子:1;发芽:1;成熟:0;"
        let secs = gc.get_plant_grow_time(2020002);
        assert_eq!(secs, 2); // 1+1
    }

    #[test]
    fn get_all_seeds_extracts() {
        let gc = isolated_config();
        let seeds = gc.get_all_seeds();
        assert!(!seeds.is_empty());
        assert!(seeds.iter().any(|s| s.seed_id == 29999));
        let one = seeds.iter().find(|s| s.seed_id == 29999).unwrap();
        assert_eq!(one.image, "/game-config/seed_images_named/seed_images/29999.webp");
        assert_eq!(one.required_level, 1);
        let pumpkin = seeds.iter().find(|s| s.seed_id == 29998).expect("哈哈南瓜种子");
        assert_eq!(pumpkin.required_level, 31);
    }

    #[test]
    fn item_image_url_matches_panel_path() {
        let gc = isolated_config();
        assert_eq!(gc.get_item_image_by_id(0), None);
        assert_eq!(gc.get_item_image_by_id(-1), None);
        // bot 46bb710：磁盘有图的物品直接命中（100001 是新增解锁卡图标）
        assert_eq!(
            gc.get_item_image_by_id(100_001).as_deref(),
            Some("/game-config/seed_images_named/seed_images/100001.webp")
        );
        // 磁盘无图的物品不再发死链（bot：getItemImageById 返回空）
        assert_eq!(gc.get_item_image_by_id(10_000), None);
    }

    #[test]
    fn get_item_by_id_works() {
        let gc = isolated_config();
        let item = gc.get_item_by_id(10000).expect("item 10000");
        assert!(!item.name.is_empty());
    }

    #[test]
    fn mutant_effects_load_all_entries_with_null_fields() {
        let gc = isolated_config();
        // 官方配置含大量 null(effect_type/param/activity_id/icon/tips...),必须全部加载成功
        for id in 1..=14i64 {
            let dto = gc
                .get_mutant_effect_by_id(id)
                .unwrap_or_else(|| panic!("mutant effect {id} missing"));
            assert_eq!(dto.id, id);
            assert!(!dto.name.is_empty(), "mutant {id} name empty");
        }
        // 名称以 effect_name 为准(bot 终态:喜鹊/闪电/晶辉)
        assert_eq!(gc.get_mutant_effect_by_id(13).unwrap().name, "喜鹊");
        assert_eq!(gc.get_mutant_effect_by_id(12).unwrap().name, "闪电");
        assert_eq!(gc.get_mutant_effect_by_id(14).unwrap().name, "晶辉");
        assert!(gc.get_mutant_effect_by_id(14).unwrap().description.contains("及以上"));
        assert_eq!(gc.get_mutant_effect_by_id(18).unwrap().name, "藏品");
        assert_eq!(gc.get_mutant_effect_by_id(18).unwrap().tag, "藏品");
        // id 13 icon 为 null → 空字符串;id 14 icon 路径末段 crystal
        assert_eq!(gc.get_mutant_effect_by_id(13).unwrap().icon, "");
        assert_eq!(gc.get_mutant_effect_by_id(14).unwrap().icon, "crystal");
        // id 14 activity_id 为 null → 0;id 12 属天气活动
        assert_eq!(gc.get_mutant_effect_by_id(14).unwrap().activity_id, 0);
        assert_eq!(gc.get_mutant_effect_by_id(12).unwrap().activity_id, 2_026_070_302);
    }

    #[test]
    fn get_level_exp_progress_works() {
        let gc = isolated_config();
        let (current, needed) = gc.get_level_exp_progress(1, 50);
        // level 1 需 0 exp, level 2 需 100 exp
        assert_eq!(current, 50);
        assert_eq!(needed, 100);
    }

    #[test]
    fn get_land_config_by_id_works() {
        let gc = isolated_config();
        let land = gc.get_land_config_by_id(1).expect("land 1");
        assert_eq!(land.grid_x, 0);
        assert_eq!(land.grid_y, 5);
        let by_coord = gc.get_land_config_by_coordinate(land.grid_x, land.grid_y).expect("coord");
        assert_eq!(by_coord.id, 1);
    }

    #[test]
    fn format_grow_time_human() {
        let gc = isolated_config();
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
        let gc = isolated_config();
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
        let _cache = crate::config::activity_windows::activity_windows_test_guard();
        use crate::config::activity_windows::{
            clear_activity_windows_for_test, set_activity_windows, ActivityWindow,
        };
        use crate::config::sell_conditions::SellConditionContext;

        clear_activity_windows_for_test();
        set_activity_windows(vec![ActivityWindow {
            id: "2026081800".into(),
            name: "示例活动".into(),
            begin_time: 1,
            end_time: 50,
        }]);
        let item = Item {
            sells: None,
            sell_cond: Some(serde_json::Value::String("活动结束后:2026081800".into())),
            cond_sells: Some(serde_json::Value::String("1:100".into())),
            ..Default::default()
        };
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
