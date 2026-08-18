//! 数据分析模块 — 作物效率分析。
//!
//! 1:1 翻译原 `core/src/services/analytics.ts`（150 行）。
//!
//! 提供 `get_plant_rankings`：按 exp / fert / gold / profit / fert_profit / level
//! 排序的植物排行（每时经验 / 每时金币 / 净收益 / 施肥后净收益等）。

use serde::{Deserialize, Serialize};

use crate::config::game_config::global as global_game_config;

/// 解析 grow_phases 字符串求和（如 "种子:30;发芽:30;成熟:0;" → 60）
#[must_use]
pub fn parse_grow_time(grow_phases: &str) -> i64 {
    if grow_phases.is_empty() {
        return 0;
    }
    let mut total: i64 = 0;
    for part in grow_phases.split(';') {
        if part.is_empty() {
            continue;
        }
        if let Some((_, secs_str)) = part.rsplit_once(':') {
            if let Ok(secs) = secs_str.parse::<i64>() {
                total += secs;
            }
        }
    }
    total
}

/// 解析第一阶段秒数（普通化肥减少的时长）
#[must_use]
pub fn parse_normal_fertilizer_reduce_sec(grow_phases: &str) -> i64 {
    if grow_phases.is_empty() {
        return 0;
    }
    let phases: Vec<&str> = grow_phases.split(';').filter(|p| !p.is_empty()).collect();
    if phases.is_empty() {
        return 0;
    }
    let first = phases[0];
    if let Some((_, secs_str)) = first.rsplit_once(':') {
        secs_str.parse::<i64>().unwrap_or(0)
    } else {
        0
    }
}

/// 格式化秒数为可读字符串
#[must_use]
pub fn format_time(secs: i64) -> String {
    if secs < 60 {
        return format!("{secs}秒");
    }
    if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        return format!("{m}分{s}秒");
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if m > 0 {
        format!("{h}时{m}分")
    } else {
        format!("{h}时")
    }
}

/// 排序方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    Exp,
    Fert,
    Gold,
    Profit,
    FertProfit,
    Level,
}

impl Default for SortBy {
    fn default() -> Self {
        Self::Exp
    }
}

impl SortBy {
    /// 从字符串解析（容错）
    #[must_use]
    pub fn from_str_opt(s: &str) -> Self {
        match s {
            "fert" => Self::Fert,
            "gold" => Self::Gold,
            "profit" => Self::Profit,
            "fert_profit" => Self::FertProfit,
            "level" => Self::Level,
            _ => Self::Exp,
        }
    }
}

/// 植物排行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlantRanking {
    pub id: i64,
    pub seed_id: i64,
    pub name: String,
    pub seasons: i64,
    pub level: Option<i64>,
    pub grow_time: i64,
    pub grow_time_str: String,
    pub reduce_sec: i64,
    pub reduce_sec_applied: i64,
    pub exp_per_hour: f64,
    pub normal_fertilizer_exp_per_hour: f64,
    pub gold_per_hour: f64,
    pub profit_per_hour: f64,
    pub normal_fertilizer_profit_per_hour: f64,
    pub income: i64,
    pub net_profit: i64,
    pub fruit_id: i64,
    pub fruit_count: i64,
    pub fruit_price: i64,
    pub seed_price: i64,
    pub image: Option<String>,
}

/// 植物效率排行
#[must_use]
pub fn get_plant_rankings(sort_by: SortBy) -> Vec<PlantRanking> {
    let gc = global_game_config();
    let plants = gc.get_all_plants();
    let mut results = Vec::new();

    for plant in plants {
        // 筛选普通作物：必须有 seed_id 和 grow_phases
        let Some(seed_id) = plant.seed_id else {
            continue;
        };
        if seed_id <= 0 {
            continue;
        }
        let Some(ref grow_phases) = plant.grow_phases else {
            continue;
        };

        let base_grow_time = parse_grow_time(grow_phases);
        if base_grow_time <= 0 {
            continue;
        }

        let seasons = plant.seasons.unwrap_or(1);
        let is_two_season = seasons == 2;
        let grow_time = if is_two_season { base_grow_time * 3 / 2 } else { base_grow_time };

        // 经验
        let harvest_exp_base = plant.exp.unwrap_or(0);
        let harvest_exp = if is_two_season { harvest_exp_base * 2 } else { harvest_exp_base };
        let exp_per_hour =
            if grow_time > 0 { (harvest_exp as f64 / grow_time as f64) * 3600.0 } else { 0.0 };

        // 化肥减时
        let reduce_sec_base = parse_normal_fertilizer_reduce_sec(grow_phases);
        let reduce_sec_applied = if is_two_season { reduce_sec_base * 2 } else { reduce_sec_base };
        let fertilized_grow_time = grow_time - reduce_sec_applied;
        let safe_fertilized_time = if fertilized_grow_time > 0 { fertilized_grow_time } else { 1 };
        let normal_fertilizer_exp_per_hour =
            (harvest_exp as f64 / safe_fertilized_time as f64) * 3600.0;

        // 果实 / 种子
        let (fruit_id, fruit_count) = match &plant.fruit {
            Some(f) => (f.id, f.count),
            None => (0, 0),
        };
        let fruit_price = gc.get_fruit_price(fruit_id);
        let seed_price = gc.get_seed_price(seed_id);

        // 单次收获毛收入 / 净收入
        let income = fruit_count * fruit_price * if is_two_season { 2 } else { 1 };
        let net_profit = income - seed_price;
        let gold_per_hour =
            if grow_time > 0 { (income as f64 / grow_time as f64) * 3600.0 } else { 0.0 };
        let profit_per_hour =
            if grow_time > 0 { (net_profit as f64 / grow_time as f64) * 3600.0 } else { 0.0 };
        let normal_fertilizer_profit_per_hour =
            (net_profit as f64 / safe_fertilized_time as f64) * 3600.0;

        // 优先从 ItemInfo.json 获取种子等级（Plant.json 的 land_level_need 全为 1，不可用）
        let cfg_level =
            gc.get_item_by_id(seed_id).and_then(|i| i.level).or(plant.land_level_need).unwrap_or(0);
        let required_level = if cfg_level > 0 { Some(cfg_level) } else { None };

        results.push(PlantRanking {
            id: plant.id,
            seed_id,
            name: plant.name.clone(),
            seasons,
            level: required_level,
            grow_time,
            grow_time_str: format_time(grow_time),
            reduce_sec: reduce_sec_base,
            reduce_sec_applied,
            exp_per_hour: round2(exp_per_hour),
            normal_fertilizer_exp_per_hour: round2(normal_fertilizer_exp_per_hour),
            gold_per_hour: round2(gold_per_hour),
            profit_per_hour: round2(profit_per_hour),
            normal_fertilizer_profit_per_hour: round2(normal_fertilizer_profit_per_hour),
            income,
            net_profit,
            fruit_id,
            fruit_count,
            fruit_price,
            seed_price,
            image: gc.get_item_image_by_id(seed_id),
        });
    }

    // 排序
    match sort_by {
        SortBy::Exp => results.sort_by(|a, b| {
            b.exp_per_hour.partial_cmp(&a.exp_per_hour).unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortBy::Fert => results.sort_by(|a, b| {
            b.normal_fertilizer_exp_per_hour
                .partial_cmp(&a.normal_fertilizer_exp_per_hour)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortBy::Gold => results.sort_by(|a, b| {
            b.gold_per_hour.partial_cmp(&a.gold_per_hour).unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortBy::Profit => results.sort_by(|a, b| {
            b.profit_per_hour.partial_cmp(&a.profit_per_hour).unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortBy::FertProfit => results.sort_by(|a, b| {
            b.normal_fertilizer_profit_per_hour
                .partial_cmp(&a.normal_fertilizer_profit_per_hour)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        SortBy::Level => results.sort_by(|a, b| {
            let av = a.level.unwrap_or(-1);
            let bv = b.level.unwrap_or(-1);
            bv.cmp(&av)
        }),
    }

    results
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

// =====================================================================
// 单元测试
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_grow_time_sum() {
        assert_eq!(parse_grow_time("种子:30;发芽:30;成熟:0;"), 60);
        assert_eq!(parse_grow_time(""), 0);
        assert_eq!(parse_grow_time("a:10"), 10);
    }

    #[test]
    fn parse_normal_fertilizer_first_phase() {
        assert_eq!(parse_normal_fertilizer_reduce_sec("种子:30;发芽:30;"), 30);
        assert_eq!(parse_normal_fertilizer_reduce_sec(""), 0);
    }

    #[test]
    fn format_time_human() {
        assert_eq!(format_time(0), "0秒");
        assert_eq!(format_time(45), "45秒");
        assert_eq!(format_time(60), "1分0秒");
        assert_eq!(format_time(3700), "1时1分");
        assert_eq!(format_time(7200), "2时");
    }

    #[test]
    fn sort_by_from_str() {
        assert_eq!(SortBy::from_str_opt("exp"), SortBy::Exp);
        assert_eq!(SortBy::from_str_opt("gold"), SortBy::Gold);
        assert_eq!(SortBy::from_str_opt("profit"), SortBy::Profit);
        assert_eq!(SortBy::from_str_opt("unknown"), SortBy::Exp);
    }

    #[test]
    fn rankings_not_empty() {
        // 依赖 gameConfig
        let _ = global_game_config();
        let r = get_plant_rankings(SortBy::Exp);
        // gameConfig 内置 255 plants，应该非空
        assert!(!r.is_empty());
    }

    #[test]
    fn rankings_sorted_by_exp_desc() {
        let _ = global_game_config();
        let r = get_plant_rankings(SortBy::Exp);
        for w in r.windows(2) {
            assert!(w[0].exp_per_hour >= w[1].exp_per_hour);
        }
    }

    #[test]
    fn rankings_sorted_by_level() {
        let _ = global_game_config();
        let r = get_plant_rankings(SortBy::Level);
        for w in r.windows(2) {
            assert!(w[0].level.unwrap_or(-1) >= w[1].level.unwrap_or(-1));
        }
    }
}
