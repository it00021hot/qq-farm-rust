use super::common::append_entity_rows;
use super::entity::entity_row;
use crate::pages::DisplayRow;
use serde_json::Value;
pub(super) fn mall_rows(raw: &Value) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    if let Some(o) = raw.as_object() {
        if o.get("slotType").and_then(Value::as_i64) == Some(-1) {
            let mut rows = mystery_shop_rows(raw);
            for row in &mut rows {
                row.section = "mystery".into();
            }
            return rows;
        }
        if let Some(catalog) = o.get("catalog") {
            let section = if o.get("slotType").and_then(Value::as_i64) == Some(4) {
                "svip"
            } else {
                "normal"
            };
            append_entity_rows(catalog, section, "商城商品", &mut rows);
            rows.insert(0,DisplayRow{id:"mall-balance".into(),section:section.into(),title:"钻石余额".into(),detail:format!("{}",o.get("diamond").and_then(Value::as_i64).unwrap_or(0)),state:String::new(),record:serde_json::json!({"diamond":o.get("diamond").cloned().unwrap_or(Value::Null)})});
        }
    }
    rows
}
pub(super) fn mystery_shop_rows(raw: &Value) -> Vec<DisplayRow> {
    let mut rows = Vec::new();
    if let Some(npc) = raw.get("npc") {
        if !npc.is_null() {
            rows.push(entity_row(npc, "npc", "神秘商人"));
        } else {
            rows.push(DisplayRow {
                id: "npc-unavailable".into(),
                section: "npc".into(),
                title: "当前没有活跃的神秘商人".into(),
                detail: "刷新商店状态后会显示可购买商品。".into(),
                state: "暂不可购买".into(),
                record: Value::Null,
            });
        }
    } else {
        append_entity_rows(raw, "shop", "神秘商店", &mut rows);
    }
    rows
}
