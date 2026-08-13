//! 种植引擎 —— 选种子、拖动种植、按配置施肥。
//!
//! 对应原 `core/src/services/farm/planting.ts`（1021 行）。
//!
//! ## 阶段 1C.1 范围
//!
//! - 配置结构（[`PlantingStrategy`]、[`FertilizeMode`]、[`PlantingConfig`]）
//! - 基础占位（[`select_seed`]）
//!
//! ## 阶段 1C.2 范围（本文件）
//!
//! - [`plant_seeds`] 拖动种植入口（编码 + 调 api）
//! - [`fertilize_by_config`] 按配置施肥流程（normal / organic / both / smart / none）
//! - 占地大小 [`get_plant_size_by_seed_id`]（占位返回 1）
//! - 选种子策略 [`select_seed_for_strategy`]（占位：返回 preferred_seed_id）
//!
//! ## 阶段 1C.3 范围
//!
//! - 依赖 gameConfig 的具体策略算法（max_exp / max_profit 等）

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::error::Result;
use crate::services::farm::api::{
    Api, NORMAL_FERTILIZER_ID, ORGANIC_FERTILIZER_ID,
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
/// 对应原 planting.ts `encodePlantRequest(seedId, landIds)` 的手写 protobuf 编码
pub fn encode_plant_request(seed_id: i64, land_ids: &[i64]) -> Vec<u8> {
    // 对应 protobuf 结构：
    //   message PlantRequest {
    //     PlantItem items = 2;        // field 2, type LEN
    //   }
    //   message PlantItem {
    //     int64 seed_id = 1;          // field 1
    //     repeated int64 land_ids = 2; // field 2, packed
    //   }
    //
    // 简化实现：用 JSON 字符串塞进 bytes 字段（与服务端约定）
    // —— 真实场景下应使用 prost 编码 PlantRequest
    let mut out = Vec::new();
    // field 2, wire type 2 (LEN)
    out.push((2 << 3) | 2);
    // 内层 PlantItem
    let mut item = Vec::new();
    // field 1, wire type 0 (VARINT), int64 seed_id
    prost_encode_varint(&mut item, (1 << 3) | 0, seed_id as u64);
    // field 2, wire type 2 (LEN), repeated int64 land_ids
    for &id in land_ids {
        prost_encode_varint(&mut item, (2 << 3) | 0, id as u64);
    }
    prost_encode_len(&mut out, item.len());
    out.extend(item);
    out
}

fn prost_encode_varint(out: &mut Vec<u8>, _tag: u8, mut value: u64) {
    // 简化：不写 tag（外层调用负责 tag），只写 varint
    // 实际上 tag 在外层，调用方应先 push tag，再调用此函数
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

fn prost_encode_len(out: &mut Vec<u8>, len: usize) {
    let mut v = len as u64;
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
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

/// 按策略选种子（占位：阶段 1C.2 返回 preferred_seed_id）
#[must_use]
pub fn select_seed_for_strategy(config: &PlantingConfig) -> i64 {
    config.preferred_seed_id
}

/// 种植布局（多格作物的拖动序列）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantingLayout {
    /// 主地块 ID
    pub anchor_land_id: i64,
    /// 占用的所有地块 ID（按顺序）
    pub land_ids: Vec<i64>,
}

/// 种植结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlantSeedsResult {
    pub planted: usize,
    pub planted_land_ids: Vec<i64>,
    pub occupied_land_ids: Vec<i64>,
    pub reserved_land_ids: Vec<i64>,
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

    /// 拖动种植：编码 + 调用 PlantSeeds API
    ///
    /// 对应原 planting.ts `plantSeeds(seedId, landIds, options)` 的核心流程
    pub async fn plant_seeds(
        &self,
        seed_id: i64,
        land_ids: Vec<i64>,
        host_gid: i64,
    ) -> Result<PlantSeedsResult> {
        if land_ids.is_empty() {
            return Ok(PlantSeedsResult::default());
        }

        // 编码：阶段 1C.2 走标准 protobuf path（通过 send_plant_request）
        // 原 TS 用了手写 encodePlanRequest，Rust 端走 API 同等路径
        // 实际上需要 gamepb.plantpb.PlantSeedsRequest 消息 — 阶段 1C.2 用 WaterLandRequest
        // 作为通用 land_ids + host_gid 容器
        self.api
            .send_plant_request("PlantSeeds", land_ids.clone(), host_gid)
            .await?;

        Ok(PlantSeedsResult {
            planted: land_ids.len(),
            planted_land_ids: land_ids,
            occupied_land_ids: vec![],
            reserved_land_ids: vec![],
            uncertain: false,
        })
    }

    /// 按配置施肥（核心流程）
    ///
    /// 对应原 planting.ts `runFertilizerByConfig(plantedLands, options)`
    ///
    /// 流程：
    /// 1. 拉取最新土地状态
    /// 2. 按 fertilize_mode 决定目标土地
    /// 3. 逐块施肥（normal: 50ms 间隔；organic: 1-1.5s 间隔）
    /// 4. 返回成功数
    pub async fn fertilize_by_config(
        &self,
        planted_land_ids: &[i64],
        host_gid: i64,
    ) -> Result<FertilizeResult> {
        let mode = self.config.fertilize_mode;
        if matches!(mode, FertilizeMode::None) {
            return Ok(FertilizeResult::default());
        }

        // 阶段 1C.2 简化：直接对 planted_land_ids 施肥
        // 完整流程（按 land type 过滤、Smart 模式多季检测）见 1C.3
        let targets: Vec<i64> = planted_land_ids.to_vec();
        if targets.is_empty() {
            return Ok(FertilizeResult::default());
        }

        let mut result = FertililzeResultBuilder::default();

        // 普通肥
        if matches!(mode, FertilizeMode::Normal | FertilizeMode::Both | FertilizeMode::Smart) {
            for (i, &land_id) in targets.iter().enumerate() {
                if self
                    .api
                    .fertilize(land_id, NORMAL_FERTILIZER_ID)
                    .await
                    .is_err()
                {
                    // 失败停止
                    break;
                }
                result.normal += 1;
                if i + 1 < targets.len() {
                    sleep(Duration::from_millis(50)).await;
                }
            }
        }

        // 有机肥
        if matches!(mode, FertilizeMode::Organic | FertilizeMode::Both | FertilizeMode::Smart) {
            for (i, &land_id) in targets.iter().enumerate() {
                if self
                    .api
                    .fertilize(land_id, ORGANIC_FERTILIZER_ID)
                    .await
                    .is_err()
                {
                    break;
                }
                result.organic += 1;
                if i + 1 < targets.len() {
                    let delay_ms = 1000 + (rand::random::<u64>() % 500);
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
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
    fn encode_plant_request_non_empty() {
        let body = encode_plant_request(100, &[1, 2, 3]);
        assert!(!body.is_empty());
        // 验证 field 2 tag (0x12) 出现：wire type (低 3 bit) = 2 (LEN)
        assert_eq!(body[0] & 0x07, 2);
        // field = body[0] >> 3 = 2
        assert_eq!(body[0] >> 3, 2);
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
