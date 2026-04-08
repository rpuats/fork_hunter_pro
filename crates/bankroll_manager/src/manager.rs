use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use shared::{BankrollConfig, BankrollState, BookmakerBalance};
use std::sync::Arc;

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
        if let Some(bk) = state.bookmakers.iter_mut().find(|b| b.bookmaker == bookmaker) {
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
        state.updated_at = Utc::now();
    }

    pub fn get_state(&self) -> BankrollState {
        let state = self.state.read();
        state.clone()
    }

    /// Рассчитывает оптимальный размер ставки.
    /// `edge` — преимущество (например, 0.05 для 5% edge), не вероятность!
    /// `odds` — десятичный коэффициент
    pub fn calculate_optimal_stake(
        &self,
        bookmaker: &str,
        edge: f64,
        odds: f64,
    ) -> f64 {
        let config = self.config.read();
        let state = self.state.read();

        let bk_balance = state.bookmakers.iter()
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