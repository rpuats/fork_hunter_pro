use chrono::Utc;
use shared::{
    FreebetConversionPlan, FreebetHedgeLeg, FreebetPlanRequest, FreebetPlanStep, FreebetStepType,
};
use std::collections::HashMap;
use uuid::Uuid;

pub struct FreebetPlanner;

impl FreebetPlanner {
    pub fn build_plan(request: &FreebetPlanRequest) -> FreebetConversionPlan {
        let freebet_retention = if request.exchange_like_hedge {
            ((request.back_odds - request.lay_odds.max(1.01)) / request.back_odds).max(0.0)
        } else {
            ((request.back_odds - 1.0) / request.back_odds).max(0.0)
        };

        let qualifying_cash_stake = if request.qualifying_odds > 1.0 {
            (request.freebet_amount / request.qualifying_odds).max(0.0)
        } else {
            0.0
        };
        let hedge_stake = if request.lay_odds > 1.0 {
            (request.freebet_amount * (request.back_odds - 1.0) / request.lay_odds).max(0.0)
        } else {
            0.0
        };
        let estimated_profit =
            (request.freebet_amount * freebet_retention) - request.estimated_qualifying_loss;

        let hedge = FreebetHedgeLeg {
            bookmaker: request.hedge_bookmaker.clone(),
            market: request.market.clone(),
            selection: request.hedge_selection.clone(),
            odds: request.lay_odds,
            stake: hedge_stake,
        };

        let required_cash_by_bookmaker = build_required_cash_by_bookmaker(
            &request.qualifying_bookmaker,
            qualifying_cash_stake,
            &request.hedge_bookmaker,
            hedge_stake,
        );
        let funding_recommendation = build_funding_recommendation(
            &required_cash_by_bookmaker,
            &request.freebet_bookmaker,
            request.freebet_amount,
        );

        let steps = vec![
            FreebetPlanStep {
                step_number: 1,
                step_type: FreebetStepType::QualifyingBet,
                bookmaker: request.qualifying_bookmaker.clone(),
                market: request.market.clone(),
                selection: request.qualifying_selection.clone(),
                odds: request.qualifying_odds,
                stake: qualifying_cash_stake,
                note: "Place qualifying bet to unlock freebet or satisfy campaign terms".into(),
            },
            FreebetPlanStep {
                step_number: 2,
                step_type: FreebetStepType::FreebetBet,
                bookmaker: request.freebet_bookmaker.clone(),
                market: request.market.clone(),
                selection: request.freebet_selection.clone(),
                odds: request.back_odds,
                stake: request.freebet_amount,
                note: "Use freebet on higher odds where retained value is stronger".into(),
            },
            FreebetPlanStep {
                step_number: 3,
                step_type: FreebetStepType::Hedge,
                bookmaker: request.hedge_bookmaker.clone(),
                market: request.market.clone(),
                selection: request.hedge_selection.clone(),
                odds: request.lay_odds,
                stake: hedge_stake,
                note: "Hedge outcome to stabilize realized conversion value".into(),
            },
        ];

        FreebetConversionPlan {
            id: Uuid::new_v4(),
            bookmaker: request.freebet_bookmaker.clone(),
            freebet_amount: request.freebet_amount,
            qualifying_cost: request.estimated_qualifying_loss,
            conversion_rate: freebet_retention,
            estimated_profit,
            required_cash_by_bookmaker,
            funding_recommendation,
            hedge,
            steps,
            created_at: Utc::now(),
        }
    }
}

fn build_required_cash_by_bookmaker(
    qualifying_bookmaker: &str,
    qualifying_cash_stake: f64,
    hedge_bookmaker: &str,
    hedge_stake: f64,
) -> HashMap<String, f64> {
    let mut required_cash_by_bookmaker = HashMap::new();

    for (bookmaker, amount) in [
        (qualifying_bookmaker, qualifying_cash_stake),
        (hedge_bookmaker, hedge_stake),
    ] {
        if amount > 0.0 {
            *required_cash_by_bookmaker
                .entry(bookmaker.to_string())
                .or_insert(0.0) += amount;
        }
    }

    required_cash_by_bookmaker
}

fn build_funding_recommendation(
    required_cash_by_bookmaker: &HashMap<String, f64>,
    freebet_bookmaker: &str,
    freebet_amount: f64,
) -> String {
    if required_cash_by_bookmaker.is_empty() {
        return format!(
            "No cash funding required before conversion; place the {freebet_amount:.2} freebet directly at {freebet_bookmaker}."
        );
    }

    let mut parts: Vec<String> = required_cash_by_bookmaker
        .iter()
        .map(|(bookmaker, amount)| format!("{bookmaker}: {amount:.2}"))
        .collect();
    parts.sort();

    format!(
        "Keep cash ready before starting the sequence: {}. The {freebet_amount:.2} freebet itself is placed at {freebet_bookmaker} without extra cash stake.",
        parts.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_three_step_plan() {
        let request = FreebetPlanRequest {
            freebet_bookmaker: "pari".into(),
            qualifying_bookmaker: "pari".into(),
            hedge_bookmaker: "fonbet".into(),
            market: "1X2".into(),
            qualifying_selection: "1".into(),
            freebet_selection: "X".into(),
            hedge_selection: "not X".into(),
            freebet_amount: 1_000.0,
            qualifying_odds: 2.0,
            back_odds: 4.0,
            lay_odds: 3.8,
            estimated_qualifying_loss: 50.0,
            exchange_like_hedge: true,
        };

        let plan = FreebetPlanner::build_plan(&request);
        assert_eq!(plan.steps.len(), 3);
        assert!(plan.conversion_rate > 0.0);
        assert!(
            (plan.required_cash_by_bookmaker["pari"] - 500.0).abs() < 1e-9,
            "unexpected pari cash requirement"
        );
        assert!(
            (plan.required_cash_by_bookmaker["fonbet"] - 789.4736842105264).abs() < 1e-9,
            "unexpected fonbet cash requirement"
        );
        assert!(plan.funding_recommendation.contains("pari: 500.00"));
        assert!(plan.funding_recommendation.contains("fonbet: 789.47"));
    }

    #[test]
    fn merges_cash_requirements_when_same_bookmaker_handles_both_steps() {
        let request = FreebetPlanRequest {
            freebet_bookmaker: "pari".into(),
            qualifying_bookmaker: "pari".into(),
            hedge_bookmaker: "pari".into(),
            market: "1X2".into(),
            qualifying_selection: "1".into(),
            freebet_selection: "X".into(),
            hedge_selection: "not X".into(),
            freebet_amount: 1_000.0,
            qualifying_odds: 2.0,
            back_odds: 4.0,
            lay_odds: 2.0,
            estimated_qualifying_loss: 50.0,
            exchange_like_hedge: false,
        };

        let plan = FreebetPlanner::build_plan(&request);

        assert_eq!(plan.required_cash_by_bookmaker.len(), 1);
        assert!((plan.required_cash_by_bookmaker["pari"] - 2_000.0).abs() < 1e-9);
        assert!(plan.funding_recommendation.contains("pari: 2000.00"));
    }
}
