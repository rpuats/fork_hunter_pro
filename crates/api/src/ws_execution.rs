//! WebSocket Execution Handler - Real-time execution events

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde_json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::handlers::AppState;
use crate::ws_events::{EventFilter, ServerEvent, SubscriptionRequest};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

/// WebSocket handler for execution events
pub async fn ws_execution_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_execution_socket(socket, state))
}

async fn handle_execution_socket(socket: WebSocket, state: Arc<AppState>) {
    let conn_id = uuid::Uuid::new_v4().to_string();
    info!(connection = %conn_id, "Execution WebSocket client connected");

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to event broadcaster
    let mut event_rx = state.event_broadcaster.subscribe();
    
    // Default filter - all channels
    let mut filter = EventFilter::new(vec!["all".to_string()]);

    // Task: Forward events to client
    let mut send_task = tokio::spawn(async move {
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);

        loop {
            tokio::select! {
                // Send heartbeat
                _ = heartbeat.tick() => {
                    let heartbeat_event = ServerEvent::Heartbeat {
                        timestamp: chrono::Utc::now(),
                        clients_connected: 1,
                    };
                    let json = match serde_json::to_string(&heartbeat_event) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize heartbeat: {}", e);
                            continue;
                        }
                    };
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }

                // Forward events
                event_result = event_rx.recv() => {
                    match event_result {
                        Ok(event) => {
                            // Filter events based on subscription
                            if !filter.accepts(&event) {
                                continue;
                            }
                            
                            let json = match serde_json::to_string(&event) {
                                Ok(s) => s,
                                Err(e) => {
                                    error!("Failed to serialize event: {}", e);
                                    continue;
                                }
                            };
                            
                            if sender.send(Message::Text(json)).await.is_err() {
                                warn!(connection = %conn_id, "Client disconnected");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Event receiver error: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    });

    // Task: Handle client messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            match message {
                Message::Text(text) => {
                    // Try to parse as subscription request
                    if let Ok(sub_req) = serde_json::from_str::<SubscriptionRequest>(&text) {
                        filter = EventFilter::new(sub_req.channels);
                        info!(connection = %conn_id, channels = ?sub_req.channels, "Subscription updated");
                    }
                }
                Message::Close(_) => {
                    info!(connection = %conn_id, "Client closed connection");
                    break;
                }
                Message::Ping(data) => {
                    // Pong is handled automatically by axum
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        }
        _ = (&mut recv_task) => {
            send_task.abort();
        }
    }

    info!(connection = %conn_id, "Execution WebSocket client disconnected");
}
