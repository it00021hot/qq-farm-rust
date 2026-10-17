use super::model::Page;

#[test]
fn all_primary_pages_are_listed_once_and_have_persistent_ids() {
    assert_eq!(Page::ALL.len(), 13);
    for (index, page) in Page::ALL.iter().enumerate() {
        assert!(!Page::ALL[..index].contains(page), "duplicate sidebar page: {page:?}");
        assert_eq!(Page::from_id(page.id()), Some(*page));
    }
    assert_eq!(Page::from_id("mystery-shop"), Some(Page::MysteryShop));
    assert_eq!(Page::from_id("activities"), Some(Page::Activities));
}
