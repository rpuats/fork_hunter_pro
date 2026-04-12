use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::Extension;
use futures::{SinkExt, StreamExt};
use shared::EventBus;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

static CONNECTION_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, event_bus))
}

async fn handle_socket(socket: WebSocket, event_bus: Arc<EventBus>) {
    let conn_id = CONNECTION_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let subscriber_id = Uuid::new_v4().to_string();

    info!(connection = conn_id, subscriber = %subscriber_id, "WebSocket client connected");

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to EventBus
    let mut event_rx = event_bus.subscribe(&subscriber_id);

    // Task: Forward events from EventBus to WebSocket client
    let mut send_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);

        loop {
            tokio::select! {
                // Send heartbeat ping
                _ = interval.tick() => {
                    if sender.send(Message::Ping(Vec::new())).await.is_err() {
                        warn!(connection = conn_id, "Failed to send ping, closing connection");
                        break;
                    }
                }

                // Forward real events from bus
                event_result = event_rx.recv() => {
                    match event_result {
                        Ok(event) => {
                            match serde_json::to_string(&event) {
                                Ok(json) => {
                                    if sender.send(Message::Text(json)).await.is_err() {
                                        warn!(connection = conn_id, "Client disconnected while sending event");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!(error = %e, "Failed to serialize event");
                                    continue;
                                }
                            }
                        }
                        Err(_) => {
                            warn!(connection = conn_id, "EventBus channel closed");
                            break;
                        }
                    }
                }
            }
        }
    });

    // Task: Handle incoming messages from client
    let mut recv_task = tokio::spawn(async move {
        loop {
            match tokio::time::timeout(CLIENT_TIMEOUT, receiver.next()).await {
                Ok(Some(Ok(msg))) => {
                    match msg {
                        Message::Close(_) => {
                            info!(connection = conn_id, "Client requested close");
                            break;
                        }
                        Message::Pong(_) => {
                            // Heartbeat response received, everything ok
                            continue;
                        }
                        Message::Text(_) | Message::Binary(_) => {
                            // Ignore client messages for now - future: filters, subscriptions
                            continue;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    warn!(connection = conn_id, error = %e, "WebSocket error");
                    break;
                }
                Ok(None) => {
                    info!(connection = conn_id, "WebSocket stream closed");
                    break;
                }
                Err(_) => {
                    warn!(connection = conn_id, "Client timeout, disconnecting");
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // Cleanup
    event_bus.unsubscribe(&subscriber_id);

    info!(connection = conn_id, subscriber = %subscriber_id, "WebSocket client disconnected");
}
