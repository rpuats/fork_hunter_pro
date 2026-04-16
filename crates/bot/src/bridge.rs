use std::sync::Arc;

use shared::{BusEvent, EventBus, Surebet};
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, info, warn};

use crate::telegram::TelegramBot;

pub fn spawn_event_bus_bridge(
    bot: Arc<TelegramBot>,
    event_bus: Arc<EventBus>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = event_bus.subscribe("telegram-bridge");
        info!("Telegram EventBus bridge started");

        loop {
            match rx.recv().await {
                Ok(event) => {
                    bot.metrics().record_bus_event();
                    bot.observe_event(&event);
                    handle_event(bot.as_ref(), event).await;
                }
                Err(RecvError::Lagged(skipped)) => {
                    bot.metrics().record_lag(skipped as u64);
                    warn!(skipped, "Telegram bridge lagged behind EventBus");
                }
                Err(RecvError::Closed) => {
                    info!("Telegram EventBus bridge stopped: channel closed");
                    break;
                }
            }
        }

        event_bus.unsubscribe("telegram-bridge");
    })
}

async fn handle_event(bot: &TelegramBot, event: BusEvent) {
    match event {
        BusEvent::SurebetFound { payload, .. } => {
            match serde_json::from_value::<Surebet>(payload) {
                Ok(surebet) => {
                    bot.record_seen_surebet(&surebet);
                    if bot.notify_surebet(&surebet).await {
                        bot.metrics().record_surebet(&surebet);
                    }
                }
                Err(error) => {
                    error!(error = %error, "Failed to decode SurebetFound payload for Telegram");
                }
            }
        }
        BusEvent::SystemAlert {
            level,
            message,
            timestamp,
        } => {
            if let Some(formatted) = bot.prepare_system_alert(&level, &message, timestamp) {
                if bot.notify_system(&formatted).await {
                    bot.metrics().record_system_alert(&level, &message);
                }
            }
        }
        _ => {}
    }
}
