use chrono::Utc;
use shared::{BonusInfo, BonusPlan, BonusStatus, BonusStep, BonusStepStatus};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct BonusPlanner {
    active_plans: HashMap<String, BonusPlan>,
}

impl BonusPlanner {
    pub fn new() -> Self {
        Self {
            active_plans: HashMap::new(),
        }
    }

    pub fn create_plan(&mut self, bonus: &BonusInfo) -> BonusPlan {
        let total_steps = (bonus.wager_requirement.ceil() as u32).min(50);
        let _wager_per_step = bonus.wager_requirement / total_steps as f64;
        let stake_per_step = bonus.max_bet.min(bonus.amount / total_steps as f64);

        let steps: Vec<BonusStep> = (0..total_steps)
            .map(|i| BonusStep {
                step_number: i + 1,
                description: format!(
                    "Step {}/{}: Place bet with odds >= {:.2}",
                    i + 1,
                    total_steps,
                    bonus.min_odds
                ),
                market: "1X2".into(),
                selection: "Value bet".into(),
                bookmaker: bonus.bookmaker.clone(),
                odds: bonus.min_odds.max(1.8),
                stake: stake_per_step,
                status: BonusStepStatus::Pending,
            })
            .collect();

        let plan = BonusPlan {
            id: Uuid::new_v4(),
            bookmaker: bonus.bookmaker.clone(),
            bonus_name: bonus.name.clone(),
            bonus_amount: bonus.amount,
            wager_required: bonus.wager_requirement * bonus.amount,
            wager_done: 0.0,
            progress_percent: 0.0,
            estimated_profit: bonus.ev,
            steps,
            created_at: Utc::now(),
            status: BonusStatus::Claimed,
        };

        self.active_plans
            .insert(bonus.bookmaker.clone(), plan.clone());
        plan
    }

    pub fn update_progress(&mut self, bookmaker: &str, wager_done: f64) {
        if let Some(plan) = self.active_plans.get_mut(bookmaker) {
            plan.wager_done = wager_done;
            plan.progress_percent = if plan.wager_required > 0.0 {
                (wager_done / plan.wager_required * 100.0).min(100.0)
            } else {
                100.0
            };

            if plan.progress_percent >= 100.0 {
                plan.status = BonusStatus::Completed;
            }

            let completed_steps =
                (plan.progress_percent / 100.0 * plan.steps.len() as f64) as usize;
            for (i, step) in plan.steps.iter_mut().enumerate() {
                if i < completed_steps {
                    step.status = BonusStepStatus::Won;
                } else if i == completed_steps {
                    step.status = BonusStepStatus::Pending;
                }
            }
        }
    }

    pub fn get_next_step(&self, bookmaker: &str) -> Option<&BonusStep> {
        self.active_plans.get(bookmaker).and_then(|plan| {
            plan.steps
                .iter()
                .find(|s| matches!(s.status, BonusStepStatus::Pending))
        })
    }

    pub fn get_all_plans(&self) -> Vec<&BonusPlan> {
        self.active_plans.values().collect()
    }

    pub fn get_plan(&self, bookmaker: &str) -> Option<&BonusPlan> {
        self.active_plans.get(bookmaker)
    }
}

impl Default for BonusPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_bonus(bookmaker: &str, amount: f64, wager: f64) -> BonusInfo {
        BonusInfo {
            id: Uuid::new_v4(),
            bookmaker: bookmaker.to_string(),
            name: "Welcome Bonus".into(),
            bonus_type: shared::BonusType::Welcome,
            amount,
            wager_requirement: wager,
            min_odds: 1.80,
            max_bet: 100.0,
            currency: "RUB".into(),
            expiry_days: 30,
            ev: amount * 0.7,
            real_value: amount * 0.5,
            difficulty: shared::BonusDifficulty::Medium,
            status: shared::BonusStatus::Available,
            wager_progress: 0.0,
            detected_at: Utc::now(),
            url: None,
        }
    }

    #[test]
    fn test_create_plan() {
        let mut planner = BonusPlanner::new();
        let bonus = make_test_bonus("pari", 5000.0, 5.0);

        let plan = planner.create_plan(&bonus);

        assert_eq!(plan.bookmaker, "pari");
        assert_eq!(plan.bonus_amount, 5000.0);
        assert_eq!(plan.wager_required, 25000.0); // 5000 * 5
        assert_eq!(plan.status, BonusStatus::Claimed);
        assert!(!plan.steps.is_empty());
        assert!(plan.steps.len() <= 50);
    }

    #[test]
    fn test_update_progress() {
        let mut planner = BonusPlanner::new();
        let bonus = make_test_bonus("marathon", 1000.0, 3.0);
        planner.create_plan(&bonus);

        planner.update_progress("marathon", 1500.0);
        let plan = planner.get_plan("marathon").unwrap();

        assert_eq!(plan.wager_done, 1500.0);
        assert_eq!(plan.wager_required, 3000.0);
        assert!((plan.progress_percent - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_update_progress_completes_plan() {
        let mut planner = BonusPlanner::new();
        let bonus = make_test_bonus("winline", 2000.0, 2.0);
        planner.create_plan(&bonus);

        planner.update_progress("winline", 4000.0); // 2000 * 2 = 4000
        let plan = planner.get_plan("winline").unwrap();

        assert_eq!(plan.status, BonusStatus::Completed);
        assert!((plan.progress_percent - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_get_next_step() {
        let mut planner = BonusPlanner::new();
        let bonus = make_test_bonus("bettery", 3000.0, 3.0);
        planner.create_plan(&bonus);

        let next = planner.get_next_step("bettery");
        assert!(next.is_some());
        assert_eq!(next.unwrap().step_number, 1);
        assert!(matches!(next.unwrap().status, BonusStepStatus::Pending));
    }

    #[test]
    fn test_get_all_plans() {
        let mut planner = BonusPlanner::new();
        planner.create_plan(&make_test_bonus("bk1", 1000.0, 2.0));
        planner.create_plan(&make_test_bonus("bk2", 2000.0, 3.0));

        let plans = planner.get_all_plans();
        assert_eq!(plans.len(), 2);
    }

    #[test]
    fn test_progress_caps_at_100() {
        let mut planner = BonusPlanner::new();
        let bonus = make_test_bonus("bk1", 1000.0, 2.0);
        planner.create_plan(&bonus);

        // Переусердствуем — больше чем требуется
        planner.update_progress("bk1", 5000.0);
        let plan = planner.get_plan("bk1").unwrap();

        assert_eq!(plan.progress_percent, 100.0);
        assert_eq!(plan.status, BonusStatus::Completed);
    }

    #[test]
    fn test_plan_steps_count_capped_at_50() {
        let mut planner = BonusPlanner::new();
        // wager_requirement = 100 => total_steps = 100, но capped at 50
        let bonus = make_test_bonus("bk1", 1000.0, 100.0);
        let plan = planner.create_plan(&bonus);

        assert!(plan.steps.len() <= 50);
    }
}
