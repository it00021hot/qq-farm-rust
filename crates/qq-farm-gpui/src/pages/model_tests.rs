use super::model::Page;

#[test]
fn page_summaries_are_localized_without_changing_dynamic_content() {
    assert_eq!(
        Page::Accounts.summary_for("账号连接状态与本地凭据", "en-US"),
        "Account connection and local credentials"
    );
    assert_eq!(
        Page::Accounts.summary_for("尚未添加账号，请先在侧栏添加账号", "en-US"),
        "No account added yet. Add one from the sidebar."
    );
    assert_eq!(
        Page::Dashboard.summary_for("服务器返回的状态摘要", "en-US"),
        "服务器返回的状态摘要"
    );
    assert_eq!(
        Page::Accounts.summary_for("账号连接状态与本地凭据", "zh-CN"),
        "账号连接状态与本地凭据"
    );
    assert_eq!(
        Page::Mall.summary_for("神秘商店限时商品", "en-US"),
        "Limited-time mystery shop items"
    );
    assert_eq!(
        Page::MysteryShop.summary_for("神秘商店限时商品", "en-US"),
        "Limited-time mystery shop items"
    );
}

#[test]
fn all_primary_pages_are_listed_once_and_have_persistent_ids() {
    assert_eq!(Page::ALL.len(), 10);
    for (index, page) in Page::ALL.iter().enumerate() {
        assert!(!Page::ALL[..index].contains(page), "duplicate sidebar page: {page:?}");
        assert_eq!(Page::from_id(page.id()), Some(*page));
    }
    assert_eq!(Page::from_id("mystery-shop"), Some(Page::MysteryShop));
}
