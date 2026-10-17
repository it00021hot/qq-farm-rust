use crate::desktop::FarmDesktop;
use gpui_kit::component::input::InputState;
use gpui_kit::component::VirtualListScrollHandle;
use gpui_kit::{AppContext as _, Context, Entity, Window};

pub(crate) struct IllustratedState {
    pub(crate) book: String,
    /// 图鉴网格滚动句柄（跨帧持久，滚动位置不随重渲染丢失）
    pub(crate) grid_scroll_handle: VirtualListScrollHandle,
    /// 图鉴种子购买弹窗（bot 0f09702：图鉴直达商店购买）
    pub(crate) purchase_count: Entity<InputState>,
    pub(crate) pending_purchase: Option<serde_json::Value>,
    pub(crate) purchase_pending: bool,
    pub(crate) purchase_generation: u64,
    pub(crate) purchase_dialog_open: bool,
    pub(crate) notice: Option<(String, bool)>,
    pub(crate) notice_generation: u64,
    /// 手动领取图鉴奖励在途
    pub(crate) claim_pending: bool,
    pub(crate) claim_generation: u64,
}

impl IllustratedState {
    pub(in crate::desktop) fn new(
        language: &str,
        window: &mut Window,
        cx: &mut Context<FarmDesktop>,
    ) -> Self {
        let purchase_count = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder(crate::i18n::text(language, "数量", "Quantity"));
            state.set_value("1", window, cx);
            state.set_min(Some(1.), window, cx);
            // 上限按条目 remainingPurchaseCount 动态放宽（bot：9999 兜底）
            state.set_max(Some(9_999.), window, cx);
            state
        });
        Self {
            book: "crop".into(),
            grid_scroll_handle: VirtualListScrollHandle::new(),
            purchase_count,
            pending_purchase: None,
            purchase_pending: false,
            purchase_generation: 0,
            purchase_dialog_open: false,
            notice: None,
            notice_generation: 0,
            claim_pending: false,
            claim_generation: 0,
        }
    }
}
