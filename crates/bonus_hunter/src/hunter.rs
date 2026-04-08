use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use shared::{BonusConfig, BonusInfo, BonusStatus, BonusType};
use std::sync::Arc;

use super::calculator::BonusCalculator;
use super::plan::BonusPlanner;

#[derive(Clone)]
pub struct BonusHunter {
    bonuses: Arc<DashMap<String, BonusInfo>>,
    #[allow(dead_code)]
    config: Arc<RwLock<BonusConfig>>,
    planner: Arc<parking_lot::Mutex<BonusPlanner>>,
}

impl BonusHunter {
    pub fn new(config: BonusConfig) -> Self {
        Self {
            bonuses: Arc::new(DashMap::new()),
            config: Arc::new(RwLock::new(config)),
            planner: Arc::new(parking_lot::Mutex::new(BonusPlanner::new())),
        }
    }

    pub fn add_bonus(&self, bonus: BonusInfo) {
        self.bonuses.insert(bonus.id.to_string(), bonus);
    }

    pub fn register_bonus(
        &self,
        bookmaker: &str,
        name: &str,
        bonus_type: BonusType,
        amount: f64,
        wager: f64,
        min_odds: f64,
        max_bet: f64,
        expiry_days: u32,
        bookmaker_margin: f64,
    ) -> BonusInfo {
        let difficulty = BonusCalculator::assess_difficulty(wager, min_odds, expiry_days, max_bet, amount);
        let real_value = BonusCalculator::calculate_real_value(amount, wager, min_odds, bookmaker_margin);
        let ev = BonusCalculator::calculate_ev(amount, wager, min_odds, bookmaker_margin, max_bet);

        let bonus = BonusInfo {
            id: uuid::Uuid::new_v4(),
            bookmaker: bookmaker.to_string(),
            bonus_type,
            name: name.to_string(),
            amount,
            currency: "RUB".into(),
            wager_requirement: wager,
            min_odds,
            max_bet,
            expiry_days,
            real_value,
            ev,
            difficulty,
            status: BonusStatus::Available,
            wager_progress: 0.0,
            detected_at: Utc::now(),
            url: None,
        };

        self.bonuses.insert(bonus.id.to_string(), bonus.clone());
        bonus
    }

    pub fn get_best_bonuses(&self, limit: usize) -> Vec<BonusInfo> {
        let mut bonuses: Vec<BonusInfo> = self.bonuses.iter()
            .filter(|e| matches!(e.value().status, BonusStatus::Available | BonusStatus::Claimed | BonusStatus::Wagering))
            .map(|e| e.value().clone())
            .collect();

        bonuses.sort_by(|a, b| b.ev.partial_cmp(&a.ev).unwrap_or(std::cmp::Ordering::Equal));
        bonuses.truncate(limit);
        bonuses
    }

    pub fn get_bonus_plan(&self, bookmaker: &str) -> Option<shared::BonusPlan> {
        let planner = self.planner.lock();
        planner.get_plan(bookmaker).cloned()
    }

    pub fn create_bonus_plan(&self, bookmaker: &str) -> Option<shared::BonusPlan> {
        let bonus = self.bonuses.iter()
            .find(|e| e.value().bookmaker == bookmaker)
            .map(|e| e.value().clone())?;

        let mut planner = self.planner.lock();
        Some(planner.create_plan(&bonus))
    }

    pub fn update_wager_progress(&self, bookmaker: &str, wager_done: f64) {
        let mut planner = self.planner.lock();
        planner.update_progress(bookmaker, wager_done);
    }

    pub fn get_all_active(&self) -> Vec<BonusInfo> {
        self.bonuses.iter()
            .filter(|e| matches!(e.value().status, BonusStatus::Available | BonusStatus::Claimed | BonusStatus::Wagering))
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn get_completed(&self) -> Vec<BonusInfo> {
        self.bonuses.iter()
            .filter(|e| matches!(e.value().status, BonusStatus::Completed))
            .map(|e| e.value().clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_bonus() {
        let hunter = BonusHunter::new(BonusConfig::default());
        let bonus = hunter.register_bonus(
            "winline", "Welcome Bonus", BonusType::Welcome,
            5000.0, 5.0, 1.8, 5000.0, 30, 5.0,
        );
        assert!(bonus.ev > 0.0);
        assert!(bonus.real_value > 0.0);
    }

    #[test]
    fn test_get_best_bonuses() {
        let hunter = BonusHunter::new(BonusConfig::default());
        hunter.register_bonus("bk1", "Bonus 1", BonusType::Welcome, 5000.0, 5.0, 1.8, 5000.0, 30, 5.0);
        hunter.register_bonus("bk2", "Bonus 2", BonusType::Welcome, 3000.0, 3.0, 1.5, 3000.0, 14, 4.0);
        let best = hunter.get_best_bonuses(1);
        assert_eq!(best.len(), 1);
    }
}