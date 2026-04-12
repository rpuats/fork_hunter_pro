use chrono::Utc;
use dashmap::DashMap;
use shared::{Event, Odd, OddsError, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct OddsErrorDetector {
    deviation_threshold: f64,
    min_samples: usize,
    recent_odds: Arc<DashMap<String, Vec<f64>>>,
}

impl OddsErrorDetector {
    pub fn new(deviation_threshold: f64, min_samples: usize) -> Self {
        Self {
            deviation_threshold,
            min_samples,
            recent_odds: Arc::new(DashMap::new()),
        }
    }

    pub fn detect_errors(&self, all_odds: &[Odd]) -> Vec<OddsError> {
        let mut errors = Vec::new();
        let by_selection = self.group_by_selection(all_odds);

        for (_key, odds) in &by_selection {
            if odds.len() < self.min_samples {
                continue;
            }

            let odds_values: Vec<f64> = odds.iter().map(|o| o.odds).collect();
            let avg = odds_values.iter().sum::<f64>() / odds_values.len() as f64;
            let variance: f64 = odds_values.iter().map(|&o| (o - avg).powi(2)).sum::<f64>()
                / odds_values.len() as f64;
            let std_dev = variance.sqrt();

            if std_dev == 0.0 {
                continue;
            }

            for odd in odds {
                let deviation = ((odd.odds - avg).abs() / std_dev) * 100.0;
                if deviation > self.deviation_threshold {
                    let event = Event {
                        id: odd.event_id.clone(),
                        sport: Sport::Football,
                        league: String::new(),
                        home_team: String::new(),
                        away_team: String::new(),
                        start_time: None,
                        is_live: false,
                        bookmaker_slug: odd.bookmaker_slug.clone(),
                        raw_url: None,
                        extra: Default::default(),
                    };

                    errors.push(OddsError {
                        id: Uuid::new_v4(),
                        bookmaker: odd.bookmaker_slug.clone(),
                        event,
                        market: odd.market.clone(),
                        selection: odd.selection.clone(),
                        suspicious_odds: odd.odds,
                        avg_market_odds: avg,
                        deviation_percent: deviation,
                        detected_at: Utc::now(),
                    });
                }
            }
        }

        errors
    }

    pub fn record_odd(&self, key: &str, odds: f64) {
        let mut entries = self.recent_odds.entry(key.to_string()).or_default();
        entries.push(odds);
        let len = entries.len();
        if len > 1000 {
            let drain_to = len - 500;
            entries.drain(..drain_to);
        }
    }

    pub fn get_market_average(&self, key: &str) -> Option<f64> {
        self.recent_odds.get(key).and_then(|entries| {
            if entries.is_empty() {
                None
            } else {
                Some(entries.iter().sum::<f64>() / entries.len() as f64)
            }
        })
    }

    fn group_by_selection<'a>(&self, all_odds: &'a [Odd]) -> HashMap<String, Vec<&'a Odd>> {
        let mut map: HashMap<String, Vec<&'a Odd>> = HashMap::new();
        for odd in all_odds {
            let key = format!(
                "{}|{}|{}",
                odd.market,
                odd.selection,
                odd.line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "none".into())
            );
            map.entry(key).or_insert_with(Vec::new).push(odd);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;

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

    #[test]
    fn test_detect_anomalous_odd() {
        let detector = OddsErrorDetector::new(150.0, 3);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.1),
            make_odd("bk3", "1", 1.9),
            make_odd("bk4", "1", 10.0),
        ];
        let errors = detector.detect_errors(&odds);
        assert!(!errors.is_empty());
        assert!((errors[0].suspicious_odds - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_no_errors_normal_odds() {
        let detector = OddsErrorDetector::new(500.0, 3);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.1),
            make_odd("bk3", "1", 1.9),
            make_odd("bk4", "1", 2.05),
        ];
        let errors = detector.detect_errors(&odds);
        assert!(errors.is_empty());
    }
}
