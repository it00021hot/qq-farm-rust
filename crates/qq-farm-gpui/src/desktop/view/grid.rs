use gpui_kit::{px, Window};

pub(crate) fn responsive_grid_columns(
    window: &Window,
    sidebar_collapsed: bool,
    sidebar_width: u32,
    card_width: f32,
    gap: f32,
    horizontal_padding: f32,
) -> usize {
    let sidebar_width =
        if sidebar_collapsed { window.rem_size().as_f32() * 4. } else { sidebar_width as f32 };
    let available_width =
        (window.viewport_size().width.as_f32() - sidebar_width - horizontal_padding)
            .max(card_width);
    ((available_width + gap) / (card_width + gap)).floor().max(1.) as usize
}

pub(crate) fn chunk_indices(indices: &[usize], columns: usize) -> Vec<Vec<usize>> {
    let columns = columns.max(1);
    indices.chunks(columns).map(<[usize]>::to_vec).collect()
}

pub(crate) fn estimated_grid_rows(
    count: usize,
    height: f32,
) -> Vec<gpui_kit::Size<gpui_kit::Pixels>> {
    vec![gpui_kit::Size::new(px(1.), px(height)); count]
}

#[cfg(test)]
mod tests {
    use super::chunk_indices;

    #[test]
    fn grid_rows_keep_order_and_include_the_partial_last_row() {
        assert_eq!(chunk_indices(&[0, 1, 2, 3, 4], 2), [vec![0, 1], vec![2, 3], vec![4]]);
        assert!(chunk_indices(&[], 3).is_empty());
        assert_eq!(chunk_indices(&[4], 0), [vec![4]]);
    }
}
