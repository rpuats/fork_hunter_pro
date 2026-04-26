/// Hedging calculator for express fork exposure management
///
/// Provides strategies to protect express fork positions by hedging
/// portions of the parlay with opposing selections from other bookmakers.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hedge strategy configuration
#[derive(Debug, Clone, Copy)]
pub enum HedgeStrategy {
    /// Hedge X% of total exposure
    Percentage(f64),
    /// Hedge at specific odds threshold (hedge if opposition odds >= threshold)
    OddsThreshold(f64),
    /// Hedge with specific ROI target
    TargetROI(f64),
    /// Dynamic hedge based on legs count
    DynamicByLegs,
}

/// Represents a hedge position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgePosition {
    pub leg_index: usize,
    pub event_id: String,
    pub opposing_selection: String,
    pub hedge_odds: f64,
    pub hedge_stake: f64,
    pub bookmaker: String,
}

/// Hedge analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgeAnalysis {
    pub strategy: String,
    pub hedge_percentage: f64,
    pub total_hedge_stake: f64,
    pub positions: Vec<HedgePosition>,
    pub max_exposure: f64,
    pub guaranteed_profit: f64,
    pub breakeven_hedge_odds: f64,
}

/// Hedging calculator
pub struct HedgeCalculator {
    strategy: HedgeStrategy,
    min_hedge_odds: f64,
}

impl HedgeCalculator {
    pub fn new(strategy: HedgeStrategy) -> Self {
        Self {
            strategy,
            min_hedge_odds: 1.5,
        }
    }

    pub fn with_min_odds(mut self, min_odds: f64) -> Self {
        self.min_hedge_odds = min_odds;
        self
    }

    /// Calculate optimal hedge stake for given express parlay
    pub fn calculate_hedge_stake(
        &self,
        express_odds: f64,
        express_stake: f64,
        backup_odds: f64,
    ) -> f64 {
        match self.strategy {
            HedgeStrategy::Percentage(pct) => express_stake * (pct / 100.0),
            HedgeStrategy::OddsThreshold(threshold) => {
                if backup_odds >= threshold {
                    // Scale hedge stake based on how much worse the backup odds are
                    let odds_ratio = backup_odds / express_odds;
                    (express_stake * express_odds / backup_odds).min(express_stake * 0.5)
                } else {
                    0.0
                }
            }
            HedgeStrategy::TargetROI(target_roi) => {
                // Calculate stake to achieve target ROI after hedge
                self.calculate_roi_hedge_stake(express_odds, express_stake, backup_odds, target_roi)
            }
            HedgeStrategy::DynamicByLegs => {
                // Will be overridden by specific leg-count logic
                0.0
            }
        }
    }

    /// Calculate hedge stake to achieve specific ROI
    fn calculate_roi_hedge_stake(
        &self,
        express_odds: f64,
        express_stake: f64,
        backup_odds: f64,
        target_roi: f64,
    ) -> f64 {
        // Solve: (express_stake * express_odds - hedge_stake) - (express_stake + hedge_stake)
        //        = express_stake * (target_roi / 100.0)
        let numerator = express_stake * (1.0 + target_roi / 100.0);
        let denominator = express_odds + backup_odds;
        (numerator / denominator).max(0.0)
    }

    /// Analyze hedging effectiveness for multi-leg parlay
    pub fn analyze_hedge(
        &self,
        express_odds: f64,
        express_stake: f64,
        leg_count: usize,
        available_oppositions: Vec<(usize, f64, String)>, // (leg_idx, odds, bookmaker)
    ) -> HedgeAnalysis {
        let hedge_percentage = self.get_hedge_percentage(leg_count);
        let total_hedge_stake = express_stake * (hedge_percentage / 100.0);

        let mut positions = Vec::new();
        let mut stake_used = 0.0;

        // Allocate hedge stakes to legs with best opposition odds
        let mut sorted_legs: Vec<_> = available_oppositions.clone();
        sorted_legs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (leg_idx, odds, bookmaker) in sorted_legs.iter().take(leg_count.min(3)) {
            if stake_used >= total_hedge_stake {
                break;
            }

            let remaining_stake = total_hedge_stake - stake_used;
            let hedge_stake = remaining_stake / (leg_count.min(3) as f64);

            if *odds >= self.min_hedge_odds {
                positions.push(HedgePosition {
                    leg_index: *leg_idx,
                    event_id: format!("leg_{}", leg_idx),
                    opposing_selection: "hedge".to_string(),
                    hedge_odds: *odds,
                    hedge_stake,
                    bookmaker: bookmaker.clone(),
                });

                stake_used += hedge_stake;
            }
        }

        let total_stake = express_stake + stake_used;
        let max_exposure = total_stake;

        // Calculate guaranteed profit (if express wins)
        let guaranteed_profit = (express_stake * express_odds) - max_exposure;

        // Calculate breakeven hedge odds (odds where hedge makes the position neutral)
        let breakeven = max_exposure / express_stake;

        HedgeAnalysis {
            strategy: format!("{:?}", self.strategy),
            hedge_percentage,
            total_hedge_stake: stake_used,
            positions,
            max_exposure,
            guaranteed_profit,
            breakeven_hedge_odds: breakeven,
        }
    }

    /// Get hedge percentage for leg count
    pub fn get_hedge_percentage(&self, leg_count: usize) -> f64 {
        match self.strategy {
            HedgeStrategy::Percentage(pct) => pct,
            HedgeStrategy::DynamicByLegs => match leg_count {
                2 => 10.0,
                3 => 15.0,
                4 => 20.0,
                5 => 25.0,
                6 => 30.0,
                7 => 35.0,
                _ => 40.0,
            },
            _ => 20.0,
        }
    }

    /// Calculate total return after hedge if express wins
    pub fn calculate_hedged_return(
        &self,
        express_stake: f64,
        express_odds: f64,
        hedge_analysis: &HedgeAnalysis,
    ) -> f64 {
        let express_return = express_stake * express_odds;
        let hedge_losses: f64 = hedge_analysis.positions.iter().map(|h| h.hedge_stake).sum();

        express_return - hedge_losses
    }

    /// Calculate effective ROI after hedging
    pub fn calculate_effective_roi(
        &self,
        express_odds: f64,
        express_stake: f64,
        hedge_analysis: &HedgeAnalysis,
    ) -> f64 {
        let total_invested = express_stake + hedge_analysis.total_hedge_stake;
        let hedged_return =
            self.calculate_hedged_return(express_stake, express_odds, hedge_analysis);
        let profit = hedged_return - total_invested;
        (profit / total_invested) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hedge_percentage_strategy() {
        let calc = HedgeCalculator::new(HedgeStrategy::Percentage(20.0));
        let stake = calc.calculate_hedge_stake(3.0, 1000.0, 2.0);
        assert!((stake - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_hedge_odds_threshold_strategy() {
        let calc = HedgeCalculator::new(HedgeStrategy::OddsThreshold(1.8));
        let stake1 = calc.calculate_hedge_stake(3.0, 1000.0, 1.95); // Above threshold
        let stake2 = calc.calculate_hedge_stake(3.0, 1000.0, 1.70); // Below threshold
        assert!(stake1 > stake2);
    }

    #[test]
    fn test_hedge_roi_target_strategy() {
        let calc = HedgeCalculator::new(HedgeStrategy::TargetROI(5.0));
        let stake = calc.calculate_hedge_stake(3.0, 1000.0, 2.0);
        assert!(stake > 0.0);
        assert!(stake < 500.0);
    }

    #[test]
    fn test_dynamic_hedge_percentages() {
        let calc = HedgeCalculator::new(HedgeStrategy::DynamicByLegs);
        assert_eq!(calc.get_hedge_percentage(2), 10.0);
        assert_eq!(calc.get_hedge_percentage(3), 15.0);
        assert_eq!(calc.get_hedge_percentage(5), 25.0);
        assert_eq!(calc.get_hedge_percentage(7), 35.0);
    }

    #[test]
    fn test_analyze_hedge_multi_leg() {
        let calc = HedgeCalculator::new(HedgeStrategy::Percentage(20.0)).with_min_odds(1.5);
        let oppositions = vec![
            (0, 2.1, "bk1".to_string()),
            (1, 1.95, "bk2".to_string()),
            (2, 2.05, "bk3".to_string()),
        ];

        let analysis = calc.analyze_hedge(3.0, 1000.0, 3, oppositions);
        assert_eq!(analysis.positions.len(), 3);
        assert!((analysis.total_hedge_stake - 200.0).abs() < 1.0);
    }

    #[test]
    fn test_hedged_return_calculation() {
        let calc = HedgeCalculator::new(HedgeStrategy::Percentage(20.0));
        let analysis = HedgeAnalysis {
            strategy: "test".to_string(),
            hedge_percentage: 20.0,
            total_hedge_stake: 200.0,
            positions: vec![],
            max_exposure: 1200.0,
            guaranteed_profit: 900.0,
            breakeven_hedge_odds: 1.2,
        };

        let return_val = calc.calculate_hedged_return(1000.0, 3.0, &analysis);
        assert!(return_val > 0.0);
    }

    #[test]
    fn test_effective_roi_after_hedge() {
        let calc = HedgeCalculator::new(HedgeStrategy::Percentage(15.0));
        let analysis = HedgeAnalysis {
            strategy: "test".to_string(),
            hedge_percentage: 15.0,
            total_hedge_stake: 150.0,
            positions: vec![],
            max_exposure: 1150.0,
            guaranteed_profit: 850.0,
            breakeven_hedge_odds: 1.15,
        };

        let roi = calc.calculate_effective_roi(3.5, 1000.0, &analysis);
        assert!(roi > 0.0);
    }

    #[test]
    fn test_min_odds_filter() {
        let calc = HedgeCalculator::new(HedgeStrategy::Percentage(25.0)).with_min_odds(2.0);
        let oppositions = vec![(0, 1.5, "bk1".to_string()), (1, 2.1, "bk2".to_string())];

        let analysis = calc.analyze_hedge(3.0, 1000.0, 2, oppositions);
        // Only position with odds >= 2.0 should be included
        assert!(analysis.positions.len() <= 1);
    }

    #[test]
    fn test_hedge_with_extreme_odds() {
        let calc = HedgeCalculator::new(HedgeStrategy::OddsThreshold(2.5));
        let stake = calc.calculate_hedge_stake(10.0, 5000.0, 3.0);
        assert!(stake > 0.0 && stake < 5000.0);
    }
}
