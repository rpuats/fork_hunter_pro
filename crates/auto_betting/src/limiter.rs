use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub struct BetLimiter {
    max_bets_per_hour: u32,
    max_daily_stake: f64,
    delay_between_bets_ms: u64,
    bets_this_hour: Vec<DateTime<Utc>>,
    daily_stake: f64,
    last_bet_time: Option<Instant>,
}

impl BetLimiter {
    pub fn new(max_bets_per_hour: u32, max_daily_stake: f64, delay_between_bets_ms: u64) -> Self {
        Self {
            max_bets_per_hour,
            max_daily_stake,
            delay_between_bets_ms,
            bets_this_hour: Vec::new(),
            daily_stake: 0.0,
            last_bet_time: None,
        }
    }

    pub fn can_bet(&mut self, stake: f64) -> Result<(), BetLimitError> {
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);

        self.bets_this_hour.retain(|t| *t > one_hour_ago);

        if self.bets_this_hour.len() >= self.max_bets_per_hour as usize {
            return Err(BetLimitError::HourlyLimitReached);
        }

        if self.daily_stake + stake > self.max_daily_stake {
            return Err(BetLimitError::DailyLimitReached);
        }

        if let Some(last) = self.last_bet_time {
            let min_delay = Duration::from_millis(self.delay_between_bets_ms);
            let random_delay =
                min_delay + Duration::from_millis(rand::thread_rng().gen_range(0..2000));
            if last.elapsed() < random_delay {
                return Err(BetLimitError::TooSoon);
            }
        }

        Ok(())
    }

    pub fn record_bet(&mut self, stake: f64) {
        self.bets_this_hour.push(Utc::now());
        self.daily_stake += stake;
        self.last_bet_time = Some(Instant::now());
    }

    pub fn reset_daily(&mut self) {
        self.daily_stake = 0.0;
    }

    pub fn get_stats(&self) -> BetLimiterStats {
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        let bets_this_hour = self
            .bets_this_hour
            .iter()
            .filter(|t| **t > one_hour_ago)
            .count();

        BetLimiterStats {
            bets_this_hour: bets_this_hour as u32,
            max_bets_per_hour: self.max_bets_per_hour,
            daily_stake: self.daily_stake,
            max_daily_stake: self.max_daily_stake,
            remaining_daily: (self.max_daily_stake - self.daily_stake).max(0.0),
        }
    }
}

#[derive(Debug)]
pub enum BetLimitError {
    HourlyLimitReached,
    DailyLimitReached,
    TooSoon,
}

impl std::fmt::Display for BetLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BetLimitError::HourlyLimitReached => write!(f, "Hourly bet limit reached"),
            BetLimitError::DailyLimitReached => write!(f, "Daily stake limit reached"),
            BetLimitError::TooSoon => write!(f, "Too soon since last bet"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetLimiterStats {
    pub bets_this_hour: u32,
    pub max_bets_per_hour: u32,
    pub daily_stake: f64,
    pub max_daily_stake: f64,
    pub remaining_daily: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_bet_within_limits() {
        let mut limiter = BetLimiter::new(10, 10000.0, 0);
        assert!(limiter.can_bet(500.0).is_ok());
    }

    #[test]
    fn test_hourly_limit_reached() {
        let mut limiter = BetLimiter::new(2, 10000.0, 0);
        // Первая ставка
        assert!(limiter.can_bet(100.0).is_ok());
        limiter.record_bet(100.0);
        // Третья — должна быть отклонена после 2 записей
        limiter.record_bet(100.0); // 2-я запись
        assert!(limiter.can_bet(100.0).is_err());
    }

    #[test]
    fn test_daily_limit_reached() {
        let mut limiter = BetLimiter::new(100, 1000.0, 0);
        assert!(limiter.can_bet(600.0).is_ok());
        limiter.record_bet(600.0);
        assert!(limiter.can_bet(500.0).is_err()); // 600+500 > 1000
    }

    #[test]
    fn test_record_bet_updates_stats() {
        let mut limiter = BetLimiter::new(100, 10000.0, 0);
        limiter.record_bet(500.0);
        let stats = limiter.get_stats();
        assert_eq!(stats.bets_this_hour, 1);
        assert_eq!(stats.daily_stake, 500.0);
        assert_eq!(stats.remaining_daily, 9500.0);
    }

    #[test]
    fn test_reset_daily() {
        let mut limiter = BetLimiter::new(100, 10000.0, 0);
        limiter.record_bet(500.0);
        limiter.reset_daily();
        let stats = limiter.get_stats();
        assert_eq!(stats.daily_stake, 0.0);
        assert_eq!(stats.remaining_daily, 10000.0);
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            format!("{}", BetLimitError::HourlyLimitReached),
            "Hourly bet limit reached"
        );
        assert_eq!(
            format!("{}", BetLimitError::DailyLimitReached),
            "Daily stake limit reached"
        );
        assert_eq!(
            format!("{}", BetLimitError::TooSoon),
            "Too soon since last bet"
        );
    }
}
