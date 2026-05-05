use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use shared::{BankrollConfig, BankrollState, BookmakerBalance, BookmakerBalanceSnapshot};
use std::sync::Arc;

use super::allocation::DepositAllocator;
use super::kelly::KellyCalculator;
use super::rebalance::RebalanceEngine;

#[derive(Clone)]
pub struct BankrollManager {
    state: Arc<RwLock<BankrollState>>,
    config: Arc<RwLock<BankrollConfig>>,
    daily_stats: Arc<DashMap<String, DailyStats>>,
}

#[derive(Debug, Clone, Default)]
struct DailyStats {
    pub profit: f64,
    pub loss: f64,
    pub bets_count: u32,
}

impl BankrollManager {
    pub fn new(config: BankrollConfig) -> Self {
        let state = BankrollState {
            total_budget: config.total_budget,
            bookmakers: Vec::new(),
            total_exposure: 0.0,
            daily_profit: 0.0,
            daily_loss: 0.0,
            total_profit: 0.0,
            updated_at: Utc::now(),
        };

        Self {
            state: Arc::new(RwLock::new(state)),
            config: Arc::new(RwLock::new(config)),
            daily_stats: Arc::new(DashMap::new()),
        }
    }

    pub fn update_balance(&self, bookmaker: &str, balance: f64, exposure: f64) {
        let mut state = self.state.write();
        if let Some(bk) = state
            .bookmakers
            .iter_mut()
            .find(|b| b.bookmaker == bookmaker)
        {
            bk.balance = balance;
            bk.exposure = exposure;
            bk.available = (balance - exposure).max(0.0);
        } else {
            state.bookmakers.push(BookmakerBalance {
                bookmaker: bookmaker.to_string(),
                balance,
                exposure,
                available: (balance - exposure).max(0.0),
                recommended_deposit: 0.0,
                recommended_withdraw: 0.0,
            });
        }
        state.total_exposure = state.bookmakers.iter().map(|entry| entry.exposure).sum();
        state.updated_at = Utc::now();
    }

    pub fn apply_balance_snapshot(&self, snapshot: &BookmakerBalanceSnapshot) {
        self.update_balance(
            &snapshot.bookmaker,
            snapshot.total_balance,
            snapshot.exposure,
        );
    }

    pub fn get_state(&self) -> BankrollState {
        let state = self.state.read();
        state.clone()
    }

    /// Рассчитывает оптимальный размер ставки.
    /// `edge` — преимущество (например, 0.05 для 5% edge), не вероятность!
    /// `odds` — десятичный коэффициент
    pub fn calculate_optimal_stake(&self, bookmaker: &str, edge: f64, odds: f64) -> f64 {
        let config = self.config.read();
        let state = self.state.read();

        let bk_balance = state
            .bookmakers
            .iter()
            .find(|b| b.bookmaker == bookmaker)
            .map(|b| b.available)
            .unwrap_or(0.0);

        // Конвертируем edge в истинную вероятность
        // edge = prob - implied_prob => prob = implied_prob + edge
        let implied_prob = 1.0 / odds;
        let true_prob = implied_prob + edge;

        KellyCalculator::optimal_stake(
            bk_balance,
            true_prob,
            odds,
            config.kelly_fraction,
            config.max_exposure_percent,
        )
    }

    pub fn get_rebalance_recommendations(&self) -> Vec<BookmakerBalance> {
        let state = self.state.read();
        let config = self.config.read();
        RebalanceEngine::calculate_rebalance(&state, config.rebalance_threshold)
    }

    pub fn get_deposit_allocation_guidance(&self) -> shared::DepositAllocationGuidance {
        let state = self.state.read();
        DepositAllocator::build_guidance(&state.bookmakers, state.total_budget)
    }

    pub fn record_bet_result(&self, _bookmaker: &str, profit: f64) {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let mut stats = self.daily_stats.entry(today).or_default();

        if profit > 0.0 {
            stats.profit += profit;
        } else {
            stats.loss += profit.abs();
        }
        stats.bets_count += 1;

        let mut state = self.state.write();
        state.total_profit += profit;
        if profit > 0.0 {
            state.daily_profit += profit;
        } else {
            state.daily_loss += profit.abs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn apply_balance_snapshot_updates_bookmaker_and_total_exposure() {
        let manager = BankrollManager::new(BankrollConfig::default());

        manager.apply_balance_snapshot(&BookmakerBalanceSnapshot {
            account_id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 7_500.0,
            exposure: 2_500.0,
            bonus_balance: Some(0.0),
            source: Some("test".into()),
            captured_at: Utc::now(),
        });

        let state = manager.get_state();
        assert_eq!(state.bookmakers.len(), 1);
        assert_eq!(state.bookmakers[0].bookmaker, "pari");
        assert_eq!(state.bookmakers[0].balance, 10_000.0);
        assert_eq!(state.bookmakers[0].available, 7_500.0);
        assert_eq!(state.total_exposure, 2_500.0);
    }

    #[test]
    fn update_balance_recomputes_total_exposure_after_multiple_updates() {
        let manager = BankrollManager::new(BankrollConfig::default());

        manager.update_balance("pari", 10_000.0, 2_000.0);
        manager.update_balance("fonbet", 8_000.0, 500.0);
        manager.update_balance("pari", 10_000.0, 1_250.0);

        let state = manager.get_state();
        assert_eq!(state.total_exposure, 1_750.0);
    }
}
