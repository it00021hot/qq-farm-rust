//! 同一账号的活动状态文件一次只允许一个读写改。
//! 千星和公益写的是同一个 `activity-center-state-{账号}.json`。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

fn locks() -> &'static Mutex<HashMap<String, Arc<Mutex<()>>>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn with_account_state<T>(account_id: &str, body: impl FnOnce() -> T) -> T {
    let mutex = {
        let mut map = locks().lock().expect("activity state locks");
        map.entry(account_id.to_owned()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    };
    let _guard = mutex.lock().expect("activity state");
    body()
}
