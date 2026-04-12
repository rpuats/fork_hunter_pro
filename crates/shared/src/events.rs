use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info};

const EVENT_BUFFER_SIZE: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BusEvent {
    RawOdds {
        bookmaker: String,
        payload: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
    NormalizedEvent {
        event_id: String,
        payload: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
    SurebetFound {
        surebet_id: String,
        payload: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
    ParserHealth {
        bookmaker: String,
        status: String,
        timestamp: DateTime<Utc>,
    },
    SystemAlert {
        level: String,
        message: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<BusEvent>,
    subscribers: Arc<RwLock<HashMap<String, broadcast::Receiver<BusEvent>>>>,
    event_count: Arc<RwLock<u64>>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_BUFFER_SIZE);
        Self {
            sender,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            event_count: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn publish(&self, event: BusEvent) -> anyhow::Result<()> {
        *self.event_count.write() += 1;
        debug!("Publishing event");
        self.sender
            .send(event)
            .map_err(|_| anyhow::anyhow!("Failed to publish event"))?;
        Ok(())
    }

    pub fn subscribe(&self, subscriber_id: &str) -> broadcast::Receiver<BusEvent> {
        let rx = self.sender.subscribe();
        self.subscribers
            .write()
            .insert(subscriber_id.to_string(), rx.resubscribe());
        info!(subscriber = subscriber_id, "New subscriber");
        rx
    }

    pub fn unsubscribe(&self, subscriber_id: &str) {
        self.subscribers.write().remove(subscriber_id);
        debug!(subscriber = subscriber_id, "Subscriber removed");
    }

    pub fn event_count(&self) -> u64 {
        *self.event_count.read()
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
