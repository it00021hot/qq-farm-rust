//! 刷新 `ActivityService.List` 活动窗口（出售条件 + 活动目录）。

use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use prost::Message;

use crate::config::activity_windows::{
    activity_windows_fresh, activity_windows_loaded, set_activity_windows, ActivityWindow,
};
use crate::constants::{ACTIVITY_SERVICE, ACTIVITY_WINDOWS_RETRY_LOG_INTERVAL_MS};
use crate::error::Result;
use crate::network::gateway::Gateway;
use crate::proto::generated::gamepb::activitypb::{ActivityListReply, ActivityListRequest};

static LAST_FAILURE_LOG: Mutex<Option<Instant>> = Mutex::new(None);

fn decode_windows(reply: &ActivityListReply) -> Vec<ActivityWindow> {
    reply
        .activity_windows
        .iter()
        .filter_map(|row| {
            let id = if row.id > 0 { row.id.to_string() } else { String::new() };
            if id.is_empty() {
                return None;
            }
            Some(ActivityWindow {
                id,
                name: row.name.clone(),
                begin_time: row.begin_time,
                end_time: row.end_time,
            })
        })
        .collect()
}

/// 过期则拉 List；失败时保留旧缓存。
pub async fn ensure_activity_windows(gateway: &Arc<Gateway>) -> Result<()> {
    if activity_windows_fresh() {
        return Ok(());
    }
    match refresh_activity_windows(gateway).await {
        Ok(()) => Ok(()),
        Err(err) if activity_windows_loaded() => {
            let mut last = LAST_FAILURE_LOG.lock();
            let should_log = match *last {
                None => true,
                Some(at) => {
                    at.elapsed().as_millis() >= u128::from(ACTIVITY_WINDOWS_RETRY_LOG_INTERVAL_MS)
                }
            };
            if should_log {
                *last = Some(Instant::now());
                tracing::warn!("刷新活动窗口失败，沿用缓存: {err}");
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn refresh_activity_windows(gateway: &Arc<Gateway>) -> Result<()> {
    let body = gateway
        .request(ACTIVITY_SERVICE, "List", &ActivityListRequest {}.encode_to_vec())
        .await?;
    let reply = ActivityListReply::decode(&body[..])?;
    let windows = decode_windows(&reply);
    if windows.is_empty() {
        return Err(crate::error::Error::Business("活动列表回包未包含时间配置".into()));
    }
    set_activity_windows(windows);
    Ok(())
}
