use nsh_core::prelude::*;
use serde_json::Value;

use crate::i18n::text_key;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OpDetail {
    Count(i64),
    Fertilizer { normal: i64, organic: i64 },
    Land(i64),
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Outcome {
    pub label_key: &'static str,
    pub detail: OpDetail,
}

pub(crate) fn op_detail(value: &Value) -> OpDetail {
    if value.get("op").and_then(Value::as_str) == Some("fertilize") {
        return OpDetail::Fertilizer {
            normal: value.get("normal").and_then(Value::as_i64).unwrap_or(0),
            organic: value.get("organic").and_then(Value::as_i64).unwrap_or(0),
        };
    }
    if let Some(count) = value.get("count").and_then(Value::as_i64) {
        return OpDetail::Count(count);
    }
    if let Some(land_id) = value.get("land_id").and_then(Value::as_i64) {
        return OpDetail::Land(land_id);
    }
    OpDetail::Done
}

pub(crate) fn outcome_line(outcome: &Outcome) -> SharedString {
    let label = text_key(outcome.label_key);
    let suffix = match &outcome.detail {
        OpDetail::Count(count) => count.to_string(),
        OpDetail::Land(id) => format!("#{id}"),
        OpDetail::Done => String::new(),
        OpDetail::Fertilizer { normal, organic } => format!(
            "{} {normal} {} {organic}",
            text_key("farm.normalFertilizer"),
            text_key("farm.organicFertilizer"),
        ),
    };
    if suffix.is_empty() {
        label
    } else {
        SharedString::from(format!("{label} {suffix}"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{op_detail, OpDetail};

    #[test]
    fn fertilize_reads_both_counts() {
        assert_eq!(
            op_detail(&json!({ "op": "fertilize", "normal": 2, "organic": 1 })),
            OpDetail::Fertilizer { normal: 2, organic: 1 }
        );
    }

    #[test]
    fn count_and_land_are_separate() {
        assert_eq!(op_detail(&json!({ "op": "harvest", "count": 4 })), OpDetail::Count(4));
        assert_eq!(op_detail(&json!({ "op": "upgrade", "land_id": 12 })), OpDetail::Land(12));
        assert_eq!(op_detail(&json!({ "ok": true, "op": "cycle" })), OpDetail::Done);
    }
}
