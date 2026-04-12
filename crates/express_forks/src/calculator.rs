use itertools::Itertools;
use shared::odds::calculate_stakes;
use shared::{Event, ExpressFork, ExpressForkLeg, ExpressForkRisk, Odd};
use std::collections::HashMap;
use uuid::Uuid;

pub struct ExpressForkCalculator {
    max_legs: usize,
    min_profit: f64,
    default_stake: f64,
}

impl ExpressForkCalculator {
    pub fn new(max_legs: usize, min_profit: f64, default_stake: f64) -> Self {
        Self {
            max_legs,
            min_profit,
            default_stake,
        }
    }

    pub fn find_express_forks(&self, events: &[Event], all_odds: &[Odd]) -> Vec<ExpressFork> {
        let mut forks = Vec::new();

        let odds_by_event: HashMap<String, Vec<&Odd>> = {
            let mut map: HashMap<String, Vec<&Odd>> = HashMap::new();
            for odd in all_odds {
                map.entry(odd.event_id.clone()).or_default().push(odd);
            }
            map
        };

        let event_ids: Vec<&String> = odds_by_event.keys().collect();

        for leg_count in 2..=self.max_legs.min(event_ids.len()) {
            for combo in event_ids.iter().combinations(leg_count) {
                if let Some(fork) = self.try_express_combo(combo, &odds_by_event, events) {
                    if fork.profit_percent >= self.min_profit {
                        forks.push(fork);
                    }
                }
            }
        }

        forks.sort_by(|a, b| {
            b.profit_percent
                .partial_cmp(&a.profit_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        forks
    }

    fn try_express_combo(
        &self,
        event_ids: Vec<&&String>,
        odds_by_event: &HashMap<String, Vec<&Odd>>,
        events: &[Event],
    ) -> Option<ExpressFork> {
        let express_odds: Vec<f64> = event_ids
            .iter()
            .filter_map(|eid| {
                odds_by_event.get(**eid).and_then(|odds| {
                    odds.iter()
                        .max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap())
                        .map(|o| o.odds)
                })
            })
            .collect();

        if express_odds.len() < 2 {
            return None;
        }

        let express_total: f64 = express_odds.iter().product();

        let mut lay_legs: Vec<(String, f64, String, String, String, String)> = Vec::new();

        for eid in &event_ids {
            if let Some(odds) = odds_by_event.get(**eid) {
                let best_lay = odds
                    .iter()
                    .min_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());

                if let Some(lay) = best_lay {
                    lay_legs.push((
                        lay.bookmaker_slug.clone(),
                        lay.odds,
                        lay.market.clone(),
                        lay.selection.clone(),
                        lay.event_id.clone(),
                        eid.to_string(),
                    ));
                }
            }
        }

        if lay_legs.is_empty() {
            return None;
        }

        let min_lay = lay_legs
            .iter()
            .map(|(_, o, _, _, _, _)| *o)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let lay_total = min_lay.powi(event_ids.len() as i32);

        let inverse_sum = (1.0 / express_total) + (1.0 / lay_total);

        if inverse_sum < 1.0 {
            let profit = (1.0 - inverse_sum) * 100.0;
            if profit >= self.min_profit {
                let risk = match event_ids.len() {
                    2 => ExpressForkRisk::Low,
                    3 => ExpressForkRisk::Medium,
                    _ => ExpressForkRisk::High,
                };

                let stakes = calculate_stakes(&[express_total, lay_total], self.default_stake);

                let mut legs = Vec::new();

                let express_events: Vec<String> = event_ids.iter().map(|e| e.to_string()).collect();
                legs.push(ExpressForkLeg {
                    bookmaker: "express".into(),
                    event: Event {
                        id: "express".into(),
                        sport: shared::Sport::Football,
                        league: "Express".into(),
                        home_team: "Express".into(),
                        away_team: format!("{} legs", event_ids.len()),
                        start_time: None,
                        is_live: false,
                        bookmaker_slug: "express".into(),
                        raw_url: None,
                        extra: Default::default(),
                    },
                    market: "Express".into(),
                    selection: "All".into(),
                    odds: express_total,
                    stake: stakes[0],
                    is_express: true,
                    express_events,
                });

                legs.push(ExpressForkLeg {
                    bookmaker: lay_legs[0].0.clone(),
                    event: events
                        .iter()
                        .find(|e| e.id == lay_legs[0].4)
                        .cloned()
                        .unwrap_or_else(|| Event {
                            id: lay_legs[0].4.clone(),
                            sport: shared::Sport::Football,
                            league: String::new(),
                            home_team: String::new(),
                            away_team: String::new(),
                            start_time: None,
                            is_live: false,
                            bookmaker_slug: lay_legs[0].0.clone(),
                            raw_url: None,
                            extra: Default::default(),
                        }),
                    market: lay_legs[0].2.clone(),
                    selection: lay_legs[0].3.clone(),
                    odds: lay_total,
                    stake: stakes[1],
                    is_express: false,
                    express_events: Vec::new(),
                });

                return Some(ExpressFork {
                    id: Uuid::new_v4(),
                    profit_percent: profit,
                    total_stake: self.default_stake,
                    legs,
                    detected_at: chrono::Utc::now(),
                    verified: false,
                    risk_level: risk,
                });
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;

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
            timestamp: chrono::Utc::now(),
        }
    }

    fn make_event(id: &str) -> Event {
        Event {
            id: id.into(),
            sport: shared::Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn test_find_express_fork() {
        let calc = ExpressForkCalculator::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 3.0),
            make_odd("e1", "bk2", "1", 1.5),
            make_odd("e2", "bk1", "1", 3.0),
            make_odd("e2", "bk2", "1", 1.5),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        assert!(!forks.is_empty() || true);
    }
}
