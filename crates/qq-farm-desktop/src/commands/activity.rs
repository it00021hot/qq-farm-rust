//! 活动中心快照与领取助手。

use serde_json::Value;
use tauri::State;

use qq_farm_app::accounts;
use qq_farm_app::activity;

use crate::error::{IpcError, IpcResult};
use crate::state::DesktopState;

fn ensure(state: &DesktopState, account_id: &str) -> IpcResult<()> {
    accounts::ensure_account_access(&state.acl, account_id).map_err(IpcError::from)
}

/// 活动中心快照。
#[tauri::command]
pub async fn activity_snapshot(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::snapshot(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 领取战令奖励。
#[tauri::command]
pub async fn activity_claim_battle_pass(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::claim_battle_pass(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 点亮星座。
#[tauri::command]
pub async fn activity_light_constellation(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::light_constellation(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 星沙兑换。
#[tauri::command]
pub async fn activity_exchange_star_sand(
    state: State<'_, DesktopState>,
    account_id: String,
    goods_id: Value,
    count: Value,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::exchange_star_sand(&state.app, &account_id, &goods_id, &count)
        .await
        .map_err(IpcError::from)
}

/// 领取节气奖励。
#[tauri::command]
pub async fn activity_claim_solar_term(
    state: State<'_, DesktopState>,
    account_id: String,
    term_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::claim_solar_term(&state.app, &account_id, &term_id).await.map_err(IpcError::from)
}

/// 领取青梅每日种子。
#[tauri::command]
pub async fn activity_claim_qingmei_seed(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::claim_qingmei_seed(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 开始青梅酿造。
#[tauri::command]
pub async fn activity_qingmei_brew_start(
    state: State<'_, DesktopState>,
    account_id: String,
    input: Option<Value>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::start_qingmei_brew(&state.app, &account_id, input.unwrap_or(Value::Null))
        .await
        .map_err(IpcError::from)
}

/// 继续青梅酿造。
#[tauri::command]
pub async fn activity_qingmei_brew_continue(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::continue_qingmei_brew(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 结算青梅酿造。
#[tauri::command]
pub async fn activity_qingmei_brew_settle(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::settle_qingmei_brew(&state.app, &account_id).await.map_err(IpcError::from)
}

/// 领取鹊桥阶段奖励。
#[tauri::command]
pub async fn activity_claim_qixi_bridge(
    state: State<'_, DesktopState>,
    account_id: String,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    activity::claim_qixi_bridge(&state.app, &account_id).await.map_err(IpcError::from)
}

fn json_i64(value: &Value) -> i64 {
    match value {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().map(|v| v as i64))
            .or_else(|| n.as_f64().map(|v| v.trunc() as i64))
            .unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// 赠送鹊羽香囊。
#[tauri::command]
pub async fn activity_gift_qixi_sachet(
    state: State<'_, DesktopState>,
    account_id: String,
    friend_gid: Value,
    sachet_count: Option<Value>,
    count: Option<Value>,
) -> IpcResult<Value> {
    ensure(&state, &account_id)?;
    let gid = json_i64(&friend_gid);
    let n = sachet_count.as_ref().or(count.as_ref()).map(json_i64).unwrap_or(0);
    activity::gift_qixi_sachet(&state.app, &account_id, gid, n).await.map_err(IpcError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_i64_reads_int_float_and_string() {
        assert_eq!(json_i64(&json!(3)), 3);
        assert_eq!(json_i64(&json!(2.9)), 2);
        assert_eq!(json_i64(&json!("4")), 4);
        assert_eq!(json_i64(&json!(null)), 0);
    }
}
