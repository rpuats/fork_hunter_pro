use dashmap::DashMap;
use parking_lot::RwLock;
use shared::{CorridorOpportunity, Event, Odd};
use std::sync::Arc;

use super::calculator::CorridorCalculator;

#[derive(Clone)]
pub struct CorridorScanner {
    min_corridor_size: f64,
    recent_corridors: Arc<DashMap<String, CorridorOpportunity>>,
    seen_keys: Arc<RwLock<Vec<String>>>,
}

impl CorridorScanner {
    pub fn new(min_corridor_size: f64) -> Self {
        Self {
            min_corridor_size,
            recent_corridors: Arc::new(DashMap::new()),
            seen_keys: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn scan(&self, _events: &[Event], all_odds: &[Odd]) -> Vec<CorridorOpportunity> {
        let mut corridors = CorridorCalculator::find_corridors(all_odds, self.min_corridor_size);

        let ah_corridors = CorridorCalculator::find_asian_handicap_corridors(all_odds, self.min_corridor_size);
        corridors.extend(ah_corridors);

        corridors.retain(|c| {
            let key = self.corridor_key(c);
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

        for corridor in &corridors {
            let key = self.corridor_key(corridor);
            self.recent_corridors.insert(key, corridor.clone());
        }

        corridors
    }

    pub fn get_recent(&self, limit: usize) -> Vec<CorridorOpportunity> {
        self.recent_corridors
            .iter()
            .take(limit)
            .map(|e| e.value().clone())
            .collect()
    }

    fn corridor_key(&self, c: &CorridorOpportunity) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            c.home_team, c.away_team, c.bookmaker_a, c.bookmaker_b, c.line_a, c.line_b
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared::odds::OddsType;

    fn make_odd(event_id: &str, bk: &str, sel: &str, odds: f64, line: f64) -> Odd {
        Odd {
            id: format!("{}-{}-{}", event_id, bk, sel),
            event_id: event_id.into(),
            bookmaker_slug: bk.into(),
            market: "Total".into(),
            selection: sel.into(),
            odds,
            odds_type: OddsType::Over,
            line: Some(line),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_scan_corridors() {
        let scanner = CorridorScanner::new(0.5);
        // Over 3.5 у bk1 и Under 2.5 у bk2 => corridor_size = 3.5 - 2.5 = 1.0
        let odds = vec![
            make_odd("e1", "bk1", "Over", 1.90, 3.5),
            make_odd("e1", "bk2", "Under", 1.90, 2.5),
        ];
        let corridors = scanner.scan(&[], &odds);
        assert!(!corridors.is_empty());
    }
}
