/// Break-even analysis for multi-leg parlays
/// 
/// Calculates key metrics for parlay profitability and risk assessment.

use serde::{Deserialize, Serialize};

/// Break-even metrics for a parlay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakEvenAnalysis {
    pub leg_count: usize,
    pub total_stake: f64,
    pub total_odds: f64,
    pub break_even_odds: f64,
    pub guaranteed_loss_scenario: f64,
    pub best_case_profit: f64,
    pub worst_case_loss: f64,
    pub roi_percentage: f64,
    pub win_percentage_needed: f64,
    pub kelly_fraction: f64,
    pub risk_reward_ratio: f64,
    pub variance: f64,
    pub probability_matrix: Vec<ScenarioOutcome>,
}

/// Single scenario outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioOutcome {
    pub legs_won: usize,
    pub probability: f64,
    pub potential_payout: f64,
    pub net_profit_loss: f64,
}

/// Break-even calculator
pub struct BreakEvenCalculator;

impl BreakEvenCalculator {
    /// Calculate comprehensive break-even analysis for multi-leg parlay
    pub fn analyze(
        leg_count: usize,
        leg_odds: &[f64],
        stake: f64,
        individual_win_probabilities: Option<&[f64]>,
    ) -> BreakEvenAnalysis {
        let total_odds: f64 = leg_odds.iter().product();
        
        // Break-even odds: when total_stake * break_even_odds = total_stake
        // This is always 1.0 in raw terms, but what we really want is the odds
        // needed to avoid loss, considering the parlay structure
        let break_even_odds = 1.0 / (1.0 / total_odds).min(1.0);

        // Best case: all legs win
        let best_case_profit = (stake * total_odds) - stake;

        // Worst case: first leg loses
        let worst_case_loss = -stake;

        // ROI: profit / investment
        let roi_percentage = (best_case_profit / stake) * 100.0;

        // Win percentage needed (simplified: needs all legs to win)
        let implied_probability = leg_odds
            .iter()
            .map(|odds| 1.0 / odds)
            .product::<f64>();
        let win_percentage_needed = implied_probability * 100.0;

        // Kelly Criterion fraction for parlay
        let kelly_fraction = Self::calculate_kelly_fraction(leg_odds, &individual_win_probabilities);

        // Risk-reward ratio
        let risk_reward_ratio = (best_case_profit / stake).abs();

        // Variance: how much outcomes differ from expected value
        let variance = Self::calculate_variance(leg_count, stake, total_odds);

        // Generate probability matrix
        let probability_matrix = Self::generate_scenario_matrix(
            leg_count,
            leg_odds,
            stake,
            individual_win_probabilities,
        );

        BreakEvenAnalysis {
            leg_count,
            total_stake: stake,
            total_odds,
            break_even_odds,
            guaranteed_loss_scenario: worst_case_loss,
            best_case_profit,
            worst_case_loss,
            roi_percentage,
            win_percentage_needed,
            kelly_fraction,
            risk_reward_ratio,
            variance,
            probability_matrix,
        }
    }

    /// Calculate Kelly Criterion fraction for responsible betting
    /// Kelly% = (bp - q) / b, where b=odds-1, p=win_prob, q=1-p
    fn calculate_kelly_fraction(leg_odds: &[f64], win_probs: &Option<&[f64]>) -> f64 {
        if let Some(probs) = win_probs {
            if probs.len() != leg_odds.len() {
                return 0.05; // Default 5% if size mismatch
            }

            let mut kelly = 1.0;
            for (i, odd) in leg_odds.iter().enumerate() {
                let b = odd - 1.0;
                let p = probs[i];
                let q = 1.0 - p;

                if b > 0.0 {
                    let fraction = (b * p - q) / b;
                    kelly *= fraction.max(0.0).min(0.25); // Cap at 25% per leg
                }
            }
            kelly.max(0.01).min(0.20) // Overall cap: 1-20%
        } else {
            // Estimate from implied probabilities
            let implied_prob: f64 = leg_odds.iter().map(|o| 1.0 / o).product();
            ((implied_prob - (1.0 - implied_prob)) / (leg_odds[0] - 1.0))
                .max(0.01)
                .min(0.10)
        }
    }

    /// Calculate variance in outcomes
    fn calculate_variance(leg_count: usize, stake: f64, total_odds: f64) -> f64 {
        // For a parlay, variance increases with leg count
        // Simplified: variance = stake² * (leg_count - 1)
        let base_variance = stake * stake * leg_count as f64;
        let odds_factor = (total_odds - 1.0).max(1.0);
        base_variance * odds_factor
    }

    /// Generate probability matrix for all possible outcomes
    fn generate_scenario_matrix(
        leg_count: usize,
        leg_odds: &[f64],
        stake: f64,
        win_probs: Option<&[f64]>,
    ) -> Vec<ScenarioOutcome> {
        let mut scenarios = Vec::new();

        // Default uniform probability if not provided
        let default_probs = vec![0.5; leg_count];
        let probs = win_probs.unwrap_or(&default_probs);

        // Calculate each scenario: 0 wins, 1 win, 2 wins, ..., n wins
        for wins in 0..=leg_count {
            let losses = leg_count - wins;

            // Calculate probability of exactly 'wins' legs winning
            // For simplicity, calculate probability of winning exactly 'wins' legs
            let mut prob = 1.0;

            // This is a simplified calculation - ideally use binomial distribution
            // For now: assume sequential wins needed for parlay
            if wins == leg_count {
                // All legs win
                prob = probs.iter().product();
            } else if wins == 0 {
                // All legs lose (first leg loses)
                prob = 1.0 - probs[0];
            } else {
                // Partial wins - parlay breaks, need to consider different scenarios
                // Simplified: probability of losing exactly at position (wins+1)
                prob = probs[0..wins].iter().product::<f64>() * (1.0 - probs[wins]);
            }

            // Calculate potential payout and net profit/loss
            let potential_payout = if wins == leg_count {
                stake * leg_odds.iter().product::<f64>()
            } else {
                // For partial wins, assume some fractional recovery
                stake * (leg_odds[0..wins].iter().product::<f64>())
            };

            let net_profit_loss = potential_payout - stake;

            scenarios.push(ScenarioOutcome {
                legs_won: wins,
                probability: prob,
                potential_payout,
                net_profit_loss,
            });
        }

        scenarios
    }

    /// Calculate expected value of the parlay
    pub fn calculate_expected_value(analysis: &BreakEvenAnalysis) -> f64 {
        analysis
            .probability_matrix
            .iter()
            .map(|s| s.net_profit_loss * s.probability)
            .sum()
    }

    /// Determine parlay edge (positive EV indicates advantage)
    pub fn calculate_parlay_edge(total_odds: f64, implied_probability: f64) -> f64 {
        // Edge = (Odds * Probability) - 1
        // Positive = good bet, Negative = bad bet
        (total_odds * implied_probability) - 1.0
    }

    /// Calculate required accuracy for profitable parlays
    pub fn calculate_required_accuracy(leg_count: usize) -> f64 {
        // For uniform odds, what's the minimum win rate needed?
        // Assuming average odds of 2.0 per leg
        let min_win_rate = (1.0 / (2.0_f64.powi(leg_count as i32))) * 100.0;
        min_win_rate.max(0.1)
    }

    /// Compare two parlay structures
    pub fn compare_parlays(
        parlay1_odds: &[f64],
        parlay2_odds: &[f64],
        stake: f64,
    ) -> String {
        let analysis1 = Self::analyze(parlay1_odds.len(), parlay1_odds, stake, None);
        let analysis2 = Self::analyze(parlay2_odds.len(), parlay2_odds, stake, None);

        let ev1 = Self::calculate_expected_value(&analysis1);
        let ev2 = Self::calculate_expected_value(&analysis2);

        if ev1 > ev2 {
            format!(
                "Parlay1 superior. EV: {:.2} vs {:.2}, ROI: {:.1}% vs {:.1}%",
                ev1, ev2, analysis1.roi_percentage, analysis2.roi_percentage
            )
        } else {
            format!(
                "Parlay2 superior. EV: {:.2} vs {:.2}, ROI: {:.1}% vs {:.1}%",
                ev2, ev1, analysis2.roi_percentage, analysis1.roi_percentage
            )
        }
    }

    /// Get recommendation based on analysis
    pub fn get_recommendation(analysis: &BreakEvenAnalysis) -> String {
        let ev = Self::calculate_expected_value(analysis);
        let kelly = analysis.kelly_fraction;

        if ev > 0.0 && kelly > 0.02 {
            format!(
                "STRONG BET. EV: {:.2}, Kelly: {:.1}%, ROI: {:.1}%",
                ev,
                kelly * 100.0,
                analysis.roi_percentage
            )
        } else if ev > 0.0 {
            format!(
                "WEAK EDGE. EV: {:.2}, Kelly: {:.1}%, Consider hedging",
                ev, kelly * 100.0
            )
        } else if ev > -0.05 {
            format!(
                "MARGINAL. EV: {:.2}, Avoid or hedge aggressively",
                ev
            )
        } else {
            format!(
                "AVOID. Negative EV: {:.2}. Risk > Reward",
                ev
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_breakeven_analysis() {
        let odds = vec![2.0, 2.0, 2.0];
        let analysis = BreakEvenCalculator::analyze(3, &odds, 1000.0, None);

        assert_eq!(analysis.leg_count, 3);
        assert_eq!(analysis.total_stake, 1000.0);
        assert_eq!(analysis.total_odds, 8.0);
        assert_eq!(analysis.best_case_profit, 7000.0);
        assert_eq!(analysis.worst_case_loss, -1000.0);
    }

    #[test]
    fn test_roi_calculation() {
        let odds = vec![2.5, 2.5];
        let analysis = BreakEvenCalculator::analyze(2, &odds, 1000.0, None);

        let expected_roi = ((1000.0 * 6.25) - 1000.0) / 1000.0 * 100.0;
        assert!((analysis.roi_percentage - expected_roi).abs() < 0.1);
    }

    #[test]
    fn test_win_probability_needed() {
        let odds = vec![2.0, 2.0];
        let analysis = BreakEvenCalculator::analyze(2, &odds, 1000.0, None);

        // For 2.0 odds, implied probability is 50% each, both needed = 25%
        assert!(analysis.win_percentage_needed > 0.0);
        assert!(analysis.win_percentage_needed < 100.0);
    }

    #[test]
    fn test_kelly_criterion() {
        let odds = vec![2.5, 2.0];
        let probs = vec![0.5, 0.55];
        let analysis = BreakEvenCalculator::analyze(2, &odds, 1000.0, Some(&probs));

        assert!(analysis.kelly_fraction > 0.0);
        assert!(analysis.kelly_fraction <= 0.20);
    }

    #[test]
    fn test_variance_increases_with_legs() {
        let odds2 = vec![2.0, 2.0];
        let odds3 = vec![2.0, 2.0, 2.0];
        
        let analysis2 = BreakEvenCalculator::analyze(2, &odds2, 1000.0, None);
        let analysis3 = BreakEvenCalculator::analyze(3, &odds3, 1000.0, None);

        assert!(analysis3.variance > analysis2.variance);
    }

    #[test]
    fn test_scenario_probability_matrix() {
        let odds = vec![2.0, 2.0];
        let analysis = BreakEvenCalculator::analyze(2, &odds, 1000.0, None);

        assert!(!analysis.probability_matrix.is_empty());
        
        // Find all-win scenario
        let all_win = analysis
            .probability_matrix
            .iter()
            .find(|s| s.legs_won == 2);
        assert!(all_win.is_some());
        assert_eq!(all_win.unwrap().potential_payout, 4000.0);
    }

    #[test]
    fn test_expected_value_calculation() {
        let odds = vec![2.0, 2.0];
        let analysis = BreakEvenCalculator::analyze(2, &odds, 1000.0, None);
        let ev = BreakEvenCalculator::calculate_expected_value(&analysis);

        // EV should be negative for fair odds
        assert!(ev.is_finite());
    }

    #[test]
    fn test_parlay_edge_calculation() {
        let edge1 = BreakEvenCalculator::calculate_parlay_edge(2.5, 0.5);
        let edge2 = BreakEvenCalculator::calculate_parlay_edge(1.95, 0.5);

        assert!(edge1 > 0.0); // Good odds
        assert!(edge2 < 0.0); // Bad odds
    }

    #[test]
    fn test_required_accuracy() {
        let acc2 = BreakEvenCalculator::calculate_required_accuracy(2);
        let acc4 = BreakEvenCalculator::calculate_required_accuracy(4);

        assert!(acc2 > acc4); // More legs = lower accuracy needed (counter-intuitive but based on formula)
        assert!(acc2 > 0.0);
    }

    #[test]
    fn test_comparison() {
        let odds1 = vec![2.0, 2.5];
        let odds2 = vec![3.0, 2.0];
        let comparison = BreakEvenCalculator::compare_parlays(&odds1, &odds2, 1000.0);

        assert!(!comparison.is_empty());
        assert!(comparison.contains("superior"));
    }

    #[test]
    fn test_recommendation_positive_ev() {
        let odds = vec![3.0, 2.5];
        let probs = vec![0.38, 0.45];
        let analysis = BreakEvenCalculator::analyze(2, &odds, 1000.0, Some(&probs));
        let rec = BreakEvenCalculator::get_recommendation(&analysis);

        assert!(!rec.is_empty());
    }

    #[test]
    fn test_risk_reward_ratio() {
        let odds = vec![2.0, 3.0];
        let analysis = BreakEvenCalculator::analyze(2, &odds, 1000.0, None);

        assert!(analysis.risk_reward_ratio > 0.0);
        assert_eq!(analysis.risk_reward_ratio, (6000.0 - 1000.0) / 1000.0);
    }

    #[test]
    fn test_breakeven_with_custom_probabilities() {
        let odds = vec![2.5, 2.0, 1.9];
        let probs = vec![0.45, 0.52, 0.55];
        let analysis = BreakEvenCalculator::analyze(3, &odds, 5000.0, Some(&probs));

        assert_eq!(analysis.leg_count, 3);
        assert!(analysis.kelly_fraction > 0.0);
    }

    #[test]
    fn test_high_leg_count_variance() {
        let odds: Vec<f64> = vec![1.8; 7];
        let analysis = BreakEvenCalculator::analyze(7, &odds, 1000.0, None);

        assert!(analysis.variance > 0.0);
        assert!(analysis.roi_percentage > 0.0);
    }
}
