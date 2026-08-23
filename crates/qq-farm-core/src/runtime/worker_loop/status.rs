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
        let now = now_ms();
        let next = self.next_runs.lock().clone();
        // 量化到 5s 桶：否则倒计时字段每 3s 必变，门控永远不命中
        let quantize = |ms: i64| (((ms / 1000).max(0) / 5) * 5) as i64;
        let farm = quantize(next.farm_at - now);
        let help = quantize(next.help_at - now);
        let steal = quantize(next.steal_at - now);
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
