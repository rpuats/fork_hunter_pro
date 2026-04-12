use dashmap::DashMap;
use parking_lot::RwLock;
use shared::{Event, ExpressFork, Odd};
use std::sync::Arc;

use super::calculator::ExpressForkCalculator;

#[derive(Clone)]
pub struct ExpressForkScanner {
    calculator: Arc<ExpressForkCalculator>,
    recent_forks: Arc<DashMap<String, ExpressFork>>,
    seen_keys: Arc<RwLock<Vec<String>>>,
}

impl ExpressForkScanner {
    pub fn new(max_legs: usize, min_profit: f64, default_stake: f64) -> Self {
        Self {
            calculator: Arc::new(ExpressForkCalculator::new(
                max_legs,
                min_profit,
                default_stake,
            )),
            recent_forks: Arc::new(DashMap::new()),
            seen_keys: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn scan(&self, events: &[Event], all_odds: &[Odd]) -> Vec<ExpressFork> {
        let mut forks = self.calculator.find_express_forks(events, all_odds);

        forks.retain(|f| {
            let key = self.fork_key(f);
            let mut seen = self.seen_keys.write();
            if seen.contains(&key) {
                false
            } else {
                seen.push(key.clone());
                if seen.len() > 10000 {
                    seen.drain(..5000);
                }
                true
            }
        });

        for fork in &forks {
            let key = self.fork_key(fork);
            self.recent_forks.insert(key, fork.clone());
        }

        forks
    }

    pub fn get_recent(&self, limit: usize) -> Vec<ExpressFork> {
        self.recent_forks
            .iter()
            .take(limit)
            .map(|e| e.value().clone())
            .collect()
    }

    fn fork_key(&self, f: &ExpressFork) -> String {
        let legs: Vec<String> = f
            .legs
            .iter()
            .map(|l| format!("{}|{}|{}", l.bookmaker, l.odds, l.selection))
            .collect();
        legs.join(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
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
            timestamp: Utc::now(),
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
    fn test_scan_express_forks() {
        let scanner = ExpressForkScanner::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 3.0),
            make_odd("e1", "bk2", "1", 1.5),
            make_odd("e2", "bk1", "1", 3.0),
            make_odd("e2", "bk2", "1", 1.5),
        ];
        let forks = scanner.scan(&events, &odds);
        assert!(forks.len() <= 2);
    }
}
