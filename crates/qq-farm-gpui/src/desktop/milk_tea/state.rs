use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, Context, Entity, Window};

use crate::desktop::FarmDesktop;

/// 奶茶活动页状态（页面数据在 `page_raw`）。
pub(crate) struct MilkTeaState {
    pub(crate) pending: bool,
    pub(crate) generation: u64,
    pub(crate) error: Option<String>,
    /// 开店 / 改名 / 加速 / 兑换数量输入
    pub(crate) create_name: Entity<InputState>,
    pub(crate) rename_name: Entity<InputState>,
    pub(crate) boost_count: Entity<InputState>,
    pub(crate) exchange_count: Entity<InputState>,
    /// 工坊选中的果实 uid 集合
    pub(crate) craft_selected: std::collections::HashSet<String>,
    /// 雇佣时长档（1-3）
    pub(crate) hire_duration_id: i64,
    /// 排行榜页码（1 起）
    pub(crate) rank_page: i64,
    /// 营业记录日期（0 = 服务端默认今天）
    pub(crate) logs_day: i64,
}

impl MilkTeaState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let shop_placeholder = crate::i18n::text(language, "店铺名", "Shop name").to_string();
        let quantity_placeholder = crate::i18n::text(language, "数量", "Quantity").to_string();
        let create_name =
            cx.new(|cx| InputState::new(window, cx).placeholder(shop_placeholder.clone()));
        let rename_name =
            cx.new(|cx| InputState::new(window, cx).placeholder(shop_placeholder.clone()));
        let boost_count = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(quantity_placeholder.clone());
            state.set_value("1", window, cx);
            state.set_min(Some(1.), window, cx);
            state.set_max(Some(9_999.), window, cx);
            state
        });
        let exchange_count = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(quantity_placeholder.clone());
            state.set_value("1", window, cx);
            state.set_min(Some(1.), window, cx);
            state.set_max(Some(9_999.), window, cx);
            state
        });
        Self {
            pending: false,
            generation: 0,
            error: None,
            create_name,
            rename_name,
            boost_count,
            exchange_count,
            craft_selected: std::collections::HashSet::new(),
            hire_duration_id: 1,
            rank_page: 1,
            logs_day: 0,
        }
    }
}
