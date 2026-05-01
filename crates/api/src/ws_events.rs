//! WebSocket Events - Real-time event streaming for scanner and execution
//! 30+ event types covering all aspects of fork hunting

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

/// Main event types (30+)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
#[serde(rename_all = "snake_case")]
pub enum ServerEvent {
    // ===================== SCANNER EVENTS =====================
    /// Scanner started scanning
    ScannerStarted {
        timestamp: DateTime<Utc>,
    },
    
    /// Scanner stopped
    ScannerStopped {
        timestamp: DateTime<Utc>,
        reason: String,
    },
    
    /// Parser connected/started
    ParserConnected {
        bookmaker: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Parser disconnected
    ParserDisconnected {
        bookmaker: String,
        timestamp: DateTime<Utc>,
        reason: String,
    },
    
    /// Parser error
    ParserError {
        bookmaker: String,
        error: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Events update from parser
    EventsUpdated {
        bookmaker: String,
        count: usize,
        timestamp: DateTime<Utc>,
    },
    
    // ===================== FORK EVENTS =====================
    /// New fork detected
    ForkDetected {
        fork_id: Uuid,
        profit_percent: Decimal,
        bookmakers: Vec<String>,
        sport: String,
        league: String,
        event: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Fork updated (odds changed)
    ForkUpdated {
        fork_id: Uuid,
        old_profit: Decimal,
        new_profit: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// Fork expired (no longer available)
    ForkExpired {
        fork_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Fork hidden by filter
    ForkFiltered {
        fork_id: Uuid,
        filter_reason: String,
    },
    
    /// Odds changed for a leg
    OddsChanged {
        fork_id: Uuid,
        bookmaker: String,
        old_odds: Decimal,
        new_odds: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    // ===================== EXECUTION EVENTS =====================
    /// Execution started
    ExecutionStarted {
        mode: ExecutionMode,
        timestamp: DateTime<Utc>,
    },
    
    /// Execution stopped
    ExecutionStopped {
        timestamp: DateTime<Utc>,
    },
    
    /// Execution paused
    ExecutionPaused {
        timestamp: DateTime<Utc>,
    },
    
    /// Execution resumed
    ExecutionResumed {
        timestamp: DateTime<Utc>,
    },
    
    /// Mode changed
    ExecutionModeChanged {
        old_mode: ExecutionMode,
        new_mode: ExecutionMode,
        timestamp: DateTime<Utc>,
    },
    
    /// Bet prepared (coupon filled)
    BetPrepared {
        fork_id: Uuid,
        bet_id: String,
        bookmaker: String,
        event: String,
        market: String,
        selection: String,
        odds: Decimal,
        stake: Decimal,
        screenshot: Option<String>, // base64
        timestamp: DateTime<Utc>,
    },
    
    /// Bet awaiting confirmation (semi-auto)
    BetAwaitingConfirmation {
        bet_id: String,
        fork_id: Uuid,
        bookmaker: String,
        expires_at: DateTime<Utc>,
    },
    
    /// Bet confirmed by operator
    BetConfirmed {
        bet_id: String,
        fork_id: Uuid,
        bookmaker: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Bet rejected by operator
    BetRejected {
        bet_id: String,
        fork_id: Uuid,
        bookmaker: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Bet placed successfully
    BetPlaced {
        bet_id: String,
        fork_id: Uuid,
        bookmaker: String,
        external_bet_id: Option<String>,
        actual_odds: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// Bet placement failed
    BetFailed {
        bet_id: String,
        fork_id: Uuid,
        bookmaker: String,
        error: String,
        retryable: bool,
        timestamp: DateTime<Utc>,
    },
    
    /// Stake changed by operator
    StakeChanged {
        bet_id: String,
        old_stake: Decimal,
        new_stake: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    // ===================== AUTH EVENTS =====================
    /// Auth started for bookmaker
    AuthStarted {
        bookmaker: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Auth progress update
    AuthProgress {
        bookmaker: String,
        step: String, // "opening_browser", "filling_login", etc.
        timestamp: DateTime<Utc>,
    },
    
    /// Auth completed successfully
    AuthSuccess {
        bookmaker: String,
        balance: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// Auth failed
    AuthFailed {
        bookmaker: String,
        error: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Captcha required
    CaptchaRequired {
        bookmaker: String,
        image_base64: String,
        timestamp: DateTime<Utc>,
    },
    
    /// 2FA required
    TwoFARequired {
        bookmaker: String,
        method: String, // "sms", "totp", "email"
        timestamp: DateTime<Utc>,
    },
    
    /// Session restored from storage
    SessionRestored {
        bookmaker: String,
        timestamp: DateTime<Utc>,
    },
    
    // ===================== BANKROLL EVENTS =====================
    /// Bankroll updated
    BankrollUpdated {
        total: Decimal,
        allocated: Decimal,
        available: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// Balance updated for bookmaker
    BalanceUpdated {
        bookmaker: String,
        old_balance: Decimal,
        new_balance: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// Low balance warning
    BalanceLow {
        bookmaker: String,
        balance: Decimal,
        threshold: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    // ===================== SYSTEM EVENTS =====================
    /// System health update
    HealthUpdate {
        status: String, // "healthy", "degraded", "critical"
        parsers_online: usize,
        parsers_total: usize,
        timestamp: DateTime<Utc>,
    },
    
    /// Error occurred
    SystemError {
        component: String,
        error: String,
        severity: ErrorSeverity,
        timestamp: DateTime<Utc>,
    },
    
    /// Config reloaded
    ConfigReloaded {
        timestamp: DateTime<Utc>,
    },
    
    /// Heartbeat
    Heartbeat {
        timestamp: DateTime<Utc>,
        clients_connected: usize,
    },
    
    // ===================== NOTIFICATION EVENTS =====================
    /// Profit target reached
    ProfitTargetReached {
        daily_profit: Decimal,
        target: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// Daily limit reached
    DailyLimitReached {
        bets_count: usize,
        stake_total: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// Opportunity missed (timeout)
    OpportunityMissed {
        fork_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Auto,
    Semi,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Client subscription request
#[derive(Debug, Deserialize)]
pub struct SubscriptionRequest {
    pub channels: Vec<String>, // "forks", "execution", "auth", "system", "all"
}

/// Event broadcaster - central hub for all events
pub struct EventBroadcaster {
    sender: broadcast::Sender<ServerEvent>,
    subscribers: Arc<RwLock<Vec<SubscriberInfo>>>,
}

#[derive(Debug, Clone)]
struct SubscriberInfo {
    id: String,
    channels: Vec<String>,
}

impl EventBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        
        Self {
            sender,
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }
    
    /// Broadcast event to all subscribers
    pub fn broadcast(&self, event: ServerEvent) {
        let _ = self.sender.send(event);
    }
    
    /// Get subscriber count
    pub async fn subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Event filter for client subscriptions
pub struct EventFilter {
    channels: Vec<String>,
}

impl EventFilter {
    pub fn new(channels: Vec<String>) -> Self {
        Self { channels }
    }
    
    pub fn accepts(&self, event: &ServerEvent) -> bool {
        if self.channels.contains(&"all".to_string()) {
            return true;
        }
        
        let event_channel = match event {
            ServerEvent::ScannerStarted { .. } |
            ServerEvent::ScannerStopped { .. } |
            ServerEvent::ParserConnected { .. } |
            ServerEvent::ParserDisconnected { .. } |
            ServerEvent::ParserError { .. } |
            ServerEvent::EventsUpdated { .. } => "scanner",
            
            ServerEvent::ForkDetected { .. } |
            ServerEvent::ForkUpdated { .. } |
            ServerEvent::ForkExpired { .. } |
            ServerEvent::ForkFiltered { .. } |
            ServerEvent::OddsChanged { .. } => "forks",
            
            ServerEvent::ExecutionStarted { .. } |
            ServerEvent::ExecutionStopped { .. } |
            ServerEvent::ExecutionPaused { .. } |
            ServerEvent::ExecutionResumed { .. } |
            ServerEvent::ExecutionModeChanged { .. } |
            ServerEvent::BetPrepared { .. } |
            ServerEvent::BetAwaitingConfirmation { .. } |
            ServerEvent::BetConfirmed { .. } |
            ServerEvent::BetRejected { .. } |
            ServerEvent::BetPlaced { .. } |
            ServerEvent::BetFailed { .. } |
            ServerEvent::StakeChanged { .. } => "execution",
            
            ServerEvent::AuthStarted { .. } |
            ServerEvent::AuthProgress { .. } |
            ServerEvent::AuthSuccess { .. } |
            ServerEvent::AuthFailed { .. } |
            ServerEvent::CaptchaRequired { .. } |
            ServerEvent::TwoFARequired { .. } |
            ServerEvent::SessionRestored { .. } => "auth",
            
            ServerEvent::BankrollUpdated { .. } |
            ServerEvent::BalanceUpdated { .. } |
            ServerEvent::BalanceLow { .. } => "bankroll",
            
            ServerEvent::HealthUpdate { .. } |
            ServerEvent::SystemError { .. } |
            ServerEvent::ConfigReloaded { .. } |
            ServerEvent::Heartbeat { .. } => "system",
            
            ServerEvent::ProfitTargetReached { .. } |
            ServerEvent::DailyLimitReached { .. } |
            ServerEvent::OpportunityMissed { .. } => "notifications",
        };
        
        self.channels.contains(&event_channel.to_string())
    }
}

/// Helper to create common events
pub mod event_factory {
    use super::*;
    
    pub fn fork_detected(
        fork_id: Uuid,
        profit_percent: Decimal,
        bookmakers: Vec<String>,
        sport: String,
        league: String,
        event: String,
    ) -> ServerEvent {
        ServerEvent::ForkDetected {
            fork_id,
            profit_percent,
            bookmakers,
            sport,
            league,
            event,
            timestamp: Utc::now(),
        }
    }
    
    pub fn bet_awaiting_confirmation(
        bet_id: String,
        fork_id: Uuid,
        bookmaker: String,
        expires_in_secs: u64,
    ) -> ServerEvent {
        ServerEvent::BetAwaitingConfirmation {
            bet_id,
            fork_id,
            bookmaker,
            expires_at: Utc::now() + chrono::Duration::seconds(expires_in_secs as i64),
        }
    }
    
    pub fn auth_progress(bookmaker: String, step: String) -> ServerEvent {
        ServerEvent::AuthProgress {
            bookmaker,
            step,
            timestamp: Utc::now(),
        }
    }
    
    pub fn heartbeat(clients: usize) -> ServerEvent {
        ServerEvent::Heartbeat {
            timestamp: Utc::now(),
            clients_connected: clients,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_event_filter() {
        let filter = EventFilter::new(vec!["forks".to_string()]);
        
        let fork_event = ServerEvent::ForkDetected {
            fork_id: Uuid::new_v4(),
            profit_percent: Decimal::from_f64_retain(1.5).unwrap(),
            bookmakers: vec!["pari".to_string()],
            sport: "Football".to_string(),
            league: "Premier League".to_string(),
            event: "Team A vs Team B".to_string(),
            timestamp: Utc::now(),
        };
        
        let auth_event = ServerEvent::AuthSuccess {
            bookmaker: "pari".to_string(),
            balance: Decimal::from_f64_retain(1000.0).unwrap(),
            timestamp: Utc::now(),
        };
        
        assert!(filter.accepts(&fork_event));
        assert!(!filter.accepts(&auth_event));
    }
    
    #[test]
    fn test_event_filter_all() {
        let filter = EventFilter::new(vec!["all".to_string()]);
        
        let event = ServerEvent::Heartbeat {
            timestamp: Utc::now(),
            clients_connected: 5,
        };
        
        assert!(filter.accepts(&event));
    }
}
