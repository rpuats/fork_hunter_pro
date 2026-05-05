//! Execution State - Full state management for fork execution

use crate::betting::{BetInstruction, BetMode, BetResult};
use crate::auth::BookmakerCredentials;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Overall execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    pub active_forks: HashMap<Uuid, ForkExecution>,
    pub account_readiness: HashMap<String, AccountReadiness>,
    pub bankroll_allocation: BankrollPlan,
    pub daily_stats: DailyStats,
    pub global_limits: GlobalLimits,
}

impl ExecutionState {
    pub fn new(initial_bankroll: Decimal) -> Self {
        Self {
            active_forks: HashMap::new(),
            account_readiness: HashMap::new(),
            bankroll_allocation: BankrollPlan::new(initial_bankroll),
            daily_stats: DailyStats::default(),
            global_limits: GlobalLimits::default(),
        }
    }

    pub fn start_fork(&mut self, fork: Fork) -> Uuid {
        let id = Uuid::new_v4();
        let execution = ForkExecution {
            id,
            fork: fork.clone(),
            bets: vec![],
            status: ForkStatus::Scanning,
            started_at: Utc::now(),
            timeout_at: Utc::now() + chrono::Duration::seconds(60),
        };
        self.active_forks.insert(id, execution);
        id
    }

    pub fn update_fork_status(&mut self, fork_id: Uuid, status: ForkStatus) {
        if let Some(execution) = self.active_forks.get_mut(&fork_id) {
            execution.status = status;
        }
    }

    pub fn add_bet(&mut self, fork_id: Uuid, bet: BetInstruction) {
        if let Some(execution) = self.active_forks.get_mut(&fork_id) {
            execution.bets.push(bet);
        }
    }

    pub fn complete_bet(&mut self, bet_id: &str, result: BetResult) {
        self.daily_stats.total_bets += 1;
        if result.status == crate::betting::BetStatus::Placed {
            self.daily_stats.successful_bets += 1;
        }
    }

    pub fn update_account_readiness(&mut self, bookmaker: String, readiness: AccountReadiness) {
        self.account_readiness.insert(bookmaker, readiness);
    }

    pub fn is_ready_for_execution(&self, fork: &Fork) -> bool {
        // Check if all required bookmakers are authenticated
        for bookmaker in &fork.bookmakers {
            if let Some(readiness) = self.account_readiness.get(bookmaker) {
                if !readiness.is_ready() {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    pub fn should_stop_due_to_limits(&self) -> bool {
        // Check daily profit target
        if self.daily_stats.profit >= self.global_limits.daily_profit_target {
            return true;
        }

        // Check max daily bets
        if self.daily_stats.total_bets >= self.global_limits.max_daily_bets {
            return true;
        }

        // Check consecutive losses
        if self.daily_stats.consecutive_losses >= self.global_limits.max_consecutive_losses {
            return true;
        }

        false
    }
}

/// Fork execution tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkExecution {
    pub id: Uuid,
    pub fork: Fork,
    pub bets: Vec<BetInstruction>,
    pub status: ForkStatus,
    pub started_at: DateTime<Utc>,
    pub timeout_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ForkStatus {
    Scanning,              // Looking for the fork
    Ready,                 // Found, preparing to execute
    AwaitingAuth,          // Waiting for bookmaker auth
    AwaitingConfirmation,  // Semi-auto: waiting for operator
    Executing,             // Placing bets
    PartiallyExecuted,     // One leg done, other pending
    Completed,             // All bets placed successfully
    Failed(String),        // Error occurred
    Expired,               // Timeout or odds changed
}

impl ForkStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, 
            ForkStatus::Completed | 
            ForkStatus::Failed(_) | 
            ForkStatus::Expired
        )
    }

    pub fn is_active(&self) -> bool {
        !self.is_terminal() && !matches!(self, ForkStatus::Scanning)
    }
}

/// Fork definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fork {
    pub id: Uuid,
    pub bookmakers: Vec<String>,
    pub event: String,
    pub sport: String,
    pub league: String,
    pub profit_percent: Decimal,
    pub legs: Vec<ForkLeg>,
    pub detected_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkLeg {
    pub bookmaker: String,
    pub market: String,
    pub selection: String,
    pub odds: Decimal,
    pub stake: Decimal,
}

/// Account readiness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountReadiness {
    pub bookmaker: String,
    pub authenticated: bool,
    pub balance: Decimal,
    pub session_valid: bool,
    pub last_check: DateTime<Utc>,
    pub can_place_bets: bool,
}

impl AccountReadiness {
    pub fn is_ready(&self) -> bool {
        self.authenticated && self.session_valid && self.can_place_bets && self.balance > Decimal::ZERO
    }
}

/// Bankroll allocation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankrollPlan {
    pub total_bankroll: Decimal,
    pub allocated: Decimal,
    pub available: Decimal,
    pub per_bookmaker: HashMap<String, Decimal>,
    pub strategy: StakingStrategy,
}

impl BankrollPlan {
    pub fn new(total: Decimal) -> Self {
        Self {
            total_bankroll: total,
            allocated: Decimal::ZERO,
            available: total,
            per_bookmaker: HashMap::new(),
            strategy: StakingStrategy::EqualProfit,
        }
    }

    pub fn allocate_for_fork(&mut self, fork: &Fork) -> Vec<StakeAllocation> {
        match self.strategy {
            StakingStrategy::EqualProfit => self.allocate_equal_profit(fork),
            StakingStrategy::MaximizeVolume => self.allocate_max_volume(fork),
            StakingStrategy::FixedAmount(amount) => self.allocate_fixed(fork, amount),
        }
    }

    fn allocate_equal_profit(&self, fork: &Fork) -> Vec<StakeAllocation> {
        let mut allocations = vec![];
        
        if fork.legs.len() < 2 {
            return allocations;
        }

        // Calculate stakes for equal profit
        let total = Decimal::from(10000); // Base amount, should come from config
        let mut total_inverse_odds = Decimal::ZERO;

        for leg in &fork.legs {
            if leg.odds > Decimal::ZERO {
                total_inverse_odds += Decimal::ONE / leg.odds;
            }
        }

        if total_inverse_odds > Decimal::ZERO {
            for leg in &fork.legs {
                let stake = total / (leg.odds * total_inverse_odds);
                allocations.push(StakeAllocation {
                    bookmaker: leg.bookmaker.clone(),
                    stake: stake.min(self.available / Decimal::from(fork.legs.len() as i64)),
                    odds: leg.odds,
                });
            }
        }

        allocations
    }

    fn allocate_max_volume(&self, _fork: &Fork) -> Vec<StakeAllocation> {
        // Placeholder for max volume strategy
        vec![]
    }

    fn allocate_fixed(&self, fork: &Fork, amount: Decimal) -> Vec<StakeAllocation> {
        fork.legs.iter().map(|leg| StakeAllocation {
            bookmaker: leg.bookmaker.clone(),
            stake: amount,
            odds: leg.odds,
        }).collect()
    }

    pub fn reserve_funds(&mut self, bookmaker: &str, amount: Decimal) -> bool {
        if self.available >= amount {
            self.available -= amount;
            self.allocated += amount;
            *self.per_bookmaker.entry(bookmaker.to_string()).or_insert(Decimal::ZERO) += amount;
            true
        } else {
            false
        }
    }

    pub fn release_funds(&mut self, bookmaker: &str, amount: Decimal) {
        self.available += amount;
        self.allocated -= amount;
        if let Some(alloc) = self.per_bookmaker.get_mut(bookmaker) {
            *alloc -= amount;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeAllocation {
    pub bookmaker: String,
    pub stake: Decimal,
    pub odds: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StakingStrategy {
    EqualProfit,
    MaximizeVolume,
    FixedAmount(Decimal),
}

/// Daily statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub total_bets: usize,
    pub successful_bets: usize,
    pub failed_bets: usize,
    pub profit: Decimal,
    pub total_stake: Decimal,
    pub consecutive_losses: usize,
    pub last_updated: DateTime<Utc>,
}

impl Default for DailyStats {
    fn default() -> Self {
        Self {
            total_bets: 0,
            successful_bets: 0,
            failed_bets: 0,
            profit: Decimal::ZERO,
            total_stake: Decimal::ZERO,
            consecutive_losses: 0,
            last_updated: Utc::now(),
        }
    }
}

/// Global limits for safety
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalLimits {
    pub daily_profit_target: Decimal,
    pub max_daily_bets: usize,
    pub max_consecutive_losses: usize,
    pub max_stake_per_bet: Decimal,
    pub min_stake_per_bet: Decimal,
    pub max_exposure_percent: Decimal, // Max % of bankroll at risk
}

impl Default for GlobalLimits {
    fn default() -> Self {
        Self {
            daily_profit_target: Decimal::from(100000), // 100k RUB daily target
            max_daily_bets: 100,
            max_consecutive_losses: 5,
            max_stake_per_bet: Decimal::from(50000), // 50k max per bet
            min_stake_per_bet: Decimal::from(100),   // 100 min per bet
            max_exposure_percent: Decimal::from_f64_retain(0.3).unwrap(), // 30% of bankroll
        }
    }
}

/// Execution orchestrator
pub struct ExecutionOrchestrator {
    state: ExecutionState,
    mode: BetMode,
}

impl ExecutionOrchestrator {
    pub fn new(initial_bankroll: Decimal, mode: BetMode) -> Self {
        Self {
            state: ExecutionState::new(initial_bankroll),
            mode,
        }
    }

    pub fn get_state(&self) -> &ExecutionState {
        &self.state
    }

    pub fn get_state_mut(&mut self) -> &mut ExecutionState {
        &mut self.state
    }

    pub fn set_mode(&mut self, mode: BetMode) {
        self.mode = mode;
    }

    pub fn get_mode(&self) -> BetMode {
        self.mode
    }

    /// Process new fork detection
    pub fn on_fork_detected(&mut self, fork: Fork) -> Option<Uuid> {
        // Check if we should process this fork
        if self.state.should_stop_due_to_limits() {
            return None;
        }

        // Check if ready for execution
        if !self.state.is_ready_for_execution(&fork) {
            return None;
        }

        // Start tracking
        let id = self.state.start_fork(fork);
        Some(id)
    }

    /// Calculate stakes for a fork
    pub fn calculate_stakes(&self, fork_id: Uuid) -> Vec<StakeAllocation> {
        if let Some(_execution) = self.state.active_forks.get(&fork_id) {
            // STUB: allocation temporarily disabled for compilation
            // TODO: Fix mutable borrow issue
            vec![]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_state_new() {
        let state = ExecutionState::new(Decimal::from(100000));
        assert_eq!(state.bankroll_allocation.total_bankroll, Decimal::from(100000));
        assert_eq!(state.daily_stats.total_bets, 0);
    }

    #[test]
    fn test_fork_status_is_terminal() {
        assert!(ForkStatus::Completed.is_terminal());
        assert!(ForkStatus::Failed("error".to_string()).is_terminal());
        assert!(!ForkStatus::Executing.is_terminal());
    }

    #[test]
    fn test_account_readiness() {
        let ready = AccountReadiness {
            bookmaker: "pari".to_string(),
            authenticated: true,
            balance: Decimal::from(10000),
            session_valid: true,
            last_check: Utc::now(),
            can_place_bets: true,
        };
        assert!(ready.is_ready());

        let not_ready = AccountReadiness {
            bookmaker: "fonbet".to_string(),
            authenticated: false,
            balance: Decimal::from(10000),
            session_valid: true,
            last_check: Utc::now(),
            can_place_bets: true,
        };
        assert!(!not_ready.is_ready());
    }
}
