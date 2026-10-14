use std::collections::HashSet;

use super::super::super::*;
use super::super::weather;

mod request;
use request::WeatherOperation;

fn operation_is_pending(
    operation: WeatherOperation,
    pending_actions: &HashSet<String>,
    scanning_gids: &HashSet<String>,
) -> bool {
    match operation {
        WeatherOperation::Bottle { action: "scan", gid: Some(gid) } => {
            scanning_gids.contains(&gid.to_string())
        }
        WeatherOperation::Bottle { action: "scan", gid: None } => false,
        _ => pending_actions.contains(&operation.key()),
    }
}

impl FarmDesktop {
    pub(in crate::desktop) fn run_weather_action(
        &mut self,
        action: &'static str,
        gid: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        if !self.weather_action_available(action, gid) {
            return;
        }
        if action == "scan" && !gid.is_some_and(|gid| gid > 0) {
            return;
        }
        let operation = WeatherOperation::Bottle { action, gid };
        if operation_is_pending(
            operation,
            &self.activities.weather.pending_actions,
            &self.activities.weather.scanning_gids,
        ) {
            return;
        }
        self.run_weather_request(operation, cx);
    }

    pub(in crate::desktop) fn advance_weather_research(
        &mut self,
        node_id: i64,
        cx: &mut Context<Self>,
    ) {
        let operation = WeatherOperation::Research(node_id);
        if self.loading
            || node_id <= 0
            || operation_is_pending(
                operation,
                &self.activities.weather.pending_actions,
                &self.activities.weather.scanning_gids,
            )
        {
            return;
        }
        let available = self
            .page_raw
            .pointer("/weather/snapshot/research/nodes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .any(|node| {
                weather::number(node.get("id")) == Some(node_id)
                    && weather::flag(node, "availableByStatus")
                    && !weather::flag(node, "completed")
                    && weather::flag(node, "affordable")
            });
        if available {
            self.run_weather_request(operation, cx);
        }
    }

    fn run_weather_request(&mut self, operation: WeatherOperation, cx: &mut Context<Self>) {
        if self.page != Page::Activities || self.page_section != "weather" || self.quitting {
            return;
        }
        let Some(account_id) = self.selected_account.clone() else { return };
        let response_account = account_id.clone();
        let app = Arc::clone(&self.app);
        let runtime = Arc::clone(&self.tokio_runtime);
        let language = self.preferences.language.clone();
        let revision = self.request_revision;
        let generation = self.activities.weather.operation_generation;
        let action = operation.action();
        let scan_gid = if let WeatherOperation::Bottle { action: "scan", gid } = operation {
            gid.map(|gid| gid.to_string())
        } else {
            None
        };
        if let Some(gid) = &scan_gid {
            self.activities.weather.scanning_gids.insert(gid.clone());
        } else {
            self.activities.weather.pending_actions.insert(operation.key());
        }
        self.activities.weather.error = None;
        self.activities.weather.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    operation.execute(&app, &account_id).await.map_err(|error| error.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.request_revision != revision
                    || this.activities.weather.operation_generation != generation
                    || this.selected_account.as_deref() != Some(response_account.as_str())
                    || this.page != Page::Activities
                    || this.page_section != "weather"
                {
                    return;
                }
                if let Some(gid) = &scan_gid {
                    this.activities.weather.scanning_gids.remove(gid);
                } else {
                    this.activities.weather.pending_actions.remove(&operation.key());
                }
                match result {
                    Ok(Ok(value)) => this.apply_weather_result(action, &value),
                    Ok(Err(error)) => this.activities.weather.error = Some(error),
                    Err(error) => {
                        this.activities.weather.error =
                            Some(crate::i18n::background_task_error(&language, error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{operation_is_pending, WeatherOperation};

    #[test]
    fn pending_weather_actions_are_scoped_by_action_and_target() {
        let pending_actions = HashSet::from(["summon".into(), "research:2".into()]);
        let scanning_gids = HashSet::from(["12".into()]);

        assert!(operation_is_pending(
            WeatherOperation::Bottle { action: "summon", gid: None },
            &pending_actions,
            &scanning_gids,
        ));
        assert!(!operation_is_pending(
            WeatherOperation::Bottle { action: "exchange", gid: None },
            &pending_actions,
            &scanning_gids,
        ));
        assert!(operation_is_pending(
            WeatherOperation::Research(2),
            &pending_actions,
            &scanning_gids,
        ));
        assert!(!operation_is_pending(
            WeatherOperation::Research(3),
            &pending_actions,
            &scanning_gids,
        ));
        assert!(operation_is_pending(
            WeatherOperation::Bottle { action: "scan", gid: Some(12) },
            &pending_actions,
            &scanning_gids,
        ));
        assert!(!operation_is_pending(
            WeatherOperation::Bottle { action: "scan", gid: Some(34) },
            &pending_actions,
            &scanning_gids,
        ));
    }
}
