use itertools::Itertools;
/// Cascade selection for 6-7 leg parlays with multi-BK optimization
///
/// Implements intelligent selection of 6-7 leg combinations while optimizing
/// bookmaker selection and managing risk across multiple bookmakers.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Strategy for selecting legs in 6-7 leg parlays
#[derive(Debug, Clone, Copy)]
pub enum CascadeStrategy {
    /// Greedy: select highest odds at each step
    GreedyOdds,
    /// Balance: maintain consistent odds and spread across BKs
    BalancedSpread,
    /// Correlation: avoid correlated events (e.g., same league multiple times)
    DecorrelatedEvents,
    /// Availability: prioritize events available in multiple BKs
    MultiBookmakerAvailability,
    /// Smart: Combination of above strategies
    SmartOptimal,
}

/// Leg selection with multi-BK context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CascadeLeg {
    pub position: usize,
    pub event_id: String,
    pub odds: f64,
    pub selection: String,
    pub primary_bk: String,
    pub backup_bks: Vec<String>,
    pub availability_score: f64, // 0-1, how many BKs have it
    pub league: String,
    pub event_time: Option<u64>, // Unix timestamp
}

/// Result of cascade leg selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeResult {
    pub strategy: String,
    pub leg_count: usize,
    pub selected_legs: Vec<CascadeLeg>,
    pub total_odds: f64,
    pub multi_bk_score: f64,    // How well distributed across BKs
    pub correlation_score: f64, // Measure of event correlation (lower is better)
    pub diversity_score: f64,   // 0-1, higher = more diverse
    pub optimal_allocation: HashMap<String, Vec<String>>, // BK -> list of events
}

/// Cascade selector for multi-leg parlays
pub struct CascadeSelector {
    strategy: CascadeStrategy,
    prefer_live: bool,
    min_odds: f64,
    max_odds: f64,
}

impl CascadeSelector {
    pub fn new(strategy: CascadeStrategy) -> Self {
        Self {
            strategy,
            prefer_live: true,
            min_odds: 1.3,
            max_odds: 5.0,
        }
    }

    pub fn with_odds_range(mut self, min: f64, max: f64) -> Self {
        self.min_odds = min;
        self.max_odds = max;
        self
    }

    pub fn prefer_live(mut self, prefer: bool) -> Self {
        self.prefer_live = prefer;
        self
    }

    /// Select optimal 6-7 leg parlay from available events
    pub fn select_cascade(
        &self,
        available_legs: Vec<CascadeLeg>,
        target_leg_count: usize,
    ) -> Option<CascadeResult> {
        if available_legs.len() < target_leg_count {
            return None;
        }

        // Filter legs by odds range
        let filtered_legs: Vec<CascadeLeg> = available_legs
            .into_iter()
            .filter(|l| l.odds >= self.min_odds && l.odds <= self.max_odds)
            .collect();

        if filtered_legs.len() < target_leg_count {
            return None;
        }

        let selected = match self.strategy {
            CascadeStrategy::GreedyOdds => {
                self.select_greedy_odds(&filtered_legs, target_leg_count)
            }
            CascadeStrategy::BalancedSpread => {
                self.select_balanced_spread(&filtered_legs, target_leg_count)
            }
            CascadeStrategy::DecorrelatedEvents => {
                self.select_decorrelated(&filtered_legs, target_leg_count)
            }
            CascadeStrategy::MultiBookmakerAvailability => {
                self.select_multi_bk(&filtered_legs, target_leg_count)
            }
            CascadeStrategy::SmartOptimal => {
                self.select_smart_optimal(&filtered_legs, target_leg_count)
            }
        };

        if selected.is_empty() {
            return None;
        }

        let total_odds: f64 = selected.iter().map(|l| l.odds).product();
        let multi_bk_score = self.calculate_multi_bk_score(&selected);
        let correlation_score = self.calculate_correlation_score(&selected);
        let diversity_score = self.calculate_diversity_score(&selected);
        let optimal_allocation = self.calculate_optimal_allocation(&selected);

        Some(CascadeResult {
            strategy: format!("{:?}", self.strategy),
            leg_count: selected.len(),
            selected_legs: selected,
            total_odds,
            multi_bk_score,
            correlation_score,
            diversity_score,
            optimal_allocation,
        })
    }

    /// Greedy selection: pick highest odds first
    fn select_greedy_odds(&self, legs: &[CascadeLeg], count: usize) -> Vec<CascadeLeg> {
        let mut sorted = legs.to_vec();
        sorted.sort_by(|a, b| {
            b.odds
                .partial_cmp(&a.odds)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(count).collect()
    }

    /// Balanced: maintain consistent odds spread
    fn select_balanced_spread(&self, legs: &[CascadeLeg], count: usize) -> Vec<CascadeLeg> {
        if legs.len() <= count {
            return legs.to_vec();
        }

        // Sort by odds and take every n-th element to balance
        let mut sorted = legs.to_vec();
        sorted.sort_by(|a, b| {
            a.odds
                .partial_cmp(&b.odds)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let step = legs.len() / count;
        sorted
            .into_iter()
            .step_by(step.max(1))
            .take(count)
            .collect()
    }

    /// Decorrelated: avoid same league multiple times
    fn select_decorrelated(&self, legs: &[CascadeLeg], count: usize) -> Vec<CascadeLeg> {
        let mut selected = Vec::new();
        let mut league_counts: HashMap<String, usize> = HashMap::new();

        // Sort by odds descending
        let mut sorted = legs.to_vec();
        sorted.sort_by(|a, b| {
            b.odds
                .partial_cmp(&a.odds)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for leg in sorted {
            if selected.len() >= count {
                break;
            }

            let league_count = league_counts.entry(leg.league.clone()).or_insert(0);

            // Prefer legs from different leagues; max 2 from same league
            if *league_count < 2 || selected.len() < count / 2 {
                selected.push(leg.clone());
                *league_count += 1;
            }
        }

        selected
    }

    /// Multi-BK: prioritize events available in many BKs
    fn select_multi_bk(&self, legs: &[CascadeLeg], count: usize) -> Vec<CascadeLeg> {
        let mut sorted = legs.to_vec();
        // Sort by availability (descending), then odds (descending)
        sorted.sort_by(|a, b| {
            match b
                .availability_score
                .partial_cmp(&a.availability_score)
                .unwrap_or(std::cmp::Ordering::Equal)
            {
                std::cmp::Ordering::Equal => b
                    .odds
                    .partial_cmp(&a.odds)
                    .unwrap_or(std::cmp::Ordering::Equal),
                other => other,
            }
        });

        sorted.into_iter().take(count).collect()
    }

    /// Smart: combination of all strategies
    fn select_smart_optimal(&self, legs: &[CascadeLeg], count: usize) -> Vec<CascadeLeg> {
        // Score each leg: 50% odds, 25% availability, 25% decorrelation
        let mut scored_legs: Vec<(CascadeLeg, f64)> = legs
            .iter()
            .map(|leg| {
                let odds_score = (leg.odds - self.min_odds) / (self.max_odds - self.min_odds);
                let avail_score = leg.availability_score;
                let score = (odds_score * 0.5) + (avail_score * 0.5);
                (leg.clone(), score)
            })
            .collect();

        scored_legs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply light decorrelation: penalize 3rd+ occurrence of same league
        let mut selected = Vec::new();
        let mut league_counts: HashMap<String, usize> = HashMap::new();

        for (leg, _score) in &scored_legs {
            if selected.len() >= count {
                break;
            }

            let count_in_league = league_counts.entry(leg.league.clone()).or_insert(0);

            if *count_in_league < 2 {
                selected.push(leg.clone());
                *count_in_league += 1;
            }
        }

        // Fill remaining if needed
        while selected.len() < count && selected.len() < legs.len() {
            if let Some((leg, _)) = scored_legs.get(selected.len()) {
                if !selected.contains(leg) {
                    selected.push(leg.clone());
                }
            } else {
                break;
            }
        }

        selected
    }

    /// Calculate how well legs are distributed across multiple BKs
    fn calculate_multi_bk_score(&self, legs: &[CascadeLeg]) -> f64 {
        let mut bk_usage: HashMap<String, usize> = HashMap::new();

        for leg in legs {
            *bk_usage.entry(leg.primary_bk.clone()).or_insert(0) += 1;
        }

        // Perfect score: even distribution across different BKs
        let total_legs = legs.len();
        let num_bks = bk_usage.len();

        if num_bks == 0 {
            return 0.0;
        }

        let ideal_per_bk = total_legs as f64 / num_bks as f64;
        let variance: f64 = bk_usage
            .values()
            .map(|&count| {
                let diff = count as f64 - ideal_per_bk;
                diff * diff
            })
            .sum::<f64>()
            / num_bks as f64;

        // Convert variance to score (0-1, higher is better)
        1.0 / (1.0 + variance.sqrt())
    }

    /// Calculate correlation between selected events
    /// Lower is better (less correlated = better diversification)
    fn calculate_correlation_score(&self, legs: &[CascadeLeg]) -> f64 {
        let mut league_counts: HashMap<String, usize> = HashMap::new();

        for leg in legs {
            *league_counts.entry(leg.league.clone()).or_insert(0) += 1;
        }

        // Correlation = variance in league distribution
        let total = legs.len() as f64;
        let expected = total / league_counts.len().max(1) as f64;

        let correlation: f64 = league_counts
            .values()
            .map(|&count| {
                let diff = count as f64 - expected;
                diff * diff
            })
            .sum::<f64>()
            / league_counts.len().max(1) as f64;

        // Lower correlation is better
        1.0 / (1.0 + correlation)
    }

    /// Calculate diversity of selected legs
    fn calculate_diversity_score(&self, legs: &[CascadeLeg]) -> f64 {
        let leagues: HashSet<_> = legs.iter().map(|l| &l.league).collect();
        let bks: HashSet<_> = legs.iter().map(|l| &l.primary_bk).collect();

        let league_diversity = leagues.len() as f64 / legs.len().max(1) as f64;
        let bk_diversity = bks.len() as f64 / legs.len().max(1) as f64;

        (league_diversity * 0.6) + (bk_diversity * 0.4)
    }

    /// Calculate optimal BK allocation
    fn calculate_optimal_allocation(&self, legs: &[CascadeLeg]) -> HashMap<String, Vec<String>> {
        let mut allocation: HashMap<String, Vec<String>> = HashMap::new();

        for leg in legs {
            allocation
                .entry(leg.primary_bk.clone())
                .or_insert_with(Vec::new)
                .push(leg.event_id.clone());
        }

        allocation
    }

    /// Suggest optimal leg count based on available events
    pub fn suggest_leg_count(available_legs: usize) -> usize {
        match available_legs {
            0..=3 => 2,
            4..=6 => 3,
            7..=10 => 4,
            11..=15 => 5,
            16..=25 => 6,
            _ => 7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cascade_leg(id: &str, odds: f64, league: &str, avail: f64, bks: usize) -> CascadeLeg {
        let mut backup_bks = Vec::new();
        for i in 0..bks.saturating_sub(1) {
            backup_bks.push(format!("bk{}", i + 2));
        }

        CascadeLeg {
            position: 0,
            event_id: id.to_string(),
            odds,
            selection: "1".to_string(),
            primary_bk: "bk1".to_string(),
            backup_bks,
            availability_score: avail,
            league: league.to_string(),
            event_time: None,
        }
    }

    #[test]
    fn test_greedy_odds_selection() {
        let legs = vec![
            make_cascade_leg("e1", 3.0, "L1", 0.8, 3),
            make_cascade_leg("e2", 2.0, "L2", 0.7, 2),
            make_cascade_leg("e3", 4.0, "L3", 0.9, 4),
            make_cascade_leg("e4", 2.5, "L1", 0.6, 2),
            make_cascade_leg("e5", 3.5, "L2", 0.8, 3),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::GreedyOdds);
        let result = selector.select_cascade(legs, 3).unwrap();

        assert_eq!(result.leg_count, 3);
        assert_eq!(result.selected_legs[0].odds, 4.0);
    }

    #[test]
    fn test_decorrelated_selection() {
        let legs = vec![
            make_cascade_leg("e1", 2.0, "EPL", 0.8, 3),
            make_cascade_leg("e2", 1.9, "EPL", 0.7, 2),
            make_cascade_leg("e3", 2.1, "LaLiga", 0.9, 4),
            make_cascade_leg("e4", 2.2, "Serie A", 0.6, 2),
            make_cascade_leg("e5", 2.3, "Bundesliga", 0.8, 3),
            make_cascade_leg("e6", 2.4, "Ligue 1", 0.8, 3),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::DecorrelatedEvents);
        let result = selector.select_cascade(legs, 4).unwrap();

        // Count leagues
        let leagues: HashSet<_> = result.selected_legs.iter().map(|l| &l.league).collect();
        assert!(leagues.len() >= 3); // Should prefer diverse leagues
    }

    #[test]
    fn test_multi_bk_selection() {
        let legs = vec![
            make_cascade_leg("e1", 2.0, "L1", 0.95, 5),
            make_cascade_leg("e2", 1.9, "L2", 0.85, 3),
            make_cascade_leg("e3", 2.1, "L3", 0.99, 6),
            make_cascade_leg("e4", 2.2, "L1", 0.60, 1),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::MultiBookmakerAvailability);
        let result = selector.select_cascade(legs, 3).unwrap();

        assert_eq!(result.leg_count, 3);
        // Should prefer high availability legs
        let avg_avail: f64 = result
            .selected_legs
            .iter()
            .map(|l| l.availability_score)
            .sum::<f64>()
            / result.selected_legs.len() as f64;
        assert!(avg_avail > 0.7);
    }

    #[test]
    fn test_smart_optimal_selection() {
        let legs = vec![
            make_cascade_leg("e1", 3.5, "EPL", 0.9, 4),
            make_cascade_leg("e2", 2.0, "EPL", 0.8, 3),
            make_cascade_leg("e3", 3.0, "LaLiga", 0.95, 5),
            make_cascade_leg("e4", 2.8, "Serie A", 0.7, 2),
            make_cascade_leg("e5", 2.9, "Bundesliga", 0.85, 3),
            make_cascade_leg("e6", 3.2, "Ligue 1", 0.92, 4),
            make_cascade_leg("e7", 2.5, "Championship", 0.75, 2),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);
        let result = selector.select_cascade(legs, 5).unwrap();

        assert_eq!(result.leg_count, 5);
        assert!(result.diversity_score > 0.5);
    }

    #[test]
    fn test_cascade_result_total_odds() {
        let legs = vec![
            make_cascade_leg("e1", 2.0, "L1", 0.8, 3),
            make_cascade_leg("e2", 3.0, "L2", 0.7, 2),
            make_cascade_leg("e3", 1.5, "L3", 0.9, 4),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::GreedyOdds);
        let result = selector.select_cascade(legs, 3).unwrap();

        let expected_odds = 3.0 * 2.0 * 1.5;
        assert!((result.total_odds - expected_odds).abs() < 0.01);
    }

    #[test]
    fn test_multi_bk_score() {
        let legs = vec![
            make_cascade_leg("e1", 2.0, "L1", 0.8, 3),
            make_cascade_leg("e2", 2.0, "L2", 0.7, 2),
            make_cascade_leg("e3", 2.0, "L3", 0.9, 4),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);
        let result = selector.select_cascade(legs, 3).unwrap();

        assert!(result.multi_bk_score >= 0.0 && result.multi_bk_score <= 1.0);
    }

    #[test]
    fn test_diversity_score() {
        let legs = vec![
            make_cascade_leg("e1", 2.0, "EPL", 0.8, 3),
            make_cascade_leg("e2", 2.0, "LaLiga", 0.7, 2),
            make_cascade_leg("e3", 2.0, "Serie A", 0.9, 4),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);
        let result = selector.select_cascade(legs, 3).unwrap();

        assert!(result.diversity_score > 0.6);
    }

    #[test]
    fn test_insufficient_legs() {
        let legs = vec![make_cascade_leg("e1", 2.0, "L1", 0.8, 3)];

        let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);
        let result = selector.select_cascade(legs, 5);

        assert!(result.is_none());
    }

    #[test]
    fn test_suggest_leg_count() {
        assert_eq!(CascadeSelector::suggest_leg_count(2), 2);
        assert_eq!(CascadeSelector::suggest_leg_count(5), 3);
        assert_eq!(CascadeSelector::suggest_leg_count(12), 5);
        assert_eq!(CascadeSelector::suggest_leg_count(20), 6);
        assert_eq!(CascadeSelector::suggest_leg_count(30), 7);
    }

    #[test]
    fn test_odds_range_filtering() {
        let legs = vec![
            make_cascade_leg("e1", 1.1, "L1", 0.8, 3), // Too low
            make_cascade_leg("e2", 3.0, "L2", 0.7, 2),
            make_cascade_leg("e3", 6.0, "L3", 0.9, 4), // Too high
        ];

        let selector = CascadeSelector::new(CascadeStrategy::GreedyOdds).with_odds_range(1.3, 5.0);
        let result = selector.select_cascade(legs, 2);

        assert!(result.is_some());
        if let Some(r) = result {
            assert_eq!(r.leg_count, 1); // Only e2 fits range
        }
    }

    #[test]
    fn test_optimal_allocation() {
        let legs = vec![
            make_cascade_leg("e1", 2.0, "L1", 0.8, 3),
            make_cascade_leg("e2", 2.0, "L2", 0.7, 2),
            make_cascade_leg("e3", 2.0, "L3", 0.9, 4),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);
        let result = selector.select_cascade(legs, 3).unwrap();

        assert!(!result.optimal_allocation.is_empty());
    }

    #[test]
    fn test_correlation_score() {
        let legs = vec![
            make_cascade_leg("e1", 2.0, "EPL", 0.8, 3),
            make_cascade_leg("e2", 2.0, "EPL", 0.7, 2),
            make_cascade_leg("e3", 2.0, "EPL", 0.9, 4),
        ];

        let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);
        let result = selector.select_cascade(legs, 3).unwrap();

        // High correlation (all same league)
        assert!(result.correlation_score < 0.5);
    }

    #[test]
    fn test_seven_leg_parlay() {
        let legs = (1..=10)
            .map(|i| {
                let odds = 1.8 + (i as f64 * 0.1);
                let league = match i % 5 {
                    0 => "EPL",
                    1 => "LaLiga",
                    2 => "Serie A",
                    3 => "Bundesliga",
                    _ => "Ligue 1",
                };
                make_cascade_leg(&format!("e{}", i), odds, league, 0.8, 3)
            })
            .collect();

        let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);
        let result = selector.select_cascade(legs, 7).unwrap();

        assert_eq!(result.leg_count, 7);
        assert!(result.total_odds > 1.0);
    }
}
