use crate::normalizer::Normalizer;
use chrono::Utc;
use dashmap::DashMap;
use shared::{Event, Odd, OddsError};
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
        self.detect_errors_with_context(&[], &HashMap::new(), all_odds, false)
    }

    pub fn detect_event_aware_errors(&self, events: &[Event], all_odds: &[Odd]) -> Vec<OddsError> {
        let event_fingerprints = self.build_event_fingerprints(events);
        self.detect_errors_with_context(events, &event_fingerprints, all_odds, true)
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

    fn detect_errors_with_context(
        &self,
        events: &[Event],
        event_fingerprints: &HashMap<String, String>,
        all_odds: &[Odd],
        use_event_scope: bool,
    ) -> Vec<OddsError> {
        let mut errors = Vec::new();
        let events_by_id: HashMap<String, Event> = events
            .iter()
            .cloned()
            .map(|event| (event.id.clone(), event))
            .collect();
        let by_selection = self.group_by_selection(all_odds, event_fingerprints, use_event_scope);

        for odds in by_selection.values() {
            let unique_bookmakers = odds
                .iter()
                .map(|odd| odd.bookmaker_slug.as_str())
                .collect::<std::collections::HashSet<_>>();
            if unique_bookmakers.len() < self.min_samples {
                continue;
            }

            let baseline = median(odds.iter().map(|odd| odd.odds).collect());
            if baseline <= 0.0 {
                continue;
            }

            for odd in odds {
                let deviation = ((odd.odds - baseline).abs() / baseline) * 100.0;
                if deviation <= self.deviation_threshold {
                    continue;
                }

                let avg_market_odds = self
                    .get_market_average(&self.history_key(odd, event_fingerprints, use_event_scope))
                    .unwrap_or(baseline);
                let event = events_by_id
                    .get(&odd.event_id)
                    .cloned()
                    .unwrap_or_else(|| Event {
                        id: odd.event_id.clone(),
                        sport: shared::Sport::Football,
                        league: String::new(),
                        home_team: String::new(),
                        away_team: String::new(),
                        start_time: None,
                        is_live: false,
                        bookmaker_slug: odd.bookmaker_slug.clone(),
                        raw_url: None,
                        extra: Default::default(),
                    });

                errors.push(OddsError {
                    id: Uuid::new_v4(),
                    bookmaker: odd.bookmaker_slug.clone(),
                    event,
                    market: odd.market.clone(),
                    selection: odd.selection.clone(),
                    suspicious_odds: odd.odds,
                    avg_market_odds,
                    deviation_percent: deviation,
                    detected_at: Utc::now(),
                });
            }

            for odd in odds {
                self.record_odd(
                    &self.history_key(odd, event_fingerprints, use_event_scope),
                    odd.odds,
                );
            }
        }

        errors.sort_by(|a, b| {
            b.deviation_percent
                .partial_cmp(&a.deviation_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        errors
    }

    fn group_by_selection<'a>(
        &self,
        all_odds: &'a [Odd],
        event_fingerprints: &HashMap<String, String>,
        use_event_scope: bool,
    ) -> HashMap<String, Vec<&'a Odd>> {
        let mut map: HashMap<String, Vec<&'a Odd>> = HashMap::new();
        for odd in all_odds {
            let key = self.history_key(odd, event_fingerprints, use_event_scope);
            map.entry(key).or_insert_with(Vec::new).push(odd);
        }
        map
    }

    fn build_event_fingerprints(&self, events: &[Event]) -> HashMap<String, String> {
        events
            .iter()
            .map(|event| (event.id.clone(), Self::event_fingerprint(event)))
            .collect()
    }

    fn history_key(
        &self,
        odd: &Odd,
        event_fingerprints: &HashMap<String, String>,
        use_event_scope: bool,
    ) -> String {
        let event_scope = if use_event_scope {
            event_fingerprints
                .get(&odd.event_id)
                .cloned()
                .unwrap_or_else(|| odd.event_id.clone())
        } else {
            "global".into()
        };

        format!(
            "{}|{}|{}|{}",
            event_scope,
            odd.market,
            odd.selection,
            odd.line
                .map(|line| line.to_string())
                .unwrap_or_else(|| "none".into())
        )
    }

    fn event_fingerprint(event: &Event) -> String {
        let norm = Normalizer::new();
        let norm_event = norm.normalize_event(event.clone());
        let home = Self::normalize_team_name(&norm_event.home_team);
        let away = Self::normalize_team_name(&norm_event.away_team);
        let league = norm_event.league.to_lowercase().replace(' ', "");
        let live_state = if norm_event.is_live {
            "live"
        } else {
            "prematch"
        };
        let (first, second) = if home < away {
            (home, away)
        } else {
            (away, home)
        };

        format!(
            "{:?}|{}|{}|{}|{}",
            event.sport, live_state, league, first, second
        )
    }

    fn normalize_team_name(name: &str) -> String {
        name.to_lowercase()
            .replace("фк ", "")
            .replace("ск ", "")
            .replace("пк ", "")
            .replace("фк", "")
            .replace("ск", "")
            .replace("пк", "")
            .replace("хк ", "")
            .replace("хк", "")
            .replace(" москва", "")
            .replace(" спб", "")
            .replace(" санкт-петербург", "")
            .replace(" с.-петербург", "")
            .replace(' ', "")
            .replace('-', "")
    }
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;
    use shared::Sport;
    use std::collections::HashMap;

    fn make_event(id: &str, bookmaker: &str, home_team: &str, away_team: &str) -> Event {
        Event {
            id: id.into(),
            sport: Sport::Football,
            league: "Premier League".into(),
            home_team: home_team.into(),
            away_team: away_team.into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: bookmaker.into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

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
    fn test_detect_event_aware_errors_groups_same_match_across_event_ids() {
        let detector = OddsErrorDetector::new(40.0, 3);
        let events = vec![
            make_event("pari-evt", "pari", "Arsenal", "Chelsea"),
            make_event("fonbet-evt", "fonbet", "Chelsea", "Arsenal"),
            make_event("marathon-evt", "marathon", "Arsenal", "Chelsea"),
        ];
        let odds = vec![
            Odd {
                event_id: "pari-evt".into(),
                bookmaker_slug: "pari".into(),
                ..make_odd("pari", "1", 10.0)
            },
            Odd {
                event_id: "fonbet-evt".into(),
                bookmaker_slug: "fonbet".into(),
                ..make_odd("fonbet", "1", 2.1)
            },
            Odd {
                event_id: "marathon-evt".into(),
                bookmaker_slug: "marathon".into(),
                ..make_odd("marathon", "1", 2.0)
            },
        ];

        let errors = detector.detect_event_aware_errors(&events, &odds);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].bookmaker, "pari");
        assert_eq!(errors[0].event.id, "pari-evt");
        assert_eq!(errors[0].event.home_team, "Arsenal");
        assert!(errors[0].deviation_percent > 100.0);
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
