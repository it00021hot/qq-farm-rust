//! 牧场待核对操作。结果未知的扣费会留下来，核对前不再对同一目标重复提交。

use std::future::Future;
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::network::error::NetworkError;

const PENDING_MESSAGE: &str = "操作结果待核对";
const INVALID_FILE: &str = "牧场待核对记录格式错误，请检查数据文件";

#[derive(Clone, Debug)]
pub struct SpendCost {
    pub item_id: i64,
    pub amount: i64,
}

#[derive(Clone, Debug)]
pub struct SpendBalance {
    pub item_id: i64,
    pub total: i64,
    pub locked: i64,
}

#[derive(Clone, Debug)]
pub struct SpendPlan {
    pub target: String,
    pub label: String,
    pub costs: Vec<SpendCost>,
    pub balances: Vec<SpendBalance>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct PendingOperation {
    id: String,
    target: String,
    label: String,
    costs: Vec<StoredCost>,
    #[serde(rename = "createdAt")]
    created_at: i64,
    message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct StoredCost {
    #[serde(rename = "resourceKey")]
    resource_key: String,
    amount: String,
}

/// 记下费用后再执行。成功或服务端明确拒绝时清除记录；结果对不上则保留。
pub async fn spend<T, F, Fut>(account_id: &str, plan: SpendPlan, run: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let record = reserve(account_id, &plan)?;
    match run().await {
        Ok(value) => {
            clear(account_id, &record.id)?;
            Ok(value)
        }
        Err(error) if !outcome_unknown(&error) => {
            clear(account_id, &record.id)?;
            Err(error)
        }
        Err(error) => {
            let _ = note(account_id, &record.id, &error.to_string());
            Err(error)
        }
    }
}

/// 当前待核对记录，供牧场页展示。
pub fn pending_json(account_id: &str) -> Result<Vec<Value>> {
    Ok(load(account_id)?
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "target": row.target,
                "label": row.label,
                "message": row.message,
                "costs": row.costs.iter().map(|cost| json!({
                    "resourceKey": cost.resource_key,
                    "amount": cost.amount,
                })).collect::<Vec<_>>(),
            })
        })
        .collect())
}

/// 人工核对后解除保护。
pub fn acknowledge(account_id: &str, id: &str) -> Result<()> {
    let mut records = load(account_id)?;
    if !records.iter().any(|row| row.id == id) {
        return Err(Error::Business("这条记录已处理，请刷新".to_owned()));
    }
    records.retain(|row| row.id != id);
    save(account_id, &records)
}

fn reserve(account_id: &str, plan: &SpendPlan) -> Result<PendingOperation> {
    let _guard = file_lock().lock();
    let records = read_file(&operations_file(account_id))?;
    if records.iter().any(|row| row.target == plan.target) {
        return Err(Error::Business("该操作的上次结果未知，请在牧场页面核对后解除保护".to_owned()));
    }
    for cost in &plan.costs {
        if cost.amount <= 0 {
            continue;
        }
        let Some(balance) = plan.balances.iter().find(|balance| balance.item_id == cost.item_id)
        else {
            return Err(Error::Business("牧场资源余额未确认".to_owned()));
        };
        let reserved = reserved_amount(&records, &resource_key(cost.item_id));
        let available = balance.total.saturating_sub(balance.locked).saturating_sub(reserved);
        if available < cost.amount {
            return Err(Error::Business("扣除待核对操作的预留资源后余额不足".to_owned()));
        }
    }
    let record = PendingOperation {
        id: uuid::Uuid::new_v4().to_string(),
        target: plan.target.clone(),
        label: plan.label.clone(),
        costs: plan
            .costs
            .iter()
            .filter(|cost| cost.amount > 0)
            .map(|cost| StoredCost {
                resource_key: resource_key(cost.item_id),
                amount: cost.amount.to_string(),
            })
            .collect(),
        created_at: crate::utils::time::now_ms(),
        message: PENDING_MESSAGE.to_owned(),
    };
    let mut records = records;
    records.push(record.clone());
    write_file(&operations_file(account_id), &records)?;
    Ok(record)
}

fn clear(account_id: &str, id: &str) -> Result<()> {
    let _guard = file_lock().lock();
    let mut records = read_file(&operations_file(account_id))?;
    records.retain(|row| row.id != id);
    write_file(&operations_file(account_id), &records)
}

fn note(account_id: &str, id: &str, message: &str) -> Result<()> {
    let _guard = file_lock().lock();
    let mut records = read_file(&operations_file(account_id))?;
    if let Some(row) = records.iter_mut().find(|row| row.id == id) {
        row.message = message.chars().take(300).collect();
    }
    write_file(&operations_file(account_id), &records)
}

fn load(account_id: &str) -> Result<Vec<PendingOperation>> {
    let _guard = file_lock().lock();
    read_file(&operations_file(account_id))
}

fn save(account_id: &str, records: &[PendingOperation]) -> Result<()> {
    let _guard = file_lock().lock();
    write_file(&operations_file(account_id), records)
}

fn reserved_amount(records: &[PendingOperation], key: &str) -> i64 {
    records
        .iter()
        .flat_map(|row| row.costs.iter())
        .filter(|cost| cost.resource_key == key)
        .filter_map(|cost| cost.amount.parse::<i64>().ok())
        .fold(0, i64::saturating_add)
}

fn resource_key(item_id: i64) -> String {
    format!("item:{item_id}")
}

fn outcome_unknown(error: &Error) -> bool {
    !matches!(error, Error::Business(_) | Error::Network(NetworkError::Gateway { .. }))
}

fn file_lock() -> &'static Mutex<()> {
    static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn operations_file(account_id: &str) -> PathBuf {
    let digest = Sha256::digest(account_id.as_bytes());
    operations_root().join("ranch-operations").join(format!("{}.json", hex::encode(digest)))
}

fn operations_root() -> PathBuf {
    #[cfg(test)]
    if let Some(dir) = test_root() {
        return dir;
    }
    crate::config::paths::get_data_dir()
}

fn read_file(path: &Path) -> Result<Vec<PendingOperation>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path).map_err(|error| Error::Internal(error.to_string()))?;
    let records: Vec<PendingOperation> =
        serde_json::from_str(&text).map_err(|_| Error::Protocol(INVALID_FILE.to_owned()))?;
    if records.iter().any(|row| !valid_record(row)) {
        return Err(Error::Protocol(INVALID_FILE.to_owned()));
    }
    Ok(records)
}

fn valid_record(row: &PendingOperation) -> bool {
    !row.id.is_empty()
        && !row.target.is_empty()
        && row.costs.iter().all(|cost| {
            cost.resource_key
                .strip_prefix("item:")
                .is_some_and(|id| !id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()))
                && !cost.amount.is_empty()
                && cost.amount.chars().all(|ch| ch.is_ascii_digit())
        })
}

fn write_file(path: &Path, records: &[PendingOperation]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| Error::Internal(error.to_string()))?;
    }
    crate::infra::json_db::write_json_file_atomic(path, &records)
        .map_err(|error| Error::Internal(error.to_string()))
}

#[cfg(test)]
thread_local! {
    static TEST_ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn test_root() -> Option<PathBuf> {
    TEST_ROOT.with(|root| root.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRoot {
        dir: PathBuf,
    }

    impl TempRoot {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "qq-farm-ranch-ops-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&dir).expect("temp dir");
            TEST_ROOT.with(|root| *root.borrow_mut() = Some(dir.clone()));
            Self { dir }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            TEST_ROOT.with(|root| *root.borrow_mut() = None);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn plan(target: &str, amount: i64, total: i64) -> SpendPlan {
        SpendPlan {
            target: target.to_owned(),
            label: "购买牧草".to_owned(),
            costs: vec![SpendCost { item_id: 1007, amount }],
            balances: vec![SpendBalance { item_id: 1007, total, locked: 0 }],
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_unconfirmed_purchase_blocks_the_same_target() {
        let _root = TempRoot::new();
        let account = "ranch-ops";
        spend(account, plan("ranch:buy:10001", 8, 20), || async { Ok(()) }).await.expect("clear");
        assert!(pending_json(account).expect("empty").is_empty());

        let error = spend(account, plan("ranch:buy:10001", 8, 20), || async {
            Err::<(), _>(Error::Protocol("商品或扣费不匹配".to_owned()))
        })
        .await
        .expect_err("kept");
        assert!(error.to_string().contains("商品或扣费不匹配"));
        let pending = pending_json(account).expect("pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0]["label"], "购买牧草");

        let blocked = spend(account, plan("ranch:buy:10001", 1, 20), || async { Ok(()) })
            .await
            .expect_err("blocked");
        assert!(blocked.to_string().contains("上次结果未知"));

        let short = spend(account, plan("ranch:buy:10002", 15, 20), || async { Ok(()) })
            .await
            .expect_err("reserved");
        assert!(short.to_string().contains("预留资源后余额不足"));

        let rejected = spend(account, plan("ranch:upgrade:9", 1, 20), || async {
            Err::<(), _>(Error::Network(NetworkError::Gateway {
                code: 1,
                service_name: "gamepb.homesteadpb.HomesteadService".to_owned(),
                method_name: "UpgradeBuilding".to_owned(),
                error_message: "拒绝".to_owned(),
                client_seq: 1,
            }))
        })
        .await
        .expect_err("rejected");
        assert!(matches!(rejected, Error::Network(_)));
        assert_eq!(pending_json(account).expect("still one").len(), 1);

        let id = pending[0]["id"].as_str().expect("id").to_owned();
        acknowledge(account, &id).expect("ack");
        assert!(pending_json(account).expect("cleared").is_empty());
        assert!(acknowledge(account, &id).is_err());
    }
}
