/// Smart leg reordering for optimal parlay execution
/// 
/// Orders parlay legs strategically to maximize early wins and minimize
/// exposure to low-odds legs at the end of the cascade.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Represents a parlay leg with scheduling info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledLeg {
    pub position: usize,
    pub event_id: String,
    pub odds: f64,
    pub market: String,
    pub selection: String,
    pub event_time_minutes: Option<u32>, // Minutes from now
    pub form_score: f64, // 0.0-1.0 confidence
    pub bookmaker: String,
}

/// Reordering strategy
#[derive(Debug, Clone, Copy)]
pub enum ReorderStrategy {
    /// Sort by odds descending (highest first) - safest early wins
    HighestOddsFirst,
    /// Sort by odds ascending (lowest first) - accumulate odds faster
    LowestOddsFirst,
    /// Balance: alternating high/low odds
    AlternatingHighLow,
    /// Sort by event time (earliest first) - quick execution
    EarliestFirst,
    /// Sort by confidence/form score (highest first)
    HighestFormFirst,
    /// Smart: High odds + early start + good form
    Smart,
}

/// Leg reordering result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderResult {
    pub strategy: String,
    pub original_order: Vec<String>,
    pub reordered: Vec<String>,
    pub cumulative_odds: Vec<f64>,
    pub efficiency_score: f64,
    pub early_win_probability: f64,
}

/// Reordering calculator
pub struct LegReorderer {
    strategy: ReorderStrategy,
}

impl LegReorderer {
    pub fn new(strategy: ReorderStrategy) -> Self {
        Self { strategy }
    }

    /// Reorder legs based on strategy
    pub fn reorder(&self, legs: Vec<ScheduledLeg>) -> ReorderResult {
        if legs.is_empty() {
            return ReorderResult {
                strategy: format!("{:?}", self.strategy),
                original_order: vec![],
                reordered: vec![],
                cumulative_odds: vec![],
                efficiency_score: 0.0,
                early_win_probability: 0.0,
            };
        }

        let original_order: Vec<String> = legs.iter().map(|l| l.event_id.clone()).collect();

        let mut sorted_legs = legs.clone();

        match self.strategy {
            ReorderStrategy::HighestOddsFirst => {
                sorted_legs.sort_by(|a, b| {
                    b.odds.partial_cmp(&a.odds).unwrap_or(Ordering::Equal)
                });
            }
            ReorderStrategy::LowestOddsFirst => {
                sorted_legs.sort_by(|a, b| {
                    a.odds.partial_cmp(&b.odds).unwrap_or(Ordering::Equal)
                });
            }
            ReorderStrategy::AlternatingHighLow => {
                sorted_legs.sort_by(|a, b| {
                    b.odds.partial_cmp(&a.odds).unwrap_or(Ordering::Equal)
                });
                let mut alternating = Vec::new();
                let mid = sorted_legs.len() / 2;
                let (high, low): (Vec<_>, Vec<_>) =
                    sorted_legs.into_iter().partition(|l| l.odds > 2.0);

                for (i, leg) in high.iter().enumerate() {
                    alternating.push(leg.clone());
                    if i < low.len() {
                        alternating.push(low[i].clone());
                    }
                }
                for leg in low.iter().skip(high.len()) {
                    alternating.push(leg.clone());
                }
                sorted_legs = alternating;
            }
            ReorderStrategy::EarliestFirst => {
                sorted_legs.sort_by(|a, b| {
                    match (a.event_time_minutes, b.event_time_minutes) {
                        (Some(a_time), Some(b_time)) => a_time.cmp(&b_time),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => Ordering::Equal,
                    }
                });
            }
            ReorderStrategy::HighestFormFirst => {
                sorted_legs.sort_by(|a, b| {
                    b.form_score.partial_cmp(&a.form_score).unwrap_or(Ordering::Equal)
                });
            }
            ReorderStrategy::Smart => {
                // Score-based reordering: favor high odds, early times, and good form
                sorted_legs.sort_by(|a, b| {
                    let score_a = self.calculate_smart_score(a);
                    let score_b = self.calculate_smart_score(b);
                    score_b.partial_cmp(&score_a).unwrap_or(Ordering::Equal)
                });
            }
        }

        let reordered: Vec<String> = sorted_legs.iter().map(|l| l.event_id.clone()).collect();

        // Calculate cumulative odds
        let mut cumulative_odds = Vec::new();
        let mut acc = 1.0;
        for leg in &sorted_legs {
            acc *= leg.odds;
            cumulative_odds.push(acc);
        }

        // Calculate efficiency score (how much better than original order)
        let original_cumulative: f64 = legs.iter().map(|l| l.odds).product();
        let best_possible_cumulative: f64 = {
            let mut sorted = legs.clone();
            sorted.sort_by(|a, b| b.odds.partial_cmp(&a.odds).unwrap_or(Ordering::Equal));
            sorted.iter().map(|l| l.odds).product()
        };

        let reordered_cumulative: f64 = sorted_legs.iter().map(|l| l.odds).product();

        let efficiency_score = if best_possible_cumulative > 0.0 {
            (reordered_cumulative - original_cumulative) / (best_possible_cumulative - original_cumulative)
        } else {
            0.0
        };

        // Calculate early win probability (odds of first 2-3 legs winning)
        let early_win_probability = sorted_legs
            .iter()
            .take(3)
            .map(|l| l.form_score)
            .product::<f64>();

        ReorderResult {
            strategy: format!("{:?}", self.strategy),
            original_order,
            reordered,
            cumulative_odds,
            efficiency_score: efficiency_score.max(0.0),
            early_win_probability,
        }
    }

    /// Calculate smart score for a leg
    fn calculate_smart_score(&self, leg: &ScheduledLeg) -> f64 {
        let odds_score = (leg.odds - 1.0) / 3.0; // Normalize to 0-1 range (for odds 1-4)
        let form_score = leg.form_score; // Already 0-1
        let time_score = match leg.event_time_minutes {
            Some(minutes) if minutes < 60 => 1.0,
            Some(minutes) if minutes < 180 => 0.8,
            Some(minutes) if minutes < 600 => 0.6,
            Some(_) => 0.4,
            None => 0.3,
        };

        // Weighted combination: odds 40%, form 35%, time 25%
        (odds_score * 0.4) + (form_score * 0.35) + (time_score * 0.25)
    }

    /// Calculate impact of reordering on expected returns
    pub fn calculate_reorder_impact(
        &self,
        original_cumulative: f64,
        reordered_cumulative: f64,
        stake: f64,
    ) -> f64 {
        let original_return = stake * original_cumulative;
        let reordered_return = stake * reordered_cumulative;
        ((reordered_return - original_return) / original_return) * 100.0
    }

    /// Suggest optimal reordering strategy based on legs
    pub fn suggest_strategy(legs: &[ScheduledLeg]) -> ReorderStrategy {
        if legs.is_empty() {
            return ReorderStrategy::Smart;
        }

        // Check if most legs have early start times
        let early_count = legs.iter().filter(|l| {
            l.event_time_minutes.map_or(false, |t| t < 120)
        }).count();

        if early_count > legs.len() / 2 {
            ReorderStrategy::EarliestFirst
        } else {
            // Check average form score
            let avg_form: f64 = legs.iter().map(|l| l.form_score).sum::<f64>() / legs.len() as f64;
            if avg_form > 0.75 {
                ReorderStrategy::HighestFormFirst
            } else {
                ReorderStrategy::HighestOddsFirst
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_leg(id: &str, odds: f64, form: f64, minutes: Option<u32>) -> ScheduledLeg {
        ScheduledLeg {
            position: 0,
            event_id: id.to_string(),
            odds,
            market: "1X2".to_string(),
            selection: "1".to_string(),
            event_time_minutes: minutes,
            form_score: form,
            bookmaker: "test".to_string(),
        }
    }

    #[test]
    fn test_highest_odds_first() {
        let legs = vec![
            make_leg("e1", 2.0, 0.8, Some(60)),
            make_leg("e2", 3.5, 0.7, Some(120)),
            make_leg("e3", 1.5, 0.9, Some(90)),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::HighestOddsFirst);
        let result = reorderer.reorder(legs);

        assert_eq!(result.reordered[0], "e2"); // 3.5
        assert_eq!(result.reordered[1], "e1"); // 2.0
        assert_eq!(result.reordered[2], "e3"); // 1.5
    }

    #[test]
    fn test_lowest_odds_first() {
        let legs = vec![
            make_leg("e1", 2.0, 0.8, Some(60)),
            make_leg("e2", 3.5, 0.7, Some(120)),
            make_leg("e3", 1.5, 0.9, Some(90)),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::LowestOddsFirst);
        let result = reorderer.reorder(legs);

        assert_eq!(result.reordered[0], "e3"); // 1.5
        assert_eq!(result.reordered[1], "e1"); // 2.0
        assert_eq!(result.reordered[2], "e2"); // 3.5
    }

    #[test]
    fn test_earliest_first() {
        let legs = vec![
            make_leg("e1", 2.0, 0.8, Some(300)),
            make_leg("e2", 3.5, 0.7, Some(60)),
            make_leg("e3", 1.5, 0.9, Some(120)),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::EarliestFirst);
        let result = reorderer.reorder(legs);

        assert_eq!(result.reordered[0], "e2"); // 60 min
        assert_eq!(result.reordered[1], "e3"); // 120 min
        assert_eq!(result.reordered[2], "e1"); // 300 min
    }

    #[test]
    fn test_highest_form_first() {
        let legs = vec![
            make_leg("e1", 2.0, 0.6, Some(60)),
            make_leg("e2", 3.5, 0.95, Some(120)),
            make_leg("e3", 1.5, 0.8, Some(90)),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::HighestFormFirst);
        let result = reorderer.reorder(legs);

        assert_eq!(result.reordered[0], "e2"); // 0.95 form
        assert_eq!(result.reordered[1], "e3"); // 0.8 form
        assert_eq!(result.reordered[2], "e1"); // 0.6 form
    }

    #[test]
    fn test_smart_reordering() {
        let legs = vec![
            make_leg("e1", 2.0, 0.8, Some(60)),
            make_leg("e2", 3.5, 0.7, Some(300)),
            make_leg("e3", 1.5, 0.9, Some(90)),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::Smart);
        let result = reorderer.reorder(legs);

        assert_eq!(result.reordered.len(), 3);
        assert!(result.efficiency_score >= 0.0);
    }

    #[test]
    fn test_efficiency_score() {
        let legs = vec![
            make_leg("e1", 2.0, 0.8, Some(60)),
            make_leg("e2", 3.5, 0.7, Some(120)),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::HighestOddsFirst);
        let result = reorderer.reorder(legs);

        assert!(result.efficiency_score > 0.0);
    }

    #[test]
    fn test_cumulative_odds() {
        let legs = vec![
            make_leg("e1", 2.0, 0.8, None),
            make_leg("e2", 3.0, 0.7, None),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::HighestOddsFirst);
        let result = reorderer.reorder(legs);

        assert_eq!(result.cumulative_odds.len(), 2);
        assert!((result.cumulative_odds[0] - 3.0).abs() < 0.01);
        assert!((result.cumulative_odds[1] - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_reorder_impact_positive() {
        let reorderer = LegReorderer::new(ReorderStrategy::HighestOddsFirst);
        let impact = reorderer.calculate_reorder_impact(4.0, 6.0, 1000.0);
        assert!(impact > 0.0);
    }

    #[test]
    fn test_early_win_probability() {
        let legs = vec![
            make_leg("e1", 2.0, 0.95, Some(60)),
            make_leg("e2", 3.5, 0.90, Some(120)),
            make_leg("e3", 1.5, 0.85, Some(90)),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::Smart);
        let result = reorderer.reorder(legs);

        assert!(result.early_win_probability > 0.0);
        assert!(result.early_win_probability <= 1.0);
    }

    #[test]
    fn test_suggest_strategy_early_events() {
        let legs = vec![
            make_leg("e1", 2.0, 0.8, Some(30)),
            make_leg("e2", 3.5, 0.7, Some(45)),
            make_leg("e3", 1.5, 0.9, Some(60)),
        ];

        let suggested = LegReorderer::suggest_strategy(&legs);
        assert!(matches!(suggested, ReorderStrategy::EarliestFirst));
    }

    #[test]
    fn test_suggest_strategy_high_form() {
        let legs = vec![
            make_leg("e1", 2.0, 0.92, None),
            make_leg("e2", 3.5, 0.88, None),
            make_leg("e3", 1.5, 0.85, None),
        ];

        let suggested = LegReorderer::suggest_strategy(&legs);
        assert!(matches!(suggested, ReorderStrategy::HighestFormFirst));
    }

    #[test]
    fn test_empty_reorder() {
        let reorderer = LegReorderer::new(ReorderStrategy::Smart);
        let result = reorderer.reorder(vec![]);
        assert!(result.reordered.is_empty());
    }

    #[test]
    fn test_alternating_high_low() {
        let legs = vec![
            make_leg("e1", 1.5, 0.8, None),
            make_leg("e2", 2.5, 0.7, None),
            make_leg("e3", 3.0, 0.9, None),
            make_leg("e4", 1.8, 0.6, None),
        ];

        let reorderer = LegReorderer::new(ReorderStrategy::AlternatingHighLow);
        let result = reorderer.reorder(legs);

        assert_eq!(result.reordered.len(), 4);
    }
}
