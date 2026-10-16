//! 等级升级时间预测。
//!
//! 1:1 翻译原 `core/src/services/level-forecast.ts`（bot `62e1880`，2026-10-02）。
//!
//! 语义：
//! - 按当前地块上的作物（仅当前季有服务器成熟时间）推算未来每笔收获的经验与时间点；
//! - 经验 = `Plant.exp × (1 + 土地 plant_exp_bonus/10000)`，缺服务器 buff 数据则不猜；
//! - 纯「已排期」够升时按收获时间点给出 `planted/ready` 预测；
//! - 不够时若自动收获开启，用会话历史经验速率在最后一笔收获之后补足差额（`mixed/history`）；
//! - 土地快照由 `AllLands` 回包登记，任何种植写操作 / 土地变更推送 / 断线都会失效
//!   （快照存 [`crate::network::gateway::Gateway`]，每账号连接一份，随连接销毁）。

use serde::Serialize;

use crate::config::game_config::global as global_game_config;
use crate::proto::generated::gamepb::plantpb::LandInfo;

/// 快照最长有效时长（秒）：超过即视为过期（bot `SNAPSHOT_MAX_AGE_SEC`）
pub const SNAPSHOT_MAX_AGE_SEC: i64 = 180;
/// 会话不足该秒数不计算历史速率（bot `HISTORY_MIN_SECONDS`）
pub const HISTORY_MIN_SECONDS: i64 = 60;

/// 土地快照：`AllLands` 回包 + 登记时刻的用户态（bot 模块级 `snapshot` 变量）
#[derive(Debug, Clone, Default)]
pub struct ForecastSnapshot {
    pub lands: Vec<LandInfo>,
    pub gid: i64,
    pub exp: i64,
    /// 登记时刻（服务器秒）
    pub at: i64,
}

/// 一笔未来收获事件
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HarvestEvent {
    pub land_id: i64,
    /// 成熟时间（秒）；已成熟的取 `max(now, begin_time)`
    pub at: i64,
    pub exp: i64,
}

/// 升级预测结果（序列化键对齐 bot `calculateLevelForecast` 返回值）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelForecast {
    pub generated_at: i64,
    pub remaining_exp: i64,
    pub scheduled_exp: i64,
    pub plot_count: usize,
    pub ready_exp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_at: Option<i64>,
    pub source: &'static str,
    pub automatic: bool,
}

/// 毫秒/秒时间归一：>1e12 视为毫秒（对齐 TS `seconds`）
fn seconds(value: i64) -> i64 {
    if value > 1_000_000_000_000 {
        value / 1000
    } else {
        value
    }
}

/// 每个种植足迹只计一次收获；只有当前季有服务器提供的成熟时间。
pub fn build_harvest_events(lands: &[LandInfo], now_sec: i64) -> Vec<HarvestEvent> {
    let gc = global_game_config();
    build_harvest_events_with(lands, now_sec, &|id| gc.get_plant_by_id(id))
}

/// [`build_harvest_events`] 的可注入变体（对齐 TS `getPlant` 参数，测试用）
pub fn build_harvest_events_with(
    lands: &[LandInfo],
    now_sec: i64,
    get_plant: &dyn Fn(i64) -> Option<crate::config::game_config::Plant>,
) -> Vec<HarvestEvent> {
    let mut events: Vec<HarvestEvent> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let slaves: std::collections::HashSet<i64> =
        lands.iter().flat_map(|land| land.slave_land_ids.iter().copied()).collect();
    for land in lands {
        let id = land.id;
        let master = land.master_land_id;
        if id == 0
            || seen.contains(&id)
            || !land.unlocked
            || slaves.contains(&id)
            || (master != 0 && master != id)
        {
            continue;
        }
        seen.insert(id);
        let Some(plant) = land.plant.as_ref() else { continue };
        if plant.phases.is_empty() {
            continue;
        }
        // 过滤掉 begin_time 为 0 的阶段
        let phases: Vec<&crate::proto::generated::gamepb::plantpb::PlantPhaseInfo> =
            plant.phases.iter().filter(|phase| seconds(phase.begin_time) > 0).collect();
        // 协议只下发「从当前阶段开始」的后缀；时钟不会推进该状态。
        let current = plant.phases.first();
        if current.is_some_and(|p| i32::from(p.phase) == 7 || i32::from(p.phase) == 0) {
            continue;
        }
        let config = get_plant(plant.id);
        let configured_phases: Vec<&str> = config
            .as_ref()
            .and_then(|c| c.grow_phases.as_deref())
            .unwrap_or_default()
            .split(';')
            .filter(|part| !part.is_empty())
            .collect();
        let final_name = configured_phases.last().and_then(|part| part.split(':').next());
        // 服务器成熟标记：phase 6/19 或 phase_id 19
        let explicit_mature = phases.iter().find(|phase| {
            i32::from(phase.phase) == 6 || i32::from(phase.phase) == 19 || phase.phase_id == 19
        });
        let final_phase = phases.last();
        let mature = match explicit_mature {
            Some(phase) => Some(*phase),
            None => {
                let blooming = matches!(final_name, Some("成熟") | Some("盛开"));
                let not_extra_season = plant.phases.len() <= configured_phases.len();
                let phase_two = final_phase.is_some_and(|p| i32::from(p.phase) == 2);
                if blooming && not_extra_season && phase_two {
                    final_phase.copied()
                } else {
                    None
                }
            }
        };
        let Some(mature) = mature else { continue };
        // 缺服务器加成数据按「未知」处理，不按土地颜色猜零加成
        let (Some(config), Some(buff)) = (config.as_ref(), land.buff.as_ref()) else {
            continue;
        };
        let bonus = buff.plant_exp_bonus;
        let exp = ((config.exp.unwrap_or(0)) as f64 * (1.0 + bonus.max(0) as f64 / 10_000.0))
            .floor() as i64;
        if exp <= 0 {
            continue;
        }
        events.push(HarvestEvent { land_id: id, at: now_sec.max(seconds(mature.begin_time)), exp });
    }
    events.sort_by(|a, b| a.at.cmp(&b.at).then(a.land_id.cmp(&b.land_id)));
    events
}

/// 预测输入
#[derive(Debug, Clone, Default)]
pub struct LevelForecastInput {
    /// 当前土地快照（无效/过期时传空）
    pub lands: Vec<LandInfo>,
    pub now_sec: i64,
    pub remaining_exp: i64,
    /// 每小时历史经验速率
    pub history_rate: f64,
    pub connected: bool,
    /// 自动收获开启（且非静默时段）才允许历史外推
    pub auto_harvest: bool,
    pub quiet: bool,
}

/// 计算升级预测（对齐 TS `calculateLevelForecast`）。
pub fn calculate_level_forecast(options: &LevelForecastInput) -> LevelForecast {
    let gc = global_game_config();
    calculate_level_forecast_with(options, &|id| gc.get_plant_by_id(id))
}

/// [`calculate_level_forecast`] 的可注入变体（测试用）
pub fn calculate_level_forecast_with(
    options: &LevelForecastInput,
    get_plant: &dyn Fn(i64) -> Option<crate::config::game_config::Plant>,
) -> LevelForecast {
    let now_sec = options.now_sec;
    let remaining_exp = options.remaining_exp.max(0);
    let automatic = options.auto_harvest && !options.quiet;
    let mut base = LevelForecast {
        generated_at: now_sec * 1000,
        remaining_exp,
        scheduled_exp: 0,
        plot_count: 0,
        ready_exp: 0,
        estimated_at: None,
        source: "unavailable",
        automatic,
    };
    if !options.connected || remaining_exp <= 0 {
        return base;
    }
    let events = build_harvest_events_with(&options.lands, now_sec, get_plant);
    base.scheduled_exp = events.iter().map(|event| event.exp).sum();
    base.plot_count = events.len();
    base.ready_exp = events.iter().filter(|event| event.at <= now_sec).map(|event| event.exp).sum();
    let mut total: i64 = 0;
    for event in &events {
        total += event.exp;
        if total >= remaining_exp {
            base.source = if event.at <= now_sec { "ready" } else { "planted" };
            base.estimated_at = Some(event.at * 1000);
            return base;
        }
    }
    // 历史速率与已排期收获不重复计数；只在最后一笔已知收获之后外推剩余差额。
    let rate = options.history_rate.max(0.0);
    if rate <= 0.0 || !automatic {
        return base;
    }
    let start_at = events.last().map_or(now_sec, |event| event.at);
    let gap_hours = (remaining_exp - total) as f64 / rate;
    base.source = if events.is_empty() { "history" } else { "mixed" };
    base.estimated_at = Some((start_at + gap_hours.ceil() as i64 * 3600) * 1000);
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::game_config::Plant;
    use crate::proto::generated::gamepb::plantpb::land_info::Buff;
    use crate::proto::generated::gamepb::plantpb::{PlantInfo, PlantPhaseInfo};

    const NOW: i64 = 1_800_000_000;

    /// 对齐 bot 测试夹具：plant 1 → exp 856，plant 2 → exp 1000（可配 grow_phases）
    fn test_plants() -> impl Fn(i64) -> Option<Plant> {
        |id| match id {
            1 => Some(Plant { id: 1, exp: Some(856), ..Default::default() }),
            2 => Some(Plant { id: 2, exp: Some(1_000), ..Default::default() }),
            _ => None,
        }
    }

    fn phase(phase: i32, begin_time: i64) -> PlantPhaseInfo {
        PlantPhaseInfo { phase, begin_time, phase_id: 0, ..Default::default() }
    }

    fn land(id: i64, mature_at: i64) -> LandInfo {
        LandInfo {
            id,
            unlocked: true,
            level: 4,
            buff: Some(Buff { plant_exp_bonus: 0, ..Default::default() }),
            plant: Some(PlantInfo {
                id: 1,
                phases: vec![phase(1, NOW - 100), phase(6, mature_at)],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn predict(options: LevelForecastInput) -> LevelForecast {
        let plants = test_plants();
        calculate_level_forecast_with(&options, &plants)
    }

    /// 24 块地全部 6 小时后成熟：scheduledExp=24×856，六小时后可升（bot issue 86 回归）
    #[test]
    fn scheduled_harvest_reaches_threshold_after_fresh_login() {
        let lands: Vec<LandInfo> = (0..24).map(|i| land(i + 1, NOW + 21_600 + i)).collect();
        let value = predict(LevelForecastInput {
            lands,
            now_sec: NOW,
            remaining_exp: 12_660,
            history_rate: 1121.0,
            connected: true,
            auto_harvest: true,
            quiet: false,
        });
        assert_eq!(value.source, "planted");
        assert_eq!(value.scheduled_exp, 20_544);
        assert_eq!(value.plot_count, 24);
        assert_eq!(value.estimated_at, Some((NOW + 21_614) * 1000));
        // 历史速率不改变「已排期够升」的预测
        let no_rate = predict(LevelForecastInput {
            lands: (0..24).map(|i| land(i + 1, NOW + 21_600 + i)).collect(),
            now_sec: NOW,
            remaining_exp: 12_660,
            history_rate: 0.0,
            connected: true,
            auto_harvest: true,
            quiet: false,
        });
        assert_eq!(no_rate.estimated_at, value.estimated_at);
    }

    /// 共享足迹与重复土地条目只计一次收获；master 的加成计入
    #[test]
    fn joint_footprints_contribute_only_one_harvest() {
        let plants = test_plants();
        let mut master = land(1, NOW + 100);
        master.slave_land_ids = vec![2, 3, 4];
        if let Some(buff) = master.buff.as_mut() {
            buff.plant_exp_bonus = 2_500;
        }
        let mut slave = land(3, NOW + 100);
        slave.master_land_id = 1;
        let events = build_harvest_events_with(
            &[master.clone(), master, land(2, NOW + 21_600), slave, land(4, NOW + 21_600)],
            NOW,
            &plants,
        );
        assert_eq!(events, vec![HarvestEvent { land_id: 1, at: NOW + 100, exp: 1_070 }]);
    }

    /// 未解锁 / 无 buff / 枯死 / 未成熟 / 无配置的地块全部排除
    #[test]
    fn excludes_locked_dead_unconfigured_and_unmature() {
        let plants = test_plants();
        let mut locked = land(1, NOW);
        locked.unlocked = false;
        let mut no_buff = land(2, NOW);
        no_buff.buff = None;
        let mut dead = land(3, NOW);
        if let Some(plant) = dead.plant.as_mut() {
            plant.phases = vec![phase(7, NOW - 1)];
        }
        let mut growing = land(4, NOW);
        if let Some(plant) = growing.plant.as_mut() {
            plant.phases = vec![phase(1, NOW - 1)];
        }
        let mut unconfigured = land(5, NOW);
        if let Some(plant) = unconfigured.plant.as_mut() {
            plant.id = 99;
            plant.phases = vec![phase(6, NOW)];
        }
        assert!(build_harvest_events_with(
            &[locked, no_buff, dead, growing, unconfigured],
            NOW,
            &plants
        )
        .is_empty());
    }

    /// 服务器成熟用 phase 6，不被后面的枯死阶段或多季猜测带偏
    #[test]
    fn maturity_uses_phase_six_not_later_dead_phase() {
        let plants = test_plants();
        let mut entry = land(1, NOW + 100);
        if let Some(plant) = entry.plant.as_mut() {
            plant.season = 1;
            plant.phases.push(phase(7, NOW + 1000));
        }
        let events = build_harvest_events_with(&[entry], NOW, &plants);
        assert_eq!(events[0].at, NOW + 100);
    }

    /// 已成熟仍取决于是否收取；手动/静默模式无历史外推
    #[test]
    fn ready_harvest_and_manual_quiet_semantics() {
        let ready = predict(LevelForecastInput {
            lands: vec![land(1, NOW - 10)],
            now_sec: NOW,
            remaining_exp: 500,
            history_rate: 1121.0,
            connected: true,
            auto_harvest: false,
            quiet: false,
        });
        assert_eq!(ready.source, "ready");
        assert!(!ready.automatic);
        assert_eq!(ready.estimated_at, Some(NOW * 1000));

        for (auto_harvest, quiet) in [(false, false), (true, true)] {
            let value = predict(LevelForecastInput {
                lands: Vec::new(),
                now_sec: NOW,
                remaining_exp: 500,
                history_rate: 1121.0,
                connected: true,
                auto_harvest,
                quiet,
            });
            assert_eq!(value.estimated_at, None);
        }
        let disconnected = predict(LevelForecastInput {
            lands: vec![land(1, NOW + 100)],
            now_sec: NOW,
            remaining_exp: 500,
            history_rate: 1121.0,
            connected: false,
            auto_harvest: true,
            quiet: false,
        });
        assert_eq!(disconnected.estimated_at, None);
        assert_eq!(disconnected.source, "unavailable");
    }

    /// 配置末段「盛开」且阶段数不多于配置时按末阶段时间预测，不多猜一季
    #[test]
    fn configured_final_blooming_phase_is_forecast() {
        let plants = |id: i64| {
            Some(Plant {
                id,
                exp: Some(100),
                grow_phases: Some("种子:50;生长:50;盛开:0".into()),
                ..Default::default()
            })
        };
        let mut blooming = land(2, NOW);
        if let Some(plant) = blooming.plant.as_mut() {
            plant.id = 2;
            plant.phases = vec![phase(2, NOW - 100), phase(2, NOW + 50)];
        }
        let events = build_harvest_events_with(&[blooming], NOW, &plants);
        assert_eq!(events[0].at, NOW + 50);
    }

    /// 历史速率只在最后一笔排期收获之后外推，不重复计数
    #[test]
    fn history_extends_gap_after_last_scheduled_harvest() {
        let value = predict(LevelForecastInput {
            lands: vec![land(1, NOW + 7_200)],
            now_sec: NOW,
            remaining_exp: 1_000,
            history_rate: 100.0,
            connected: true,
            auto_harvest: true,
            quiet: false,
        });
        assert_eq!(value.source, "mixed");
        // 856 排期 + 差 144 ÷ 100/h = 1.44h → 向上取整 2h = 7200s
        assert_eq!(value.estimated_at, Some((NOW + 7_200 + 7_200) * 1000));

        let history_only = predict(LevelForecastInput {
            lands: Vec::new(),
            now_sec: NOW,
            remaining_exp: 1_000,
            history_rate: 100.0,
            connected: true,
            auto_harvest: true,
            quiet: false,
        });
        assert_eq!(history_only.source, "history");

        let no_rate = predict(LevelForecastInput {
            lands: Vec::new(),
            now_sec: NOW,
            remaining_exp: 1_000,
            history_rate: 0.0,
            connected: true,
            auto_harvest: true,
            quiet: false,
        });
        assert_eq!(no_rate.estimated_at, None);
    }
}
