pub(crate) fn activity_section_for_record(record: &serde_json::Value) -> Option<&'static str> {
    activity_section_for_record_with_stellar_fallback(record, "season")
}

pub(crate) fn activity_section_for_record_with_stellar_fallback(
    record: &serde_json::Value,
    current_stellar_section: &str,
) -> Option<&'static str> {
    fn stellar_section(target: Option<&str>) -> Option<&'static str> {
        match target {
            Some("travel") => Some("season"),
            Some("constellation") => Some("constellation"),
            Some("shop") => Some("shop"),
            Some("solar") => Some("solarTerms"),
            _ => None,
        }
    }

    fn fallback_section(section: &str) -> &'static str {
        match section {
            "constellation" => "constellation",
            "shop" => "shop",
            "solarTerms" => "solarTerms",
            _ => "season",
        }
    }

    let detail_target = record.get("detailTarget").and_then(serde_json::Value::as_str);
    if let Some(gameplay) =
        record.get("gameplayKey").and_then(serde_json::Value::as_str).filter(|key| !key.is_empty())
    {
        return match gameplay {
            "stellar" => Some(
                stellar_section(detail_target)
                    .unwrap_or_else(|| fallback_section(current_stellar_section)),
            ),
            "qixi" => Some("qixi"),
            "greenPlum" => Some("qingmei"),
            "weather" => Some("weather"),
            "charity" => Some("charity"),
            "pet" => Some("petDiary"),
            "autumnWish" | "autumnHappy" => Some("autumn"),
            _ => None,
        };
    }

    let detail_target = detail_target.filter(|target| !target.is_empty())?;
    match detail_target {
        "qixi" => Some("qixi"),
        "greenPlum" => Some("qingmei"),
        "weather" => Some("weather"),
        "charity" => Some("charity"),
        "pet" => Some("petDiary"),
        "wish" | "happy" => Some("autumn"),
        target => Some(
            stellar_section(Some(target))
                .unwrap_or_else(|| fallback_section(current_stellar_section)),
        ),
    }
}

pub(crate) fn activity_autumn_mode_for_record(record: &serde_json::Value) -> Option<&'static str> {
    if let Some(gameplay) =
        record.get("gameplayKey").and_then(serde_json::Value::as_str).filter(|key| !key.is_empty())
    {
        return match gameplay {
            "autumnWish" => Some("wish"),
            "autumnHappy" => Some("happy"),
            _ => None,
        };
    }
    match record.get("detailTarget").and_then(serde_json::Value::as_str) {
        Some("wish") => Some("wish"),
        Some("happy") => Some("happy"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        activity_autumn_mode_for_record, activity_section_for_record,
        activity_section_for_record_with_stellar_fallback,
    };

    #[test]
    fn autumn_directory_entry_opens_the_exact_gameplay_even_if_target_conflicts() {
        assert_eq!(
            activity_autumn_mode_for_record(&serde_json::json!({
                "gameplayKey": "autumnHappy",
                "detailTarget": "wish"
            })),
            Some("happy")
        );
        assert_eq!(
            activity_autumn_mode_for_record(&serde_json::json!({"detailTarget": "happy"})),
            Some("happy")
        );
        assert_eq!(
            activity_autumn_mode_for_record(&serde_json::json!({"gameplayKey": "autumnWish"})),
            Some("wish")
        );
        assert_eq!(
            activity_autumn_mode_for_record(&serde_json::json!({
                "gameplayKey": "qixi",
                "detailTarget": "happy"
            })),
            None
        );
        assert_eq!(
            activity_autumn_mode_for_record(&serde_json::json!({"detailTarget": "autumnHappy"})),
            None
        );
        assert_eq!(
            activity_autumn_mode_for_record(&serde_json::json!({"gameplayKey": "qixi"})),
            None
        );
    }

    #[test]
    fn gameplay_key_wins_over_a_conflicting_detail_target_and_unknown_key_stays_disabled() {
        assert_eq!(
            activity_section_for_record(&serde_json::json!({
                "gameplayKey": "qixi",
                "detailTarget": "shop"
            })),
            Some("qixi")
        );
        assert_eq!(
            activity_section_for_record(&serde_json::json!({
                "gameplayKey": "future-mode",
                "detailTarget": "qixi"
            })),
            None
        );
        assert_eq!(
            activity_section_for_record(&serde_json::json!({"id": "qixi", "name": "七夕鹊桥"})),
            None
        );
    }

    #[test]
    fn unknown_detail_target_without_gameplay_key_falls_back_to_stellar_like_vue() {
        assert_eq!(
            activity_section_for_record(&serde_json::json!({"detailTarget": "future-tab"})),
            Some("season")
        );
        assert_eq!(
            activity_section_for_record(&serde_json::json!({"detailTarget": "qixiBridge"})),
            Some("season")
        );
        assert_eq!(
            activity_section_for_record(&serde_json::json!({
                "gameplayKey": "future-mode",
                "detailTarget": "future-tab"
            })),
            None
        );
    }

    #[test]
    fn directory_routing_uses_vue_gameplay_precedence_and_remembers_stellar_tab() {
        let current_tab = "solarTerms";
        for record in [
            serde_json::json!({"gameplayKey": "stellar"}),
            serde_json::json!({"gameplayKey": "stellar", "detailTarget": "future-tab"}),
            serde_json::json!({"detailTarget": "future-tab"}),
            serde_json::json!({"detailTarget": "qixiBridge"}),
            serde_json::json!({"detailTarget": "autumnHappy"}),
        ] {
            assert_eq!(
                activity_section_for_record_with_stellar_fallback(&record, current_tab),
                Some("solarTerms"),
                "record {record}"
            );
        }
        assert_eq!(
            activity_section_for_record_with_stellar_fallback(
                &serde_json::json!({"gameplayKey": "stellar", "detailTarget": "shop"}),
                current_tab
            ),
            Some("shop")
        );
        assert_eq!(
            activity_section_for_record_with_stellar_fallback(
                &serde_json::json!({"gameplayKey": "stellar", "detailTarget": "qixi"}),
                current_tab
            ),
            Some("solarTerms")
        );
        assert_eq!(
            activity_section_for_record_with_stellar_fallback(
                &serde_json::json!({"gameplayKey": "qixi", "detailTarget": "shop"}),
                current_tab
            ),
            Some("qixi")
        );
        assert_eq!(
            activity_section_for_record_with_stellar_fallback(
                &serde_json::json!({"gameplayKey": "future-mode", "detailTarget": "qixi"}),
                current_tab
            ),
            None
        );
    }
}
