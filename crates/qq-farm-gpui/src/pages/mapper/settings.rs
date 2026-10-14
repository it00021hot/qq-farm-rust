use crate::pages::DisplayRow;
use serde_json::Value;

/// Settings are rendered as grouped forms, matching the Vue page. Turning the
/// backing JSON into generic rows exposed duplicate, untranslated field paths
/// below the actual controls.
pub(super) fn rows(_raw: &Value) -> Vec<DisplayRow> {
    Vec::new()
}
