//! 种植引擎 —— 选种子、拖动种植、按配置施肥、空地自动种植。
//!
//! 对应原 `core/src/services/farm/planting.ts`。
//!
//! - [`plant_seeds`] / [`plant_seeds_with_layouts`]：按 layout 种植并确认占地
//! - [`auto_plant_empty_lands`]：背包优先，不足再走商店选种
//! - [`fertilize_by_config`]：normal / organic / both / smart / none

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::error::Result;
use crate::proto::generated::gamepb::plantpb::PlantItem;
use crate::services::farm::api::{
    Api, NORMAL_FERTILIZER_ID,
};
use crate::services::farm::land_analysis::{
    analyze_lands, build_planting_layouts, resolve_occupied_land_ids, select_non_overlapping_layouts,
};

/// 种植策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlantingStrategy {
    #[default]
    MaxExp,
    MaxProfit,
    MaxFertExp,
    MaxFertProfit,
    Level,
    Preferred,
    BagPriority,
}

/// 施肥模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FertilizeMode {
    /// 智能（默认）—— 普通 + 有机（如果多季作物）
    #[default]
    Smart,
    /// 有机 + 普通
    Both,
    /// 仅有机
    Organic,
    /// 仅普通
    Normal,
    /// 关闭
    None,
}

/// 施肥调用选项（对齐 TS `runFertilizerByConfig` options）
#[derive(Debug, Clone, Copy, Default)]
pub struct FertilizeOptions {
    pub skip_normal: bool,
    pub multi_season: bool,
}

fn fertilizer_types_to_analysis(
    types: &[crate::models::types::FertilizerLandType],
) -> Vec<crate::services::farm::land_analysis::LandType> {
    use crate::models::types::FertilizerLandType;
    use crate::services::farm::land_analysis::LandType as AnalysisLandType;
    types
        .iter()
        .map(|t| match t {
            FertilizerLandType::Normal => AnalysisLandType::Normal,
            FertilizerLandType::Gold => AnalysisLandType::Gold,
            FertilizerLandType::Black => AnalysisLandType::Black,
            FertilizerLandType::Red => AnalysisLandType::Red,
            FertilizerLandType::PurpleGold => AnalysisLandType::PurpleGold,
        })
        .collect()
}

/// 种植配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantingConfig {
    pub enabled: bool,
    pub strategy: PlantingStrategy,
    pub preferred_seed_id: i64,
    pub fertilize_mode: FertilizeMode,
    pub auto_buy_organic: bool,
    pub auto_buy_normal: bool,
    pub organic_threshold: u32,
    pub normal_threshold: u32,
    pub organic_buy_count: u32,
    pub normal_buy_count: u32,
}

impl Default for PlantingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: PlantingStrategy::MaxExp,
            preferred_seed_id: 0,
            fertilize_mode: FertilizeMode::Smart,
            auto_buy_organic: false,
            auto_buy_normal: false,
            organic_threshold: 10,
            normal_threshold: 10,
            organic_buy_count: 50,
            normal_buy_count: 50,
        }
    }
}

/// 种子占地大小（按 `plant.size` 查；找不到则返回 1）
///
/// 对应原 planting.ts `getPlantSizeBySeedId(seedId)`
#[must_use]
pub fn get_plant_size_by_seed_id(seed_id: i64) -> usize {
    if seed_id <= 0 {
        return 1;
    }
    let cfg = crate::config::game_config::global();
    if let Some(plant) = cfg.get_plant_by_seed_id(seed_id) {
        std::cmp::max(1, plant.size.unwrap_or(1) as usize)
    } else {
        1
    }
}

/// 编码 PlantRequest（protobuf bytes 字段：seed_id + land_ids[]）
///
/// 1:1 对齐原 planting.ts `encodePlantRequest`：
/// ```text
/// message PlantRequest { PlantItem items = 2; }       // field 2, LEN
/// message PlantItem { int64 seed_id = 1; repeated int64 land_ids = 2; }  // land_ids packed
/// ```
pub fn encode_plant_request(seed_id: i64, land_ids: &[i64]) -> Vec<u8> {
    // 内层 PlantItem
    let mut item = Vec::new();
    // field 1 (seed_id), wire type 0 (varint)
    put_tag(&mut item, 1, 0);
    put_varint(&mut item, seed_id as u64);
    // field 2 (land_ids), wire type 2 (LEN)，packed
    let mut ids = Vec::new();
    for &id in land_ids {
        put_varint(&mut ids, id as u64);
    }
    put_tag(&mut item, 2, 2);
    put_varint(&mut item, ids.len() as u64);
    item.extend(ids);

    // 外层 field 2 (PlantItem), wire type 2 (LEN)
    let mut out = Vec::new();
    put_tag(&mut out, 2, 2);
    put_varint(&mut out, item.len() as u64);
    out.extend(item);
    out
}

/// 写 protobuf tag（field << 3 | wire_type，varint 编码）
fn put_tag(out: &mut Vec<u8>, field: u64, wire_type: u64) {
    put_varint(out, (field << 3) | wire_type);
}

/// 写 varint（base-128）
fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// 种植策略标签
pub const PLANTING_STRATEGY_LABELS: &[(&str, &str)] = &[
    ("preferred", "优先种植种子"),
    ("level", "最高等级作物"),
    ("max_exp", "最大经验/时"),
    ("max_fert_exp", "最大普通肥经验/时"),
    ("max_profit", "最大净利润/时"),
    ("max_fert_profit", "最大普通肥净利润/时"),
    ("bag_priority", "背包种子优先"),
];

/// 获取种植策略的中文标签
#[must_use]
pub fn get_planting_strategy_label(strategy: &str) -> String {
    PLANTING_STRATEGY_LABELS
        .iter()
        .find(|(k, _)| *k == strategy)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| strategy.to_string())
}

/// 背包种子按 priority 列表排序
///
/// 对应原 planting.ts `sortBagSeedsForPlanting(bagSeeds, priorityList)`
#[must_use]
pub fn sort_bag_seeds_for_planting(
    bag_seeds: &[BagSeedLite],
    priority_list: &[i64],
) -> Vec<BagSeedLite> {
    let mut index_map = std::collections::HashMap::new();
    for (idx, &seed_id) in priority_list.iter().enumerate() {
        if seed_id > 0 {
            index_map.insert(seed_id, idx);
        }
    }
    let mut sorted = bag_seeds.to_vec();
    sorted.sort_by(|a, b| {
        let a_idx = index_map
            .get(&a.seed_id)
            .copied()
            .unwrap_or(usize::MAX);
        let b_idx = index_map
            .get(&b.seed_id)
            .copied()
            .unwrap_or(usize::MAX);
        if a_idx != b_idx {
            return a_idx.cmp(&b_idx);
        }
        let a_level = a.required_level;
        let b_level = b.required_level;
        if a_level != b_level {
            return b_level.cmp(&a_level);
        }
        a.seed_id.cmp(&b.seed_id)
    });
    sorted
}

/// 背包种子轻量信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BagSeedLite {
    pub seed_id: i64,
    pub required_level: i64,
    pub count: i64,
}

/// 确认"种植成功"的占地是否完整
///
/// - `expected_land_ids`: 客户端请求的所有占地
/// - `master_land_id`: 服务端返回的 master
/// - `occupied_land_ids`: 服务端返回的占用
/// - `lands`: 全量土地列表（用于验证 master 有 plant）
///
/// 对应原 planting.ts `confirmsPlantedFootprint(...)`
#[must_use]
pub fn confirms_planted_footprint(
    expected_land_ids: &std::collections::HashSet<i64>,
    master_land_id: i64,
    occupied_land_ids: &[i64],
    lands: &[crate::proto::generated::gamepb::plantpb::LandInfo],
) -> bool {
    if !expected_land_ids
        .iter()
        .all(|id| occupied_land_ids.contains(id))
    {
        return false;
    }
    let land_map: std::collections::HashMap<i64, &crate::proto::generated::gamepb::plantpb::LandInfo> =
        lands.iter().map(|l| (l.id, l)).collect();
    match land_map.get(&master_land_id) {
        Some(master) => master.plant.is_some(),
        None => false,
    }
}

/// 按策略选种子
///
/// 对应原 planting.ts `findBestSeed` 顶层逻辑（无网络版）：
/// - `Preferred`：返回 config.preferred_seed_id
/// - `BagPriority`：返回 0（调用方需扫描背包）
/// - 其他（MaxExp / MaxProfit / MaxFertExp / MaxFertProfit / Level）：
///   1. 从 GameConfig 拉所有种子
///   2. 按 strategy 排序（调用 analytics.get_plant_rankings）
///   3. 返回排序后的第一个
#[must_use]
pub fn select_seed_for_strategy(config: &PlantingConfig) -> i64 {
    use crate::services::analytics::{get_plant_rankings, SortBy};
    if config.strategy == PlantingStrategy::Preferred {
        return config.preferred_seed_id;
    }
    if config.strategy == PlantingStrategy::BagPriority {
        // 调用方应走背包扫描；返回 0 表示"待选"
        return 0;
    }
    let sort_by = match config.strategy {
        PlantingStrategy::MaxExp => SortBy::Exp,
        PlantingStrategy::MaxFertExp => SortBy::Fert,
        PlantingStrategy::MaxProfit => SortBy::Profit,
        PlantingStrategy::MaxFertProfit => SortBy::FertProfit,
        PlantingStrategy::Level => SortBy::Level,
        _ => SortBy::Exp,
    };
    let rankings = get_plant_rankings(sort_by);
    for r in rankings {
        if r.seed_id > 0 {
            return r.seed_id;
        }
    }
    config.preferred_seed_id
}

/// 种植布局（多格作物的拖动序列）
pub use crate::services::farm::land_analysis::PlantingLayout;

/// 种植结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlantSeedsResult {
    pub planted: usize,
    pub planted_land_ids: Vec<i64>,
    pub occupied_land_ids: Vec<i64>,
    pub reserved_land_ids: Vec<i64>,
    pub uncertain: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutoPlantResult {
    pub planted_lands: Vec<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BagPlantResult {
    pub remaining_land_ids: Vec<i64>,
    pub fallback_allowed: bool,
    pub planted_land_ids: Vec<i64>,
    pub total_planted: usize,
    pub occupied_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShopPlantResult {
    pub planted_lands: Vec<i64>,
    pub remaining_land_ids: Vec<i64>,
    pub uncertain: bool,
}

/// 种植引擎
pub struct PlantingEngine {
    api: Api,
    config: PlantingConfig,
}

impl PlantingEngine {
    /// 创建
    #[must_use]
    pub fn new(api: Api, config: PlantingConfig) -> Self {
        Self { api, config }
    }

    /// 当前配置
    #[must_use]
    pub fn config(&self) -> &PlantingConfig {
        &self.config
    }

    /// 修改配置
    pub fn set_config(&mut self, config: PlantingConfig) {
        self.config = config;
    }

    /// 拖动种植：按 layout 循环、50ms 间隔、占地确认失败则补拉 `getAllLands`
    pub async fn plant_seeds(
        &self,
        seed_id: i64,
        land_ids: Vec<i64>,
        host_gid: i64,
    ) -> Result<PlantSeedsResult> {
        let layouts = land_ids
            .into_iter()
            .filter(|id| *id > 0)
            .map(|id| PlantingLayout {
                anchor_land_id: id,
                land_ids: vec![id],
            })
            .collect();
        self.plant_seeds_with_layouts(seed_id, layouts, usize::MAX, host_gid)
            .await
    }

    /// 带布局的种植入口（对齐 TS `plantSeeds(..., { layouts, maxPlantCount })`）
    pub async fn plant_seeds_with_layouts(
        &self,
        seed_id: i64,
        layouts: Vec<PlantingLayout>,
        max_plant_count: usize,
        host_gid: i64,
    ) -> Result<PlantSeedsResult> {
        let selected: Vec<PlantingLayout> = layouts
            .into_iter()
            .filter(|l| l.anchor_land_id > 0 && !l.land_ids.is_empty())
            .take(max_plant_count)
            .collect();
        if selected.is_empty() {
            return Ok(PlantSeedsResult::default());
        }

        let mut planted_land_ids = Vec::new();
        let mut occupied_land_ids: std::collections::HashSet<i64> =
            std::collections::HashSet::new();
        let mut reserved_land_ids: std::collections::HashSet<i64> =
            std::collections::HashSet::new();
        let mut uncertain = false;

        for (index, layout) in selected.iter().enumerate() {
            let land_id = layout.anchor_land_id;
            let items = vec![PlantItem {
                seed_id,
                land_ids: layout.land_ids.clone(),
                auto_slave: false,
            }];
            let reply = match self.api.plant(items).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(land_id, error = %e, "种植 RPC 失败");
                    uncertain = true;
                    break;
                }
            };
            let mut changed = reply.land;
            let (mut resolved_master, mut resolved_occupied) =
                resolve_occupied_land_ids(land_id, &changed);
            if resolved_master == 0 {
                resolved_master = land_id;
            }
            let expected: std::collections::HashSet<i64> =
                layout.land_ids.iter().copied().collect();
            let mut confirmed =
                confirms_planted_footprint(&expected, resolved_master, &resolved_occupied, &changed);
            if !confirmed {
                match self.api.get_all_lands(host_gid).await {
                    Ok(latest) => {
                        changed = latest.lands;
                        let resolved = resolve_occupied_land_ids(land_id, &changed);
                        resolved_master = if resolved.0 != 0 { resolved.0 } else { land_id };
                        resolved_occupied = resolved.1;
                        confirmed = confirms_planted_footprint(
                            &expected,
                            resolved_master,
                            &resolved_occupied,
                            &changed,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(land_id, error = %e, "种植成功但补拉占地失败");
                        uncertain = true;
                    }
                }
            }
            if !confirmed {
                uncertain = true;
                for id in &layout.land_ids {
                    reserved_land_ids.insert(*id);
                }
                tracing::warn!(land_id, "无法确认完整占地");
                break;
            }
            planted_land_ids.push(resolved_master);
            for id in resolved_occupied {
                occupied_land_ids.insert(id);
            }
            for id in &layout.land_ids {
                reserved_land_ids.insert(*id);
            }
            if selected.len() > 1 && index + 1 < selected.len() {
                sleep(Duration::from_millis(50)).await;
            }
        }

        Ok(PlantSeedsResult {
            planted: planted_land_ids.len(),
            planted_land_ids,
            occupied_land_ids: occupied_land_ids.into_iter().collect(),
            reserved_land_ids: reserved_land_ids.into_iter().collect(),
            uncertain,
        })
    }

    /// 按配置施肥（对齐 TS `runFertilizerByConfig`）
    pub async fn fertilize_by_config(
        &self,
        planted_land_ids: &[i64],
        host_gid: i64,
    ) -> Result<FertilizeResult> {
        self.fertilize_by_config_ex(
            planted_land_ids,
            host_gid,
            "",
            FertilizeOptions::default(),
        )
        .await
    }

    /// 带账号配置 / skipNormal / 多季原因的施肥
    pub async fn fertilize_by_config_ex(
        &self,
        planted_land_ids: &[i64],
        _host_gid: i64,
        account_id: &str,
        options: FertilizeOptions,
    ) -> Result<FertilizeResult> {
        use crate::models::types::FertilizerMode;
        use crate::services::farm::land_analysis::{
            filter_ids_by_land_types, get_fast_mature_lands,
            get_organic_fertilizer_targets_from_lands, ALL_FERTILIZER_LAND_TYPES,
        };

        let auto = if account_id.is_empty() {
            None
        } else {
            Some(crate::models::store::account_config::get_automation(Some(
                account_id,
            )))
        };
        let mode = auto
            .as_ref()
            .map(|a| a.fertilizer)
            .unwrap_or(match self.config.fertilize_mode {
                FertilizeMode::None => FertilizerMode::None,
                FertilizeMode::Normal => FertilizerMode::Normal,
                FertilizeMode::Organic => FertilizerMode::Organic,
                FertilizeMode::Both => FertilizerMode::Both,
                FertilizeMode::Smart => FertilizerMode::Smart,
            });
        if matches!(mode, FertilizerMode::None) {
            return Ok(FertilizeResult::default());
        }

        let selected = auto
            .as_ref()
            .map(|a| fertilizer_types_to_analysis(&a.fertilizer_land_types))
            .unwrap_or_else(|| ALL_FERTILIZER_LAND_TYPES.to_vec());
        if selected.is_empty() {
            return Ok(FertilizeResult::default());
        }

        let planted: Vec<i64> = {
            let mut seen = std::collections::HashSet::new();
            planted_land_ids
                .iter()
                .copied()
                .filter(|id| *id > 0 && seen.insert(*id))
                .collect()
        };
        if planted.is_empty()
            && !matches!(
                mode,
                FertilizerMode::Organic | FertilizerMode::Both | FertilizerMode::Smart
            )
        {
            return Ok(FertilizeResult::default());
        }

        let latest_lands = self
            .api
            .get_all_lands(0)
            .await
            .map(|r| r.lands)
            .unwrap_or_default();
        // 拉地失败/空列表时 fail-closed：无法确认土地类型则跳过本轮施肥（对齐 bot）
        if latest_lands.is_empty() {
            return Ok(FertilizeResult::default());
        }

        let normal_targets = filter_ids_by_land_types(&planted, &latest_lands, &selected);

        let mut result = FertililzeResultBuilder::default();
        if !options.skip_normal
            && matches!(
                mode,
                FertilizerMode::Normal | FertilizerMode::Both | FertilizerMode::Smart
            )
            && !normal_targets.is_empty()
        {
            for (i, &land_id) in normal_targets.iter().enumerate() {
                if self
                    .api
                    .fertilize(land_id, NORMAL_FERTILIZER_ID)
                    .await
                    .is_err()
                {
                    break;
                }
                result.normal += 1;
                if i + 1 < normal_targets.len() {
                    sleep(Duration::from_millis(50)).await;
                }
            }
        }

        if matches!(mode, FertilizerMode::Organic | FertilizerMode::Both) {
            let mut organic_targets = planted.clone();
            if !latest_lands.is_empty() {
                organic_targets = get_organic_fertilizer_targets_from_lands(&latest_lands);
                organic_targets = filter_ids_by_land_types(&organic_targets, &latest_lands, &selected);
            }
            result.organic = self.api.fertilize_organic_loop(&organic_targets).await;
        } else if matches!(mode, FertilizerMode::Smart) {
            let smart_secs = auto
                .as_ref()
                .map(|a| a.fertilizer_smart_seconds)
                .filter(|n| *n > 0)
                .unwrap_or(300);
            let lands = if latest_lands.is_empty() {
                self.api.get_all_lands(0).await.map(|r| r.lands).unwrap_or_default()
            } else {
                latest_lands
            };
            let organic_targets = get_fast_mature_lands(&lands, smart_secs);
            if !organic_targets.is_empty() {
                result.organic = self.api.fertilize_organic_loop(&organic_targets).await;
            }
        }

        if result.normal + result.organic > 0 && !account_id.is_empty() {
            crate::services::stats::record_operation_for(
                account_id,
                "fertilize",
                (result.normal + result.organic) as i64,
            );
        }
        Ok(result.build())
    }

    /// 收获指定土地
    pub async fn harvest(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<usize> {
        let n = land_ids.len();
        let _ = self.api.harvest(land_ids, host_gid, true).await?;
        Ok(n)
    }

    /// 铲除植物
    pub async fn remove_plant(&self, land_ids: Vec<i64>) -> Result<usize> {
        let n = land_ids.len();
        let _ = self.api.remove_plant(land_ids).await?;
        Ok(n)
    }

    /// 浇水
    pub async fn water(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<()> {
        self.api.water_land(land_ids, host_gid).await?;
        Ok(())
    }

    /// 锄地
    pub async fn farm(&self, land_ids: Vec<i64>, host_gid: i64) -> Result<()> {
        self.api.farming(land_ids, host_gid).await?;
        Ok(())
    }

    /// 自动种空地（背包优先或商店选种）
    pub async fn auto_plant_empty_lands(
        &self,
        dead_land_ids: &[i64],
        empty_land_ids: &[i64],
        host_gid: i64,
        account_id: &str,
    ) -> Result<AutoPlantResult> {
        let mut lands_to_plant: Vec<i64> = {
            let mut seen = std::collections::HashSet::new();
            empty_land_ids
                .iter()
                .copied()
                .filter(|id| *id > 0 && seen.insert(*id))
                .collect()
        };
        if !dead_land_ids.is_empty() {
            if self.api.remove_plant(dead_land_ids.to_vec()).await.is_ok() {
                crate::services::panel_log::log(
                    account_id,
                    "铲除",
                    format!("清理枯株 {} 块土地", dead_land_ids.len()),
                    crate::constants::PanelEvent::RemovePlant, Some(serde_json::json!({
                        "module": "farm", 
                        "result": "ok",
                        "count": dead_land_ids.len(),
                        "landIds": dead_land_ids,
                    })),
                );
                if let Ok(latest) = self.api.get_all_lands(host_gid).await {
                    lands_to_plant = analyze_lands(&latest.lands, host_gid).empty;
                }
            }
        }
        if lands_to_plant.is_empty() {
            return Ok(AutoPlantResult::default());
        }

        let strategy = crate::models::store::account_config::get_planting_strategy(Some(account_id));
        if strategy == crate::models::types::PlantingStrategy::BagPriority {
            let bag = match self
                .plant_from_bag_seeds(&lands_to_plant, host_gid, account_id)
                .await
            {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "读取背包种子失败，本轮跳过第二优先策略以避免误购"
                    );
                    return Ok(AutoPlantResult::default());
                }
            };
            let mut planted = bag.planted_land_ids.clone();
            if bag.fallback_allowed && !bag.remaining_land_ids.is_empty() {
                let fallback =
                    crate::models::store::account_config::get_bag_seed_fallback_strategy(Some(
                        account_id,
                    ));
                let shop = self
                    .plant_from_shop(
                        &bag.remaining_land_ids,
                        host_gid,
                        account_id,
                        Some(fallback),
                    )
                    .await?;
                planted.extend(shop.planted_lands);
            }
            planted.sort_unstable();
            planted.dedup();
            if !planted.is_empty() {
                let _ = self
                    .fertilize_by_config_ex(
                        &planted,
                        host_gid,
                        account_id,
                        FertilizeOptions::default(),
                    )
                    .await;
            }
            return Ok(AutoPlantResult { planted_lands: planted });
        }

        let shop = self
            .plant_from_shop(&lands_to_plant, host_gid, account_id, Some(strategy))
            .await?;
        if !shop.planted_lands.is_empty() {
            let _ = self
                .fertilize_by_config_ex(
                    &shop.planted_lands,
                    host_gid,
                    account_id,
                    FertilizeOptions::default(),
                )
                .await;
        }
        Ok(AutoPlantResult {
            planted_lands: shop.planted_lands,
        })
    }

    /// 用背包种子种植
    pub async fn plant_from_bag_seeds(
        &self,
        lands_to_plant: &[i64],
        host_gid: i64,
        account_id: &str,
    ) -> Result<BagPlantResult> {
        let mut target: Vec<i64> = {
            let mut seen = std::collections::HashSet::new();
            lands_to_plant
                .iter()
                .copied()
                .filter(|id| *id > 0 && seen.insert(*id))
                .collect()
        };
        if target.is_empty() {
            return Ok(BagPlantResult::default());
        }
        let warehouse =
            crate::services::warehouse::WarehouseService::new(self.api.gateway().clone());
        let bag_seeds = warehouse.get_bag_seeds().await?;
        let state_level = crate::services::status::status_data_for(account_id).level;
        let mapped: Vec<BagSeedWithLevel> = bag_seeds
            .iter()
            .map(|s| BagSeedWithLevel {
                seed_id: s.seed_id,
                name: s.name.clone(),
                count: s.count,
                required_level: s.required_level,
                plant_size: s.plant_size.max(1) as usize,
                state_level,
            })
            .collect();
        let (usable, level_locked, _) = filter_bag_seeds(&mapped);
        let level_locked_ids: std::collections::HashSet<i64> =
            level_locked.iter().map(|s| s.seed_id).collect();
        let priority = crate::models::store::account_config::get_bag_seed_priority(Some(account_id));
        let ordered = plan_bag_planting_order(&usable, &priority);
        if ordered.is_empty() {
            return Ok(BagPlantResult {
                remaining_land_ids: target,
                fallback_allowed: true,
                ..Default::default()
            });
        }

        let mut fallback_allowed = true;
        let mut planted_land_ids = Vec::new();
        let mut occupied = std::collections::HashSet::new();
        let mut total_planted = 0usize;

        for seed in ordered {
            if target.is_empty() || !fallback_allowed {
                break;
            }
            let plant_size = seed.plant_size.max(1);
            let all_layouts = build_planting_layouts(&target, plant_size);
            let layouts =
                select_non_overlapping_layouts(&all_layouts, seed.count.max(0) as usize);
            if layouts.is_empty() {
                continue;
            }
            let result = self
                .plant_seeds_with_layouts(seed.seed_id, layouts, usize::MAX, host_gid)
                .await?;
            for id in result
                .reserved_land_ids
                .iter()
                .chain(result.occupied_land_ids.iter())
            {
                if *id > 0 {
                    occupied.insert(*id);
                }
            }
            target.retain(|id| !occupied.contains(id));
            if result.planted > 0 {
                total_planted += result.planted;
                planted_land_ids.extend(result.planted_land_ids);
            }
            if result.uncertain && !level_locked_ids.contains(&seed.seed_id) {
                fallback_allowed = false;
            }
        }
        planted_land_ids.sort_unstable();
        planted_land_ids.dedup();
        Ok(BagPlantResult {
            remaining_land_ids: target,
            fallback_allowed,
            planted_land_ids,
            total_planted,
            occupied_count: occupied.len(),
        })
    }

    /// 商店选种并种植
    pub async fn plant_from_shop(
        &self,
        lands_to_plant: &[i64],
        host_gid: i64,
        account_id: &str,
        override_strategy: Option<crate::models::types::PlantingStrategy>,
    ) -> Result<ShopPlantResult> {
        let candidates = self.find_best_seed(account_id, override_strategy).await?;
        let mut remaining: Vec<i64> = {
            let mut seen = std::collections::HashSet::new();
            lands_to_plant
                .iter()
                .copied()
                .filter(|id| *id > 0 && seen.insert(*id))
                .collect()
        };
        if candidates.is_empty() {
            return Ok(ShopPlantResult {
                remaining_land_ids: remaining,
                ..Default::default()
            });
        }
        let mut planted_lands = Vec::new();
        let mut uncertain = false;
        let mut gold = crate::services::status::status_data_for(account_id).gold;

        for candidate in candidates {
            if remaining.is_empty() || uncertain {
                break;
            }
            let plant_size = get_plant_size_by_seed_id(candidate.seed_id);
            let all_layouts = build_planting_layouts(&remaining, plant_size);
            let mut layouts = select_non_overlapping_layouts(&all_layouts, all_layouts.len());
            if layouts.is_empty() {
                continue;
            }
            let unit = candidate.unit_item_count.max(1);
            let required_seed_count = layouts.len() as i64;
            let required_purchase = (required_seed_count + unit - 1) / unit;
            let max_purchase = if candidate.max_purchase_count < 0 {
                required_purchase
            } else {
                candidate.max_purchase_count
            };
            let affordable = if candidate.price > 0 {
                gold / candidate.price
            } else {
                0
            };
            let purchase_units = required_purchase.min(max_purchase).min(affordable);
            if purchase_units <= 0 {
                continue;
            }
            let mut need_count = required_seed_count.min(purchase_units * unit);
            layouts.truncate(need_count as usize);

            let buy = match self
                .api
                .buy_goods(candidate.goods_id, purchase_units, candidate.price)
                .await
            {
                Ok(r) => r,
                Err(_) => {
                    uncertain = true;
                    break;
                }
            };
            let mut actual_seed_id = candidate.seed_id;
            if let Some(item) = buy.get_items.first() {
                if item.id > 0 {
                    actual_seed_id = item.id;
                }
                if item.count > 0 && item.count < need_count {
                    need_count = item.count;
                    layouts.truncate(need_count as usize);
                }
            }
            for item in &buy.cost_items {
                gold -= item.count;
            }

            let result = self
                .plant_seeds_with_layouts(
                    actual_seed_id,
                    layouts,
                    need_count as usize,
                    host_gid,
                )
                .await?;
            planted_lands.extend(result.planted_land_ids);
            let consumed: std::collections::HashSet<i64> = result
                .reserved_land_ids
                .iter()
                .chain(result.occupied_land_ids.iter())
                .copied()
                .filter(|id| *id > 0)
                .collect();
            remaining.retain(|id| !consumed.contains(id));
            if result.uncertain {
                uncertain = true;
            }
        }
        planted_lands.sort_unstable();
        planted_lands.dedup();
        Ok(ShopPlantResult {
            planted_lands,
            remaining_land_ids: remaining,
            uncertain,
        })
    }

    /// 商店候选种子（analytics 排序 + 金币/限购）
    pub async fn find_best_seed(
        &self,
        account_id: &str,
        override_strategy: Option<crate::models::types::PlantingStrategy>,
    ) -> Result<Vec<ShopSeedCandidate>> {
        const SEED_SHOP_ID: i64 = 2;
        let shop = self.api.get_shop_info(SEED_SHOP_ID).await?;
        if shop.goods_list.is_empty() {
            return Ok(vec![]);
        }
        let state_level = crate::services::status::status_data_for(account_id).level;
        let gc = crate::config::game_config::global();
        let mut inputs = Vec::new();
        for goods in &shop.goods_list {
            let mut required_level = 0i64;
            let mut cond_type = 0i64;
            let mut cond_param = 0i64;
            for cond in &goods.conds {
                if cond.r#type == 1 {
                    required_level = cond.param;
                    cond_type = 1;
                    cond_param = cond.param;
                }
            }
            inputs.push(ShopSeedInput {
                goods_id: goods.id,
                seed_id: goods.item_id,
                name: gc.get_plant_name_by_seed_id(goods.item_id),
                price: goods.price,
                required_level,
                limit_count: goods.limit_count,
                bought_num: goods.bought_num,
                item_count: goods.item_count,
                unlocked: goods.unlocked,
                cond_type,
                cond_param,
            });
        }
        let available = filter_shop_seeds(&inputs, state_level);
        let strategy =
            override_strategy.unwrap_or_else(|| {
                crate::models::store::account_config::get_planting_strategy(Some(account_id))
            });
        let strategy_key = planting_strategy_key(strategy);
        let rankings = crate::services::analytics::get_plant_rankings(
            crate::services::analytics::SortBy::from_str_opt(match strategy_key {
                "max_exp" => "exp",
                "max_fert_exp" => "fert",
                "max_profit" => "profit",
                "max_fert_profit" => "fert_profit",
                _ => "level",
            }),
        );
        let ranking_ids: Vec<i64> = rankings
            .into_iter()
            .filter(|r| {
                r.seed_id > 0
                    && r.level
                        .map(|l| l <= 0 || l <= state_level)
                        .unwrap_or(true)
            })
            .map(|r| r.seed_id)
            .collect();
        let preferred =
            crate::models::store::account_config::get_preferred_seed(Some(account_id));
        Ok(sort_shop_seeds_by_strategy(
            available,
            strategy_key,
            &ranking_ids,
            preferred,
        ))
    }
}

fn planting_strategy_key(s: crate::models::types::PlantingStrategy) -> &'static str {
    match s {
        crate::models::types::PlantingStrategy::Preferred => "preferred",
        crate::models::types::PlantingStrategy::Level => "level",
        crate::models::types::PlantingStrategy::MaxExp => "max_exp",
        crate::models::types::PlantingStrategy::MaxFertExp => "max_fert_exp",
        crate::models::types::PlantingStrategy::MaxProfit => "max_profit",
        crate::models::types::PlantingStrategy::MaxFertProfit => "max_fert_profit",
        crate::models::types::PlantingStrategy::BagPriority => "bag_priority",
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FertilizeResult {
    pub normal: usize,
    pub organic: usize,
}

#[derive(Default)]
struct FertililzeResultBuilder {
    normal: usize,
    organic: usize,
}

impl FertililzeResultBuilder {
    fn build(self) -> FertilizeResult {
        FertilizeResult {
            normal: self.normal,
            organic: self.organic,
        }
    }
}

/// 选择应种植的种子 ID
#[must_use]
pub fn select_seed(config: &PlantingConfig) -> i64 {
    config.preferred_seed_id
}

// =====================================================================
// 阶段 2E：背包种植 / 商店选种 / 自动种空地 核心算法 1:1 翻译
// （完整执行版需 run_one_cycle 上下文，以下是 1:1 算法骨架）
// =====================================================================

/// 背包种子（含玩家等级适配）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BagSeedWithLevel {
    pub seed_id: i64,
    pub name: String,
    pub count: i64,
    pub required_level: i64,
    pub plant_size: usize,
    pub state_level: i64,
}

/// 过滤背包种子：剔除 count=0 / plant_size<1，记录等级锁定
///
/// 对应原 planting.ts `plantFromBagSeeds` 内的过滤逻辑
#[must_use]
pub fn filter_bag_seeds(
    seeds: &[BagSeedWithLevel],
) -> (Vec<BagSeedWithLevel>, Vec<BagSeedWithLevel>, Vec<(i64, String, &'static str)>) {
    let mut skipped: Vec<(i64, String, &'static str)> = Vec::new();
    let mut level_locked: Vec<BagSeedWithLevel> = Vec::new();
    let usable: Vec<BagSeedWithLevel> = seeds
        .iter()
        .filter(|s| {
            if s.count <= 0 {
                skipped.push((s.seed_id, s.name.clone(), "count_zero"));
                return false;
            }
            if s.plant_size < 1 {
                skipped.push((s.seed_id, s.name.clone(), "invalid_size"));
                return false;
            }
            if s.required_level > s.state_level {
                level_locked.push((*s).clone());
            }
            true
        })
        .cloned()
        .collect();
    (usable, level_locked, skipped)
}

/// 把 `BagSeedWithLevel` 转 `BagSeedLite`（用于 sort_bag_seeds_for_planting）
fn to_bag_seed_lite(seeds: &[BagSeedWithLevel]) -> Vec<BagSeedLite> {
    seeds
        .iter()
        .map(|s| BagSeedLite {
            seed_id: s.seed_id,
            required_level: s.required_level,
            count: s.count,
        })
        .collect()
}

/// 排序后的背包种植顺序（含 priority + level desc + id 排序）
///
/// 对应原 planting.ts `plantFromBagSeeds` 主循环前的排序
#[must_use]
pub fn plan_bag_planting_order(
    seeds: &[BagSeedWithLevel],
    priority: &[i64],
) -> Vec<BagSeedWithLevel> {
    let lites = to_bag_seed_lite(seeds);
    let sorted = sort_bag_seeds_for_planting(&lites, priority);
    // 把 BagSeedLite 顺序映回 BagSeedWithLevel
    let mut out = Vec::with_capacity(sorted.len());
    for lite in sorted {
        if let Some(orig) = seeds.iter().find(|s| s.seed_id == lite.seed_id) {
            out.push(orig.clone());
        }
    }
    out
}

/// 商店可买种子（含等级 / 限购 / 售价过滤）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShopSeedCandidate {
    pub goods_id: i64,
    pub seed_id: i64,
    pub name: String,
    pub price: i64,
    pub required_level: i64,
    pub unit_item_count: i64,
    pub max_purchase_count: i64, // -1 表示无限
    pub bought_num: i64,
    pub limit_count: i64,
}

/// 商店种子（输入：来自 api.get_shop_info 的原始商品）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShopSeedInput {
    pub goods_id: i64,
    pub seed_id: i64,
    pub name: String,
    pub price: i64,
    pub required_level: i64,
    pub limit_count: i64,
    pub bought_num: i64,
    pub item_count: i64,
    pub unlocked: bool,
    pub cond_type: i64,    // 1=等级条件
    pub cond_param: i64,   // cond_type=1 时是等级阈值
}

/// 过滤商店种子
///
/// 对应原 planting.ts `findBestSeed` 的过滤部分
#[must_use]
pub fn filter_shop_seeds(
    inputs: &[ShopSeedInput],
    state_level: i64,
) -> Vec<ShopSeedCandidate> {
    let mut out = Vec::new();
    for g in inputs {
        if !g.unlocked {
            continue;
        }
        if g.cond_type == 1 && state_level < g.cond_param {
            continue;
        }
        let required_level = if g.cond_type == 1 {
            g.cond_param
        } else {
            g.required_level
        };
        if g.limit_count > 0 && g.bought_num >= g.limit_count {
            continue;
        }
        if g.price <= 0 {
            continue;
        }
        out.push(ShopSeedCandidate {
            goods_id: g.goods_id,
            seed_id: g.seed_id,
            name: g.name.clone(),
            price: g.price,
            required_level,
            unit_item_count: std::cmp::max(1, g.item_count),
            max_purchase_count: if g.limit_count > 0 {
                std::cmp::max(0, g.limit_count - g.bought_num)
            } else {
                -1
            },
            bought_num: g.bought_num,
            limit_count: g.limit_count,
        });
    }
    out
}

/// 按策略排序商店种子
///
/// - `level` / 默认：required_level desc + seed_id asc
/// - `preferred`：preferred 提到最前
/// - `max_exp` / `max_profit` / `max_fert_exp` / `max_fert_profit`：用 ranking 注入顺序
#[must_use]
pub fn sort_shop_seeds_by_strategy(
    candidates: Vec<ShopSeedCandidate>,
    strategy: &str,
    ranking_seed_ids: &[i64],
    preferred_seed_id: i64,
) -> Vec<ShopSeedCandidate> {
    let mut c = candidates;
    if matches!(strategy, "max_exp" | "max_profit" | "max_fert_exp" | "max_fert_profit") {
        let ranking: std::collections::HashMap<i64, usize> = ranking_seed_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();
        c.sort_by(|a, b| {
            let ra = ranking.get(&a.seed_id).copied().unwrap_or(usize::MAX);
            let rb = ranking.get(&b.seed_id).copied().unwrap_or(usize::MAX);
            ra.cmp(&rb).then_with(|| {
                b.required_level
                    .cmp(&a.required_level)
                    .then(a.seed_id.cmp(&b.seed_id))
            })
        });
    } else {
        // 默认 level desc
        c.sort_by(|a, b| {
            b.required_level
                .cmp(&a.required_level)
                .then(a.seed_id.cmp(&b.seed_id))
        });
    }
    if strategy == "preferred" && preferred_seed_id > 0 {
        if let Some(idx) = c.iter().position(|s| s.seed_id == preferred_seed_id) {
            let preferred = c.remove(idx);
            c.insert(0, preferred);
        }
    }
    c
}

/// 自动种植空地的执行计划
///
/// - `dead_land_ids`：已收获 / 死亡土地（不需要种）
/// - `empty_land_ids`：空地（需要种）
/// - `bag_usable`：背包可用种子
/// - `shop_candidates`：商店候选（背包用完后回退）
///
/// 返回按顺序执行的计划
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlantingPlan {
    /// 背包种子阶段：每个种子用多少个 anchor
    pub bag_phase: Vec<BagPhaseEntry>,
    /// 商店阶段
    pub shop_phase: Vec<ShopPhaseEntry>,
    /// 最终剩余未种的土地 id
    pub remaining_land_ids: Vec<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BagPhaseEntry {
    pub seed_id: i64,
    pub name: String,
    pub plant_size: usize,
    pub anchor_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShopPhaseEntry {
    pub goods_id: i64,
    pub seed_id: i64,
    pub name: String,
    pub price: i64,
    pub max_purchase_count: i64,
    pub anchor_count: usize,
}

/// 用背包种子生成种植计划
///
/// 简化版：每种背包种子按 count 个 anchor 计划（不展开 layout 选位）
#[must_use]
pub fn plan_auto_plant_with_bag(
    empty_land_ids: &[i64],
    bag_usable: &[BagSeedWithLevel],
    priority: &[i64],
) -> PlantingPlan {
    let mut remaining = empty_land_ids.to_vec();
    let mut plan = PlantingPlan::default();
    let ordered = plan_bag_planting_order(bag_usable, priority);
    for seed in ordered {
        if remaining.is_empty() {
            break;
        }
        let anchor_count = std::cmp::min(seed.count as usize, remaining.len());
        if anchor_count == 0 {
            continue;
        }
        plan.bag_phase.push(BagPhaseEntry {
            seed_id: seed.seed_id,
            name: seed.name.clone(),
            plant_size: seed.plant_size,
            anchor_count,
        });
        // 消耗前 anchor_count 个 land id
        remaining = remaining.split_off(anchor_count);
    }
    plan.remaining_land_ids = remaining;
    plan
}

/// 用商店种子生成补种计划
#[must_use]
pub fn plan_shop_planting(
    remaining_land_ids: &[i64],
    shop_candidates: &[ShopSeedCandidate],
    strategy: &str,
    ranking_seed_ids: &[i64],
    preferred_seed_id: i64,
) -> PlantingPlan {
    let sorted = sort_shop_seeds_by_strategy(
        shop_candidates.to_vec(),
        strategy,
        ranking_seed_ids,
        preferred_seed_id,
    );
    let mut remaining = remaining_land_ids.to_vec();
    let mut plan = PlantingPlan::default();
    for c in &sorted {
        if remaining.is_empty() {
            break;
        }
        let max = if c.max_purchase_count < 0 {
            remaining.len() as i64
        } else {
            c.max_purchase_count
        };
        let anchor_count = std::cmp::min(max as usize, remaining.len());
        if anchor_count == 0 {
            continue;
        }
        plan.shop_phase.push(ShopPhaseEntry {
            goods_id: c.goods_id,
            seed_id: c.seed_id,
            name: c.name.clone(),
            price: c.price,
            max_purchase_count: max,
            anchor_count,
        });
        remaining = remaining.split_off(anchor_count);
    }
    plan.remaining_land_ids = remaining;
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let c = PlantingConfig::default();
        assert!(c.enabled);
        assert_eq!(c.strategy, PlantingStrategy::MaxExp);
        assert_eq!(c.fertilize_mode, FertilizeMode::Smart);
    }

    #[test]
    fn select_seed_returns_preferred() {
        let c = PlantingConfig { preferred_seed_id: 42, ..Default::default() };
        assert_eq!(select_seed(&c), 42);
    }

    #[test]
    fn strategy_serde_roundtrip() {
        let s = serde_json::to_string(&PlantingStrategy::MaxProfit).unwrap();
        assert_eq!(s, "\"MaxProfit\"");
    }

    #[test]
    fn fertilize_mode_default_smart() {
        let m = FertilizeMode::default();
        assert_eq!(m, FertilizeMode::Smart);
    }

    #[test]
    fn get_plant_size_default_one() {
        assert_eq!(get_plant_size_by_seed_id(100), 1);
        assert_eq!(get_plant_size_by_seed_id(0), 1);
    }

    // ===== 阶段 2E 补全测试 =====

    #[test]
    fn planting_strategy_label_basic() {
        assert_eq!(get_planting_strategy_label("preferred"), "优先种植种子");
        assert_eq!(get_planting_strategy_label("max_exp"), "最大经验/时");
        assert_eq!(get_planting_strategy_label("max_profit"), "最大净利润/时");
        // 未知策略 → 原样返回
        assert_eq!(get_planting_strategy_label("unknown"), "unknown");
    }

    #[test]
    fn sort_bag_seeds_priority() {
        let seeds = vec![
            BagSeedLite { seed_id: 100, required_level: 1, count: 5 },
            BagSeedLite { seed_id: 200, required_level: 5, count: 3 },
            BagSeedLite { seed_id: 300, required_level: 10, count: 1 },
        ];
        // priority: 300 在前
        let sorted = sort_bag_seeds_for_planting(&seeds, &[300, 100]);
        // 300 (priority 0), 100 (priority 1), 200 (no priority, 按 required_level desc)
        assert_eq!(sorted[0].seed_id, 300);
        assert_eq!(sorted[1].seed_id, 100);
        assert_eq!(sorted[2].seed_id, 200);
    }

    #[test]
    fn sort_bag_seeds_no_priority() {
        let seeds = vec![
            BagSeedLite { seed_id: 100, required_level: 1, count: 5 },
            BagSeedLite { seed_id: 200, required_level: 5, count: 3 },
        ];
        let sorted = sort_bag_seeds_for_planting(&seeds, &[]);
        // 无 priority → 按 required_level desc
        assert_eq!(sorted[0].seed_id, 200);
        assert_eq!(sorted[1].seed_id, 100);
    }

    #[test]
    fn confirms_planted_footprint_basic() {
        use crate::proto::generated::gamepb::plantpb::{LandInfo, PlantInfo};
        let mut expected = std::collections::HashSet::new();
        expected.insert(1);
        expected.insert(2);
        let occupied = vec![1, 2, 3];
        let lands = vec![LandInfo {
            id: 1,
            plant: Some(PlantInfo {
                id: 100,
                ..Default::default()
            }),
            ..Default::default()
        }];
        // master=1 有 plant → true
        assert!(confirms_planted_footprint(&expected, 1, &occupied, &lands));
    }

    #[test]
    fn confirms_planted_footprint_missing_land() {
        use crate::proto::generated::gamepb::plantpb::LandInfo;
        let mut expected = std::collections::HashSet::new();
        expected.insert(1);
        expected.insert(2);
        let occupied = vec![1]; // 缺 2
        let lands = vec![LandInfo {
            id: 1,
            ..Default::default()
        }];
        assert!(!confirms_planted_footprint(&expected, 1, &occupied, &lands));
    }

    #[test]
    fn confirms_planted_footprint_no_plant() {
        use crate::proto::generated::gamepb::plantpb::LandInfo;
        let mut expected = std::collections::HashSet::new();
        expected.insert(1);
        let occupied = vec![1];
        let lands = vec![LandInfo {
            id: 1,
            plant: None,
            ..Default::default()
        }];
        // master=1 没 plant → false
        assert!(!confirms_planted_footprint(&expected, 1, &occupied, &lands));
    }

    #[test]
    fn encode_plant_request_matches_prost() {
        use prost::Message as _;
        // 手写编码应与 prost 生成的 PlantRequest（单个 PlantItem）字节一致
        let body = encode_plant_request(100, &[1, 2, 3]);
        let expected = crate::proto::generated::gamepb::plantpb::PlantRequest {
            land_and_seed: Default::default(),
            items: vec![PlantItem {
                seed_id: 100,
                land_ids: vec![1, 2, 3],
                auto_slave: false,
            }],
        }
        .encode_to_vec();
        assert_eq!(body, expected);
        // round-trip 解码校验
        let decoded = crate::proto::generated::gamepb::plantpb::PlantRequest::decode(&*body)
            .expect("应能解码为 PlantRequest");
        assert_eq!(decoded.items.len(), 1);
        assert_eq!(decoded.items[0].seed_id, 100);
        assert_eq!(decoded.items[0].land_ids, vec![1, 2, 3]);
    }

    // ===== 阶段 2E：背包/商店种植算法 =====

    fn bag_seed(seed_id: i64, count: i64, required_level: i64, state_level: i64) -> BagSeedWithLevel {
        BagSeedWithLevel {
            seed_id,
            name: format!("seed-{seed_id}"),
            count,
            required_level,
            plant_size: 1,
            state_level,
        }
    }

    #[test]
    fn filter_bag_seeds_skips_zero_count() {
        let seeds = vec![bag_seed(1, 0, 1, 5), bag_seed(2, 3, 1, 5)];
        let (usable, _level, skipped) = filter_bag_seeds(&seeds);
        assert_eq!(usable.len(), 1);
        assert_eq!(usable[0].seed_id, 2);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].2, "count_zero");
    }

    #[test]
    fn filter_bag_seeds_records_level_locked() {
        let seeds = vec![bag_seed(1, 3, 10, 5)];
        let (usable, level, _skipped) = filter_bag_seeds(&seeds);
        assert_eq!(usable.len(), 1);
        assert_eq!(level.len(), 1);
        assert_eq!(level[0].seed_id, 1);
    }

    #[test]
    fn filter_bag_seeds_skips_invalid_size() {
        let mut s = bag_seed(1, 3, 1, 5);
        s.plant_size = 0;
        let (usable, _, skipped) = filter_bag_seeds(&[s]);
        assert_eq!(usable.len(), 0);
        assert_eq!(skipped[0].2, "invalid_size");
    }

    #[test]
    fn plan_bag_planting_order_priority() {
        let seeds = vec![
            bag_seed(100, 5, 1, 10),
            bag_seed(200, 5, 1, 10),
            bag_seed(300, 5, 1, 10),
        ];
        // priority: 300 在前
        let plan = plan_bag_planting_order(&seeds, &[300, 100]);
        assert_eq!(plan[0].seed_id, 300);
        assert_eq!(plan[1].seed_id, 100);
    }

    #[test]
    fn plan_auto_plant_with_bag_uses_count() {
        let empty = vec![1, 2, 3, 4, 5];
        let bag = vec![bag_seed(100, 3, 1, 10)];
        let plan = plan_auto_plant_with_bag(&empty, &bag, &[]);
        assert_eq!(plan.bag_phase.len(), 1);
        assert_eq!(plan.bag_phase[0].seed_id, 100);
        assert_eq!(plan.bag_phase[0].anchor_count, 3);
        assert_eq!(plan.remaining_land_ids, vec![4, 5]);
    }

    #[test]
    fn plan_auto_plant_with_bag_stops_when_empty() {
        let empty = vec![1, 2];
        let bag = vec![bag_seed(100, 5, 1, 10)];
        let plan = plan_auto_plant_with_bag(&empty, &bag, &[]);
        assert_eq!(plan.bag_phase[0].anchor_count, 2);
        assert!(plan.remaining_land_ids.is_empty());
    }

    #[test]
    fn filter_shop_seeds_basic() {
        let inputs = vec![
            ShopSeedInput {
                goods_id: 1, seed_id: 100, name: "萝卜".into(), price: 100,
                required_level: 0, limit_count: 0, bought_num: 0, item_count: 1,
                unlocked: true, cond_type: 1, cond_param: 1,
            },
            ShopSeedInput {
                goods_id: 2, seed_id: 200, name: "白菜".into(), price: 0,  // price=0 应过滤
                required_level: 0, limit_count: 0, bought_num: 0, item_count: 1,
                unlocked: true, cond_type: 0, cond_param: 0,
            },
            ShopSeedInput {
                goods_id: 3, seed_id: 300, name: "玉米".into(), price: 200,
                required_level: 0, limit_count: 5, bought_num: 5, item_count: 1, // 已限购
                unlocked: true, cond_type: 0, cond_param: 0,
            },
            ShopSeedInput {
                goods_id: 4, seed_id: 400, name: "南瓜".into(), price: 500,
                required_level: 0, limit_count: 0, bought_num: 0, item_count: 1,
                unlocked: false, cond_type: 0, cond_param: 0, // 未解锁
            },
            ShopSeedInput {
                goods_id: 5, seed_id: 500, name: "高等级".into(), price: 1000,
                required_level: 0, limit_count: 0, bought_num: 0, item_count: 1,
                unlocked: true, cond_type: 1, cond_param: 10, // 等级锁
            },
        ];
        let cands = filter_shop_seeds(&inputs, 5); // state_level=5
        // 应该只有 goods_id=1（萝卜）
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].seed_id, 100);
        assert_eq!(cands[0].required_level, 1);
        assert_eq!(cands[0].max_purchase_count, -1);
    }

    #[test]
    fn filter_shop_seeds_limit_count() {
        let inputs = vec![ShopSeedInput {
            goods_id: 1, seed_id: 100, name: "x".into(), price: 100,
            required_level: 0, limit_count: 10, bought_num: 3, item_count: 1,
            unlocked: true, cond_type: 0, cond_param: 0,
        }];
        let cands = filter_shop_seeds(&inputs, 1);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].max_purchase_count, 7);
    }

    #[test]
    fn sort_shop_seeds_by_strategy_default_level() {
        let cands = vec![
            ShopSeedCandidate { goods_id: 1, seed_id: 100, name: "A".into(), price: 100, required_level: 1, unit_item_count: 1, max_purchase_count: -1, bought_num: 0, limit_count: 0 },
            ShopSeedCandidate { goods_id: 2, seed_id: 200, name: "B".into(), price: 200, required_level: 5, unit_item_count: 1, max_purchase_count: -1, bought_num: 0, limit_count: 0 },
            ShopSeedCandidate { goods_id: 3, seed_id: 150, name: "C".into(), price: 150, required_level: 3, unit_item_count: 1, max_purchase_count: -1, bought_num: 0, limit_count: 0 },
        ];
        let sorted = sort_shop_seeds_by_strategy(cands, "level", &[], 0);
        // 按 required_level desc: 5, 3, 1
        assert_eq!(sorted[0].seed_id, 200);
        assert_eq!(sorted[1].seed_id, 150);
        assert_eq!(sorted[2].seed_id, 100);
    }

    #[test]
    fn sort_shop_seeds_by_strategy_preferred() {
        let cands = vec![
            ShopSeedCandidate { goods_id: 1, seed_id: 100, name: "A".into(), price: 100, required_level: 1, unit_item_count: 1, max_purchase_count: -1, bought_num: 0, limit_count: 0 },
            ShopSeedCandidate { goods_id: 2, seed_id: 200, name: "B".into(), price: 200, required_level: 5, unit_item_count: 1, max_purchase_count: -1, bought_num: 0, limit_count: 0 },
        ];
        let sorted = sort_shop_seeds_by_strategy(cands, "preferred", &[], 200);
        assert_eq!(sorted[0].seed_id, 200);
    }

    #[test]
    fn sort_shop_seeds_by_strategy_ranking() {
        let cands = vec![
            ShopSeedCandidate { goods_id: 1, seed_id: 100, name: "A".into(), price: 100, required_level: 5, unit_item_count: 1, max_purchase_count: -1, bought_num: 0, limit_count: 0 },
            ShopSeedCandidate { goods_id: 2, seed_id: 200, name: "B".into(), price: 200, required_level: 3, unit_item_count: 1, max_purchase_count: -1, bought_num: 0, limit_count: 0 },
        ];
        // ranking: 200 在前
        let sorted = sort_shop_seeds_by_strategy(cands, "max_exp", &[200, 100], 0);
        assert_eq!(sorted[0].seed_id, 200);
    }

    #[test]
    fn plan_shop_planting_uses_max_purchase() {
        let remaining = vec![1, 2, 3, 4, 5];
        let cands = vec![
            ShopSeedCandidate { goods_id: 1, seed_id: 100, name: "A".into(), price: 100, required_level: 1, unit_item_count: 1, max_purchase_count: 2, bought_num: 0, limit_count: 5 },
        ];
        let plan = plan_shop_planting(&remaining, &cands, "level", &[], 0);
        assert_eq!(plan.shop_phase.len(), 1);
        assert_eq!(plan.shop_phase[0].anchor_count, 2);
        assert_eq!(plan.remaining_land_ids, vec![3, 4, 5]);
    }

    #[test]
    fn plan_shop_planting_unlimited() {
        let remaining = vec![1, 2, 3];
        let cands = vec![
            ShopSeedCandidate { goods_id: 1, seed_id: 100, name: "A".into(), price: 100, required_level: 1, unit_item_count: 1, max_purchase_count: -1, bought_num: 0, limit_count: 0 },
        ];
        let plan = plan_shop_planting(&remaining, &cands, "level", &[], 0);
        assert_eq!(plan.shop_phase[0].anchor_count, 3);
        assert!(plan.remaining_land_ids.is_empty());
    }
}
