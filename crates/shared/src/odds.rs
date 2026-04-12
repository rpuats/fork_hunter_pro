use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Display)]
pub enum OddsType {
    #[strum(serialize = "1")]
    Home,
    #[strum(serialize = "X")]
    Draw,
    #[strum(serialize = "2")]
    Away,
    #[strum(serialize = "over")]
    Over,
    #[strum(serialize = "under")]
    Under,
    #[strum(serialize = "handicap")]
    Handicap,
    #[strum(serialize = "asian_handicap")]
    AsianHandicap,
    #[strum(serialize = "both_teams_score_yes")]
    BothTeamsScoreYes,
    #[strum(serialize = "both_teams_score_no")]
    BothTeamsScoreNo,
    #[strum(serialize = "total")]
    Total,
    #[strum(serialize = "individual_total")]
    IndividualTotal,
    #[strum(serialize = "even")]
    Even,
    #[strum(serialize = "odd")]
    Odd,
    #[strum(serialize = "custom")]
    Custom,
}

impl OddsType {
    pub fn is_two_way(&self) -> bool {
        matches!(
            self,
            OddsType::Over
                | OddsType::Under
                | OddsType::Handicap
                | OddsType::AsianHandicap
                | OddsType::BothTeamsScoreYes
                | OddsType::BothTeamsScoreNo
                | OddsType::Even
                | OddsType::Odd
        )
    }

    pub fn is_three_way(&self) -> bool {
        matches!(self, OddsType::Home | OddsType::Draw | OddsType::Away)
    }

    pub fn opposite(&self) -> Option<Self> {
        match self {
            OddsType::Home => Some(OddsType::Away),
            OddsType::Away => Some(OddsType::Home),
            OddsType::Over => Some(OddsType::Under),
            OddsType::Under => Some(OddsType::Over),
            OddsType::BothTeamsScoreYes => Some(OddsType::BothTeamsScoreNo),
            OddsType::BothTeamsScoreNo => Some(OddsType::BothTeamsScoreYes),
            _ => None,
        }
    }
}

pub fn decimal_to_implied_probability(odds: f64) -> f64 {
    if odds <= 1.0 {
        return 0.0;
    }
    1.0 / odds
}

pub fn calculate_margin(odds: &[f64]) -> f64 {
    if odds.is_empty() {
        return 0.0;
    }
    let sum: f64 = odds
        .iter()
        .map(|&o| decimal_to_implied_probability(o))
        .sum();
    (sum - 1.0) * 100.0
}

pub fn calculate_surebet_profit(odds: &[f64]) -> Option<f64> {
    if odds.len() < 2 {
        return None;
    }
    let inverse_sum: f64 = odds
        .iter()
        .map(|&o| decimal_to_implied_probability(o))
        .sum();
    if inverse_sum < 1.0 {
        Some((1.0 - inverse_sum) * 100.0)
    } else {
        None
    }
}

pub fn calculate_stakes(odds: &[f64], total_stake: f64) -> Vec<f64> {
    let inverse_sum: f64 = odds
        .iter()
        .map(|&o| decimal_to_implied_probability(o))
        .sum();
    if inverse_sum == 0.0 {
        return vec![total_stake / odds.len() as f64; odds.len()];
    }
    odds.iter()
        .map(|&o| (total_stake * decimal_to_implied_probability(o)) / inverse_sum)
        .collect()
}

pub fn calculate_payout(stake: f64, odds: f64) -> f64 {
    stake * odds
}
