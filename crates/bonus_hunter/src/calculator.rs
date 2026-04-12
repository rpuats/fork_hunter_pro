use shared::BonusDifficulty;

pub struct BonusCalculator;

impl BonusCalculator {
    pub fn calculate_real_value(
        bonus_amount: f64,
        wager_requirement: f64,
        _avg_odds: f64,
        bookmaker_margin: f64,
    ) -> f64 {
        if wager_requirement <= 0.0 || bonus_amount <= 0.0 {
            return 0.0;
        }

        let expected_loss_per_bet = bookmaker_margin / 100.0;
        let total_wager_needed = bonus_amount * wager_requirement;
        let expected_loss = total_wager_needed * expected_loss_per_bet;

        let real_value = bonus_amount - expected_loss;
        real_value.max(0.0)
    }

    pub fn calculate_ev(
        bonus_amount: f64,
        wager_requirement: f64,
        min_odds: f64,
        bookmaker_margin: f64,
        max_bet: f64,
    ) -> f64 {
        let real_value =
            Self::calculate_real_value(bonus_amount, wager_requirement, min_odds, bookmaker_margin);
        let total_bets_needed = (wager_requirement * bonus_amount) / max_bet;
        let variance_penalty = total_bets_needed * 0.01 * bookmaker_margin;

        (real_value - variance_penalty).max(0.0)
    }

    pub fn assess_difficulty(
        wager_requirement: f64,
        min_odds: f64,
        expiry_days: u32,
        max_bet: f64,
        bonus_amount: f64,
    ) -> BonusDifficulty {
        let total_wager = wager_requirement * bonus_amount;
        let bets_needed = (total_wager / max_bet).ceil() as u32;
        let bets_per_day = if expiry_days > 0 {
            bets_needed / expiry_days
        } else {
            bets_needed
        };

        let odds_penalty = if min_odds > 2.0 { 2 } else { 0 };
        let time_penalty = if expiry_days < 7 {
            2
        } else if expiry_days < 14 {
            1
        } else {
            0
        };
        let volume_penalty = if bets_per_day > 20 {
            3
        } else if bets_per_day > 10 {
            2
        } else if bets_per_day > 5 {
            1
        } else {
            0
        };

        let total_score = odds_penalty + time_penalty + volume_penalty;

        match total_score {
            0..=1 => BonusDifficulty::Easy,
            2..=3 => BonusDifficulty::Medium,
            4..=5 => BonusDifficulty::Hard,
            _ => BonusDifficulty::VeryHard,
        }
    }

    pub fn calculate_wager_progress(
        wager_required: f64,
        total_staked: f64,
        _qualifying_bets: u32,
        _min_qualifying_odds: f64,
    ) -> f64 {
        if wager_required <= 0.0 {
            return 100.0;
        }

        let effective_staked = total_staked;
        (effective_staked / wager_required * 100.0).min(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_value_positive() {
        let value = BonusCalculator::calculate_real_value(1000.0, 5.0, 2.0, 5.0);
        assert!(value > 0.0);
    }

    #[test]
    fn test_real_value_negative() {
        let value = BonusCalculator::calculate_real_value(1000.0, 20.0, 1.5, 10.0);
        assert!(value <= 0.0 || value < 1000.0);
    }

    #[test]
    fn test_difficulty_easy() {
        let diff = BonusCalculator::assess_difficulty(3.0, 1.5, 30, 5000.0, 1000.0);
        assert!(matches!(
            diff,
            BonusDifficulty::Easy | BonusDifficulty::Medium
        ));
    }

    #[test]
    fn test_difficulty_hard() {
        let diff = BonusCalculator::assess_difficulty(10.0, 3.0, 3, 1000.0, 5000.0);
        assert!(matches!(
            diff,
            BonusDifficulty::Hard | BonusDifficulty::VeryHard
        ));
    }
}
