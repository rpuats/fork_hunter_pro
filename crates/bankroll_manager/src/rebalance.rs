use shared::{BankrollState, BookmakerBalance};
use std::collections::HashMap;

pub struct RebalanceEngine;

impl RebalanceEngine {
    pub fn calculate_rebalance(
        state: &BankrollState,
        threshold_percent: f64,
    ) -> Vec<BookmakerBalance> {
        let total_available: f64 = state.bookmakers.iter().map(|b| b.available).sum();
        let n = state.bookmakers.len() as f64;
        if n == 0.0 {
            return Vec::new();
        }
        let target_per_bk = total_available / n;
        let threshold = target_per_bk * (threshold_percent / 100.0);

        state.bookmakers.iter().map(|bk| {
            let diff = bk.available - target_per_bk;
            let (recommended_deposit, recommended_withdraw) = if diff.abs() > threshold {
                if diff > 0.0 {
                    (0.0, diff)
                } else {
                    (diff.abs(), 0.0)
                }
            } else {
                (0.0, 0.0)
            };

            BookmakerBalance {
                bookmaker: bk.bookmaker.clone(),
                balance: bk.balance,
                exposure: bk.exposure,
                available: bk.available,
                recommended_deposit,
                recommended_withdraw,
            }
        }).collect()
    }

    pub fn optimal_distribution(
        total_budget: f64,
        bookmaker_stats: &HashMap<String, BookmakerStats>,
    ) -> HashMap<String, f64> {
        let total_weight: f64 = bookmaker_stats.values().map(|s| s.weight).sum();
        if total_weight == 0.0 {
            let n = bookmaker_stats.len() as f64;
            return bookmaker_stats.keys().map(|k| (k.clone(), total_budget / n)).collect();
        }

        bookmaker_stats.iter()
            .map(|(k, v)| (k.clone(), total_budget * (v.weight / total_weight)))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct BookmakerStats {
    pub weight: f64,
    pub fork_frequency: f64,
    pub avg_profit: f64,
    pub reliability: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimal_distribution_equal() {
        let mut stats = HashMap::new();
        stats.insert("bk1".into(), BookmakerStats { weight: 1.0, fork_frequency: 1.0, avg_profit: 1.0, reliability: 1.0 });
        stats.insert("bk2".into(), BookmakerStats { weight: 1.0, fork_frequency: 1.0, avg_profit: 1.0, reliability: 1.0 });
        let dist = RebalanceEngine::optimal_distribution(10000.0, &stats);
        assert!((dist["bk1"] - 5000.0).abs() < 0.01);
        assert!((dist["bk2"] - 5000.0).abs() < 0.01);
    }
}
