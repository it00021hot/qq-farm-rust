use std::sync::Arc;

use futures::{channel::mpsc, SinkExt};
use qq_farm_app::session::AppContext;
use qq_farm_core::runtime::runtime_state::RuntimeEvent;
use tokio::sync::broadcast;

use crate::desktop::DesktopEvent;

async fn receive_desktop_event(
    events: &mut broadcast::Receiver<RuntimeEvent>,
) -> Option<DesktopEvent> {
    match events.recv().await {
        Ok(event) => {
            Some(DesktopEvent::Realtime(qq_farm_app::events::AppEvent::from(event).to_realtime()))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => Some(DesktopEvent::Refresh),
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

pub(in crate::desktop) fn subscribe_realtime(
    app: &Arc<AppContext>,
    runtime: &Arc<tokio::runtime::Runtime>,
    test_mode: bool,
) -> (mpsc::Receiver<DesktopEvent>, Option<tokio::task::AbortHandle>) {
    let (mut sender, receiver) = mpsc::channel(128);
    let subscription = if !test_mode {
        let mut events = app.subscribe_events();
        Some(
            runtime
                .spawn(async move {
                    loop {
                        let Some(message) = receive_desktop_event(&mut events).await else {
                            break;
                        };
                        if sender.send(message).await.is_err() {
                            break;
                        }
                    }
                })
                .abort_handle(),
        )
    } else {
        None
    };
    (receiver, subscription)
}

#[cfg(test)]
mod tests {
    use super::receive_desktop_event;
    use crate::desktop::DesktopEvent;
    use qq_farm_core::runtime::runtime_state::RuntimeEvent;
    use serde_json::json;
    use tokio::sync::broadcast;

    fn status_event(account_id: &str) -> RuntimeEvent {
        RuntimeEvent::Status {
            account_id: account_id.into(),
            account_name: account_id.into(),
            status: json!({"status": {"runStatus": 1}}),
        }
    }

    #[tokio::test]
    async fn a_lagged_runtime_subscription_requests_a_fresh_snapshot() {
        let (sender, mut receiver) = broadcast::channel(1);
        sender.send(status_event("first")).expect("first runtime event");
        sender.send(status_event("latest")).expect("latest runtime event");

        assert!(matches!(receive_desktop_event(&mut receiver).await, Some(DesktopEvent::Refresh)));
        assert!(matches!(
            receive_desktop_event(&mut receiver).await,
            Some(DesktopEvent::Realtime(events)) if events.iter().any(|event| event.account_id.as_deref() == Some("latest"))
        ));
    }

    #[tokio::test]
    async fn a_closed_runtime_subscription_stops_the_forwarder() {
        let (sender, mut receiver) = broadcast::channel(1);
        drop(sender);

        assert!(receive_desktop_event(&mut receiver).await.is_none());
    }
}
