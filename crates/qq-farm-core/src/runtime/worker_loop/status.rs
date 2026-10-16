//! 状态广播：脏标记驱动 + 5s 量化 + 内容门控（对齐 node 哈希推送语义）。

use super::*;

impl WorkerLoop {
    /// 标记状态已变化（业务事件后调用，下一次 status_sync 立即重建广播）
    pub fn mark_status_dirty(&self) {
        self.status_dirty.store(true, Ordering::Release);
    }

    pub fn sync_status(&self) {
        let now_ms_val = now_ms();
        let fallback_due = now_ms_val - self.last_status_sent_ms.load(Ordering::Acquire)
            >= crate::constants::STATUS_FALLBACK_BROADCAST_MS as i64;
        let dirty = self.status_dirty.swap(false, Ordering::AcqRel);
        if !dirty && !fallback_due && self.last_status_sent_ms.load(Ordering::Acquire) > 0 {
            return;
        }
        self.last_status_sent_ms.store(now_ms_val, Ordering::Release);
        let st = status_svc::status_data_for(&self.account.id);
        let user = serde_json::json!({
            "name": st.name,
            "avatar": st.avatar,
            "level": st.level,
            "gold": st.gold,
            "exp": st.exp,
            "platform": st.platform,
            "coupon": *self.coupon.lock(),
            "goldBean": *self.gold_bean.lock(),
        });
        let connected = self.login_ready();
        let limits = self.friend.get_operation_limits();
        let mut full = crate::services::stats::get_stats_for(
            &self.account.id,
            Some(&user),
            Some(&user),
            connected,
            limits,
        );
        let session_exp = full
            .get("sessionExpGained")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0);
        let session_elapsed = full
            .get("sessionElapsedSeconds")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0);
        let now = now_ms();
        let next = self.next_runs.lock().clone();
        // 量化到 5s 桶：否则倒计时字段每 3s 必变，门控永远不命中
        let quantize = |ms: i64| ((ms / 1000).max(0) / 5) * 5;
        let farm = quantize(next.farm_at - now);
        let friend = quantize(next.friend_at - now);
        // help/steal 已合并为统一好友 tick（friend_at）；
        // 面板字段的旧键保留并镜像 friend 值，避免前端空值
        let help = friend;
        let steal = friend;
        let auto = crate::models::store::account_config::get_automation(Some(&self.account.id));
        let preferred =
            crate::models::store::account_config::get_preferred_seed(Some(&self.account.id));
        let (current, needed) =
            crate::config::game_config::global().get_level_exp_progress(st.level, st.exp);
        if let Some(obj) = full.as_object_mut() {
            obj.insert(
                "nextChecks".to_string(),
                serde_json::json!({
                    "farmRemainSec": farm,
                    "helpRemainSec": help,
                    "stealRemainSec": steal,
                    "friendRemainSec": help.max(steal),
                    // 静默中不再显示巡查倒计时（对齐 bot syncStatus nextChecks）
                    "farmQuiet": crate::services::friend::visit_strategy::in_farm_quiet_hours_for(
                        Some(&self.account.id), None),
                    "helpQuiet": crate::services::friend::visit_strategy::in_friend_quiet_hours_for(
                        Some(&self.account.id), None),
                    "stealQuiet": crate::services::friend::visit_strategy::in_friend_quiet_hours_for(
                        Some(&self.account.id), None),
                }),
            );
            obj.insert(
                "automation".to_string(),
                serde_json::to_value(&auto).unwrap_or(serde_json::json!({})),
            );
            obj.insert("preferredSeed".to_string(), serde_json::json!(preferred));
            obj.insert(
                "levelProgress".to_string(),
                serde_json::json!({ "current": current, "needed": needed }),
            );
            // 等级升级时间预测（bot 62e1880：fullStats.levelForecast）。
            // 土地快照以「登记时刻的 gid/exp + 180s 时效」校验，失效即按无排期计算。
            {
                use crate::services::level_forecast::{
                    calculate_level_forecast, LevelForecastInput, HISTORY_MIN_SECONDS,
                    SNAPSHOT_MAX_AGE_SEC,
                };
                let gateway = self.farm.api().gateway().clone();
                let now_sec = crate::utils::time::get_server_time_secs();
                let history_rate = if session_elapsed >= HISTORY_MIN_SECONDS as f64 {
                    session_exp / session_elapsed * 3600.0
                } else {
                    0.0
                };
                let lands = gateway
                    .valid_land_forecast_lands(
                        self.current_gid(),
                        st.exp,
                        now_sec,
                        SNAPSHOT_MAX_AGE_SEC,
                    )
                    .unwrap_or_default();
                let mut forecast = calculate_level_forecast(&LevelForecastInput {
                    lands,
                    now_sec,
                    remaining_exp: needed - current,
                    history_rate,
                    connected,
                    auto_harvest: auto.farm,
                    quiet: crate::services::friend::visit_strategy::in_farm_quiet_hours_for(
                        Some(&self.account.id),
                        None,
                    ),
                });
                // generatedAt 量化到 5s 桶（UI 只用它算「还有几分钟」）：否则 1s 精度
                // 会让状态广播的内容门控每轮都失效，每 3s 全量推送一次
                forecast.generated_at = (now_ms() / 5_000) * 5_000;
                obj.insert(
                    "levelForecast".to_string(),
                    serde_json::to_value(forecast).unwrap_or(serde_json::json!(null)),
                );
            }
            obj.insert(
                "configRevision".to_string(),
                serde_json::json!(self.applied_config_revision.load(Ordering::Acquire)),
            );
            obj.insert("accountId".to_string(), serde_json::json!(self.account.id));
            obj.insert("accountName".to_string(), serde_json::json!(self.account.display_name));
            obj.insert(
                "uptime".to_string(),
                // 同样量化，避免浮点秒每 tick 都变
                serde_json::json!((self.started_at.elapsed().as_secs() / 5) * 5),
            );
        }
        // 对齐 node（哈希变化或超时才推送）：内容未变化时跳过广播
        let payload = serde_json::to_string(&full).unwrap_or_default();
        {
            let mut last = self.last_status_json.lock();
            if *last == payload {
                return;
            }
            *last = payload;
        }
        let _ = self.event_tx.send(WorkerEvent::Status {
            account_id: self.account.id.clone(),
            account_name: self.account.display_name.clone(),
            status: full,
        });
    }
}
