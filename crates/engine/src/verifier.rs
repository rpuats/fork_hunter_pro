use chrono::{DateTime, Utc};
use moka::future::Cache;
use shared::odds::calculate_surebet_profit;
use shared::{Odd, Surebet};
use std::time::Duration;
use tracing::debug;

#[derive(Clone)]
pub struct OddsVerifier {
    cache: Cache<String, VerificationResult>,
    #[allow(dead_code)]
    max_retries: u32,
}

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub surebet_id: String,
    pub verified: bool,
    pub profit_before: f64,
    pub profit_after: Option<f64>,
    pub changed_legs: Vec<String>,
    pub verified_at: DateTime<Utc>,
}

impl OddsVerifier {
    pub fn new(max_retries: u32, _timeout_secs: u64, ttl_secs: u64) -> Self {
        Self {
            cache: Cache::builder()
                .time_to_live(Duration::from_secs(ttl_secs))
                .max_capacity(5000)
                .build(),
            max_retries,
        }
    }

    pub async fn verify_surebet(&self, surebet: &Surebet, all_odds: &[Odd]) -> VerificationResult {
        let cache_key = format!("verify-{}", surebet.id);

        if let Some(cached) = self.cache.get(&cache_key).await {
            debug!(cache_key, "Using cached verification result");
            return cached;
        }

        let mut changed_legs = Vec::new();
        let mut verified = true;

        for leg in &surebet.legs {
            let current_odds = all_odds.iter().find(|o| {
                o.bookmaker_slug == leg.bookmaker
                    && o.market == leg.market
                    && o.selection == leg.selection
            });

            if let Some(current) = current_odds {
                if (current.odds - leg.odds).abs() > 0.05 {
                    changed_legs.push(format!(
                        "{}: {} -> {}",
                        leg.bookmaker, leg.odds, current.odds
                    ));
                    verified = false;
                }
            } else {
                changed_legs.push(format!("{}: odds not found", leg.bookmaker));
                verified = false;
            }
        }

        let profit_after = if verified {
            let current_odds: Vec<f64> = surebet.legs.iter().map(|l| l.odds).collect();
            calculate_surebet_profit(&current_odds)
        } else {
            None
        };

        let result = VerificationResult {
            surebet_id: surebet.id.to_string(),
            verified,
            profit_before: surebet.profit_percent,
            profit_after,
            changed_legs,
            verified_at: Utc::now(),
        };

        self.cache.insert(cache_key, result.clone()).await;
        result
    }

    pub async fn batch_verify(&self, surebets: &[Surebet], all_odds: &[Odd]) -> Vec<VerificationResult> {
        let mut results = Vec::new();
        for surebet in surebets {
            let result = self.verify_surebet(surebet, all_odds).await;
            results.push(result);
        }
        results
    }

    pub fn cache_stats(&self) -> (u64, u64) {
        (self.cache.entry_count(), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;
    use shared::{Sport, SurebetLeg};
    use uuid::Uuid;

    fn make_odd(bk: &str, sel: &str, odds: f64) -> Odd {
        Odd {
            id: format!("{}-{}", bk, sel),
            event_id: "evt1".into(),
            bookmaker_slug: bk.into(),
            market: "1X2".into(),
            selection: sel.into(),
            odds,
            odds_type: OddsType::Home,
            line: None,
            timestamp: Utc::now(),
        }
    }

    fn make_surebet(legs: Vec<SurebetLeg>) -> Surebet {
        Surebet {
            id: Uuid::new_v4(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: false,
            profit_percent: 2.0,
            total_stake: 1000.0,
            legs,
            detected_at: Utc::now(),
            verified: false,
            mirror: false,
        }
    }

    #[tokio::test]
    async fn test_verify_valid_surebet() {
        let verifier = OddsVerifier::new(3, 10, 60);
        let legs = vec![
            SurebetLeg { bookmaker: "bk1".into(), market: "1X2".into(), selection: "1".into(), odds: 2.10, line: None, stake: 500.0, payout: 1050.0, url: None },
            SurebetLeg { bookmaker: "bk2".into(), market: "1X2".into(), selection: "2".into(), odds: 2.10, line: None, stake: 500.0, payout: 1050.0, url: None },
        ];
        let surebet = make_surebet(legs);
        let all_odds = vec![make_odd("bk1", "1", 2.10), make_odd("bk2", "2", 2.10)];
        let result = verifier.verify_surebet(&surebet, &all_odds).await;
        assert!(result.verified);
        assert!(result.changed_legs.is_empty());
    }

    #[tokio::test]
    async fn test_verify_changed_odds() {
        let verifier = OddsVerifier::new(3, 10, 60);
        let legs = vec![
            SurebetLeg { bookmaker: "bk1".into(), market: "1X2".into(), selection: "1".into(), odds: 2.10, line: None, stake: 500.0, payout: 1050.0, url: None },
            SurebetLeg { bookmaker: "bk2".into(), market: "1X2".into(), selection: "2".into(), odds: 2.10, line: None, stake: 500.0, payout: 1050.0, url: None },
        ];
        let surebet = make_surebet(legs);
        let all_odds = vec![make_odd("bk1", "1", 1.80), make_odd("bk2", "2", 2.10)];
        let result = verifier.verify_surebet(&surebet, &all_odds).await;
        assert!(!result.verified);
        assert!(!result.changed_legs.is_empty());
    }
}
