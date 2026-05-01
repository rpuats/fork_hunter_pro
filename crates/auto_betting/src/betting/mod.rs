//! Betting module - Auto/Semi/Manual bet placement

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub mod auto_bet;
pub mod semi_auto_bet;
pub mod manual_bet;
pub mod operator_queue;

/// Bet execution mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BetMode {
    Auto,      // Fully automatic
    SemiAuto,  // Operator confirmation required
    Manual,    // Manual preparation only
}

/// Bet instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetInstruction {
    pub id: String,
    pub fork_id: Uuid,
    pub bookmaker_id: String,
    pub event_id: String,
    pub event_name: String,
    pub sport: String,
    pub league: String,
    pub market: String,      // "1X2", "total_over", "handicap_1"
    pub selection: String,   // "P1", "over_2.5", "H1_-1.5"
    pub odds: Decimal,
    pub stake: Decimal,
    pub mode: BetMode,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl BetInstruction {
    pub fn new(
        fork_id: Uuid,
        bookmaker_id: String,
        event_name: String,
        market: String,
        selection: String,
        odds: Decimal,
        stake: Decimal,
        mode: BetMode,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            fork_id,
            bookmaker_id: bookmaker_id.clone(),
            event_id: String::new(),
            event_name,
            sport: String::new(),
            league: String::new(),
            market,
            selection,
            odds,
            stake,
            mode,
            expires_at: Utc::now() + chrono::Duration::seconds(60),
            created_at: Utc::now(),
        }
    }
}

/// Bet result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetResult {
    pub bet_id: String,
    pub status: BetStatus,
    pub external_bet_id: Option<String>,
    pub actual_odds: Option<Decimal>,
    pub error: Option<String>,
    pub screenshot: Option<Vec<u8>>,
    pub placed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BetStatus {
    Pending,
    Preparing,       // Filling coupon
    AwaitingConfirmation, // Semi-auto: waiting for operator
    Placed,
    Rejected,
    Timeout,
    Error(String),
}

/// Betting errors
#[derive(Debug, thiserror::Error)]
pub enum BettingError {
    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: Decimal, required: Decimal },
    
    #[error("Odds changed: expected {expected}, actual {actual}")]
    OddsChanged { expected: Decimal, actual: Decimal },
    
    #[error("Stake limits exceeded: min {min}, max {max}, requested {requested}")]
    StakeLimitsExceeded { min: Decimal, max: Decimal, requested: Decimal },
    
    #[error("Market closed or suspended")]
    MarketClosed,
    
    #[error("Event not found: {0}")]
    EventNotFound(String),
    
    #[error("Browser error: {0}")]
    BrowserError(String),
    
    #[error("Operator cancelled")]
    OperatorCancelled,
    
    #[error("Timeout: {operation}")]
    Timeout { operation: String },
    
    #[error("Session expired for {bookmaker}")]
    SessionExpired { bookmaker: String },
}

/// Bet execution engine
pub struct BettingEngine {
    mode: BetMode,
    operator_queue: operator_queue::OperatorQueue,
    active_bets: std::collections::HashMap<String, BetInstruction>,
}

impl BettingEngine {
    pub fn new(mode: BetMode) -> Self {
        Self {
            mode,
            operator_queue: operator_queue::OperatorQueue::new(),
            active_bets: std::collections::HashMap::new(),
        }
    }

    pub fn set_mode(&mut self, mode: BetMode) {
        self.mode = mode;
    }

    pub fn get_mode(&self) -> BetMode {
        self.mode
    }

    pub fn submit_bet(&mut self, instruction: BetInstruction) {
        self.active_bets.insert(instruction.id.clone(), instruction);
    }

    pub fn get_pending_bets(&self) -> Vec<&BetInstruction> {
        self.active_bets.values().collect()
    }

    pub fn remove_bet(&mut self, bet_id: &str) -> Option<BetInstruction> {
        self.active_bets.remove(bet_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bet_instruction_creation() {
        let bet = BetInstruction::new(
            Uuid::new_v4(),
            "pari".to_string(),
            "Team A vs Team B".to_string(),
            "1X2".to_string(),
            "P1".to_string(),
            Decimal::from_f64_retain(1.85).unwrap(),
            Decimal::from_f64_retain(1000.0).unwrap(),
            BetMode::SemiAuto,
        );

        assert_eq!(bet.bookmaker_id, "pari");
        assert_eq!(bet.mode, BetMode::SemiAuto);
    }
}
