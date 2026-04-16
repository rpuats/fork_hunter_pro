use chrono::{DateTime, Utc};
use moka::future::Cache;
use shared::odds::calculate_surebet_profit;
use shared::{Odd, Surebet, SurebetLeg};
use std::time::Duration;
use tracing::debug;

const ODDS_CHANGE_TOLERANCE: f64 = 0.05;
const LINE_MATCH_TOLERANCE: f64 = 0.05;
const MAX_ODDS_AGE_SECS: i64 = 90;

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
        let mut current_odds = Vec::with_capacity(surebet.legs.len());

        for leg in &surebet.legs {
            let matching_odds = self.find_matching_odd(leg, all_odds);

            if let Some(current) = matching_odds {
                current_odds.push(current.odds);

                if (current.odds - leg.odds).abs() > ODDS_CHANGE_TOLERANCE {
                    changed_legs.push(format!(
                        "{}: {} -> {}",
                        leg.bookmaker, leg.odds, current.odds
                    ));
                    verified = false;
                }
            } else if all_odds.iter().any(|odd| Self::same_market_key(leg, odd)) {
                changed_legs.push(format!(
                    "{}: matching odds are stale (>{}s)",
                    leg.bookmaker, MAX_ODDS_AGE_SECS
                ));
                verified = false;
            } else {
                changed_legs.push(format!("{}: odds not found", leg.bookmaker));
                verified = false;
            }
        }

        let profit_after = if current_odds.len() == surebet.legs.len() {
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

    fn find_matching_odd<'a>(&self, leg: &SurebetLeg, all_odds: &'a [Odd]) -> Option<&'a Odd> {
        all_odds
            .iter()
            .filter(|odd| Self::same_market_key(leg, odd) && Self::line_matches(leg.line, odd.line))
            .filter(|odd| Self::is_fresh(odd.timestamp))
            .max_by(|left, right| {
                left.timestamp.cmp(&right.timestamp).then_with(|| {
                    (left.odds - leg.odds)
                        .abs()
                        .partial_cmp(&(right.odds - leg.odds).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .reverse()
                })
            })
    }

    fn same_market_key(leg: &SurebetLeg, odd: &Odd) -> bool {
        odd.bookmaker_slug == leg.bookmaker
            && odd.market.eq_ignore_ascii_case(&leg.market)
            && odd.selection.eq_ignore_ascii_case(&leg.selection)
    }

    fn line_matches(expected: Option<f64>, actual: Option<f64>) -> bool {
        match (expected, actual) {
            (Some(left), Some(right)) => (left - right).abs() <= LINE_MATCH_TOLERANCE,
            (None, None) => true,
            _ => false,
        }
    }

    fn is_fresh(timestamp: DateTime<Utc>) -> bool {
        Utc::now().signed_duration_since(timestamp).num_seconds() <= MAX_ODDS_AGE_SECS
    }

    pub async fn batch_verify(
        &self,
        surebets: &[Surebet],
        all_odds: &[Odd],
    ) -> Vec<VerificationResult> {
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

    fn make_total_odd(bk: &str, sel: &str, odds: f64, line: Option<f64>, age_secs: i64) -> Odd {
        Odd {
            id: format!("{}-{}-{:?}", bk, sel, line),
            event_id: "evt1".into(),
            bookmaker_slug: bk.into(),
            market: "Total".into(),
            selection: sel.into(),
            odds,
            odds_type: if sel.eq_ignore_ascii_case("over") {
                OddsType::Over
            } else {
                OddsType::Under
            },
            line,
            timestamp: Utc::now() - chrono::Duration::seconds(age_secs),
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
            SurebetLeg {
                bookmaker: "bk1".into(),
                market: "1X2".into(),
                selection: "1".into(),
                odds: 2.10,
                line: None,
                stake: 500.0,
                payout: 1050.0,
                url: None,
            },
            SurebetLeg {
                bookmaker: "bk2".into(),
                market: "1X2".into(),
                selection: "2".into(),
                odds: 2.10,
                line: None,
                stake: 500.0,
                payout: 1050.0,
                url: None,
            },
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
            SurebetLeg {
                bookmaker: "bk1".into(),
                market: "1X2".into(),
                selection: "1".into(),
                odds: 2.10,
                line: None,
                stake: 500.0,
                payout: 1050.0,
                url: None,
            },
            SurebetLeg {
                bookmaker: "bk2".into(),
                market: "1X2".into(),
                selection: "2".into(),
                odds: 2.10,
                line: None,
                stake: 500.0,
                payout: 1050.0,
                url: None,
            },
        ];
        let surebet = make_surebet(legs);
        let all_odds = vec![make_odd("bk1", "1", 1.80), make_odd("bk2", "2", 2.10)];
        let result = verifier.verify_surebet(&surebet, &all_odds).await;
        assert!(!result.verified);
        assert!(!result.changed_legs.is_empty());
        assert_eq!(
            result.profit_after,
            shared::odds::calculate_surebet_profit(&[1.80, 2.10])
        );
    }

    #[tokio::test]
    async fn test_verify_matches_same_line_only() {
        let verifier = OddsVerifier::new(3, 10, 60);
        let surebet = make_surebet(vec![
            SurebetLeg {
                bookmaker: "bk1".into(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: 2.05,
                line: Some(2.5),
                stake: 500.0,
                payout: 1025.0,
                url: None,
            },
            SurebetLeg {
                bookmaker: "bk2".into(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: 2.05,
                line: Some(2.5),
                stake: 500.0,
                payout: 1025.0,
                url: None,
            },
        ]);
        let all_odds = vec![
            make_total_odd("bk1", "Over", 2.05, Some(3.5), 0),
            make_total_odd("bk1", "Over", 2.05, Some(2.5), 0),
            make_total_odd("bk2", "Under", 2.05, Some(2.5), 0),
        ];

        let result = verifier.verify_surebet(&surebet, &all_odds).await;

        assert!(result.verified);
        assert!(result.changed_legs.is_empty());
    }

    #[tokio::test]
    async fn test_verify_rejects_stale_odds() {
        let verifier = OddsVerifier::new(3, 10, 60);
        let surebet = make_surebet(vec![
            SurebetLeg {
                bookmaker: "bk1".into(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: 2.05,
                line: Some(2.5),
                stake: 500.0,
                payout: 1025.0,
                url: None,
            },
            SurebetLeg {
                bookmaker: "bk2".into(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: 2.05,
                line: Some(2.5),
                stake: 500.0,
                payout: 1025.0,
                url: None,
            },
        ]);
        let all_odds = vec![
            make_total_odd("bk1", "Over", 2.05, Some(2.5), 120),
            make_total_odd("bk2", "Under", 2.05, Some(2.5), 0),
        ];

        let result = verifier.verify_surebet(&surebet, &all_odds).await;

        assert!(!result.verified);
        assert!(result
            .changed_legs
            .iter()
            .any(|entry| entry.contains("stale")));
    }
}
