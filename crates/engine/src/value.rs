use chrono::Utc;
use shared::odds::decimal_to_implied_probability;
use shared::{Event, Odd, ValueBet};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct ValueDetector {
    min_edge: f64,
}

impl ValueDetector {
    pub fn new(min_edge: f64) -> Self {
        Self { min_edge }
    }

    pub fn detect_values(&self, events: &[Event], all_odds: &[Odd]) -> Vec<ValueBet> {
        let mut values = Vec::new();
        let market_averages = self.calculate_market_averages(all_odds);

        for odd in all_odds {
            let key = format!("{}|{}|{}", odd.market, odd.selection, odd.line.map(|l| l.to_string()).unwrap_or_else(|| "none".into()));

            if let Some(&avg_odds) = market_averages.get(&key) {
                let implied = decimal_to_implied_probability(odd.odds);
                let fair_implied = decimal_to_implied_probability(avg_odds);

                if implied < fair_implied {
                    let edge = ((fair_implied - implied) / fair_implied) * 100.0;
                    if edge >= self.min_edge {
                        let event = events.iter().find(|e| e.id == odd.event_id);
                        if let Some(event) = event {
                            values.push(ValueBet {
                                id: Uuid::new_v4(),
                                bookmaker: odd.bookmaker_slug.clone(),
                                event: event.clone(),
                                market: odd.market.clone(),
                                selection: odd.selection.clone(),
                                odds: odd.odds,
                                fair_odds: avg_odds,
                                edge_percent: edge,
                                detected_at: Utc::now(),
                            });
                        }
                    }
                }
            }
        }

        values.sort_by(|a, b| b.edge_percent.partial_cmp(&a.edge_percent).unwrap());
        values
    }

    fn calculate_market_averages(&self, all_odds: &[Odd]) -> HashMap<String, f64> {
        let mut groups: HashMap<String, Vec<f64>> = HashMap::new();

        for odd in all_odds {
            let key = format!("{}|{}|{}", odd.market, odd.selection, odd.line.map(|l| l.to_string()).unwrap_or_else(|| "none".into()));
            groups.entry(key).or_default().push(odd.odds);
        }

        groups
            .into_iter()
            .map(|(key, odds)| {
                let avg = odds.iter().sum::<f64>() / odds.len() as f64;
                (key, avg)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;
    use shared::Sport;
    use std::collections::HashMap;

    fn make_event(id: &str) -> Event {
        Event {
            id: id.into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

    fn make_odd(event_id: &str, bk: &str, sel: &str, odds: f64) -> Odd {
        Odd {
            id: format!("{}-{}-{}", event_id, bk, sel),
            event_id: event_id.into(),
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
    fn test_detect_value_bet() {
        let detector = ValueDetector::new(5.0);
        let event = make_event("evt1");
        let odds = vec![
            make_odd("evt1", "bk1", "1", 2.50),
            make_odd("evt1", "bk2", "1", 2.00),
            make_odd("evt1", "bk3", "1", 2.00),
        ];

        let values = detector.detect_values(&[event], &odds);
        assert!(!values.is_empty());
        assert!(values[0].edge_percent >= 5.0);
    }
}
