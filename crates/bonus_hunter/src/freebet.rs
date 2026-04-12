use chrono::Utc;
use shared::{
    FreebetConversionPlan, FreebetHedgeLeg, FreebetPlanRequest, FreebetPlanStep, FreebetStepType,
};
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
            hedge,
            steps,
            created_at: Utc::now(),
        }
    }
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
    }
}
