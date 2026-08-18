//! 登录后的用户运行时状态。
//!
//! 对应原 network.ts 里的 `userState` 对象 + `userBound` 标志。

use std::sync::RwLock;

/// 用户运行时状态（登录后才有意义）
#[derive(Debug, Clone, Default)]
pub struct UserState {
    /// 用户 GID
    pub gid: i64,
    /// 用户昵称
    pub name: String,
    /// 等级
    pub level: i32,
    /// 金币
    pub gold: i64,
    /// 经验值
    pub exp: i64,
    /// 点券
    pub coupon: i64,
    /// 金豆豆
    pub gold_bean: i64,
    /// openid（游戏会话标识）
    pub open_id: String,
}

/// 线程安全的用户状态容器
#[derive(Debug, Default)]
pub struct UserStateStore {
    inner: RwLock<UserState>,
}

impl UserStateStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 读取快照
    #[must_use]
    pub fn snapshot(&self) -> UserState {
        self.inner.read().expect("user_state poisoned").clone()
    }

    /// 整体替换（登录成功时调用）
    pub fn replace(&self, new: UserState) {
        *self.inner.write().expect("user_state poisoned") = new;
    }

    /// 部分更新（登录后从背包/推送同步字段）
    pub fn update<F: FnOnce(&mut UserState)>(&self, f: F) {
        let mut guard = self.inner.write().expect("user_state poisoned");
        f(&mut guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_default() {
        let s = UserStateStore::new();
        let snap = s.snapshot();
        assert_eq!(snap.gid, 0);
        assert_eq!(snap.name, "");
    }

    #[test]
    fn update_gold() {
        let s = UserStateStore::new();
        s.update(|u| u.gold = 100);
        s.update(|u| u.gold += 50);
        assert_eq!(s.snapshot().gold, 150);
    }

    #[test]
    fn replace_full() {
        let s = UserStateStore::new();
        s.replace(UserState { gid: 12345, name: "tester".into(), level: 10, ..Default::default() });
        let snap = s.snapshot();
        assert_eq!(snap.gid, 12345);
        assert_eq!(snap.name, "tester");
        assert_eq!(snap.level, 10);
    }
}
