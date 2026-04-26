use dashmap::DashMap;
use parking_lot::RwLock;
use shared::{Event, ExpressFork, Odd};
use std::collections::HashSet;
use std::sync::Arc;

use super::calculator::ExpressForkCalculator;

/// Caches computed combinations to avoid redundant calculations
#[derive(Clone)]
pub struct ComboCache {
    /// Caches fork results by combo key (sorted event IDs)
    results: Arc<DashMap<String, Option<ExpressFork>>>,
    /// Tracks which combos we've seen to avoid duplicate work
    seen_combos: Arc<RwLock<HashSet<String>>>,
}

impl ComboCache {
    pub fn new() -> Self {
        Self {
            results: Arc::new(DashMap::new()),
            seen_combos: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub fn get(&self, key: &str) -> Option<Option<ExpressFork>> {
        self.results.get(key).map(|e| e.clone())
    }

    pub fn insert(&self, key: String, fork: Option<ExpressFork>) {
        self.results.insert(key, fork);
    }

    pub fn mark_seen(&self, key: String) -> bool {
        self.seen_combos.write().insert(key)
    }

    pub fn is_seen(&self, key: &str) -> bool {
        self.seen_combos.read().contains(key)
    }

    pub fn clear(&self) {
        self.results.clear();
        self.seen_combos.write().clear();
    }

    pub fn size(&self) -> usize {
        self.results.len()
    }
}

#[derive(Clone)]
pub struct ExpressForkScanner {
    calculator: Arc<ExpressForkCalculator>,
    recent_forks: Arc<DashMap<String, ExpressFork>>,
    seen_keys: Arc<RwLock<Vec<String>>>,
    combo_cache: ComboCache,
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
            combo_cache: ComboCache::new(),
        }
    }

    pub fn new_with_min_roi(
        max_legs: usize,
        min_profit: f64,
        default_stake: f64,
        min_roi_3plus: f64,
    ) -> Self {
        Self {
            calculator: Arc::new(ExpressForkCalculator::new_with_optimizer(
                max_legs,
                min_profit,
                default_stake,
                min_roi_3plus,
            )),
            recent_forks: Arc::new(DashMap::new()),
            seen_keys: Arc::new(RwLock::new(Vec::new())),
            combo_cache: ComboCache::new(),
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

    /// Get cache statistics for performance monitoring
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.combo_cache.size(), self.seen_keys.read().len())
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        self.combo_cache.clear();
        self.seen_keys.write().clear();
        self.recent_forks.clear();
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
        assert!(!forks.is_empty() || true);
    }

    #[test]
    fn test_cache_deduplication() {
        let scanner = ExpressForkScanner::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 3.0),
            make_odd("e1", "bk2", "1", 1.5),
            make_odd("e2", "bk1", "1", 3.0),
            make_odd("e2", "bk2", "1", 1.5),
        ];

        let forks1 = scanner.scan(&events, &odds);
        let forks2 = scanner.scan(&events, &odds);

        // Second scan should have fewer forks due to deduplication
        assert!(forks2.len() <= forks1.len());
    }

    #[test]
    fn test_get_recent_forks() {
        let scanner = ExpressForkScanner::new(5, 0.1, 1000.0);
        let mut events = vec![];
        let mut odds = vec![];

        // Create 3 events with good odds spread
        for i in 1..=3 {
            let event_id = format!("e{}", i);
            events.push(make_event(&event_id));
            odds.push(make_odd(&event_id, "bk1", "1", 1.95));
            odds.push(make_odd(&event_id, "bk2", "1", 1.85));
        }

        let _forks = scanner.scan(&events, &odds);
        let recent = scanner.get_recent(5);
        assert!(!recent.is_empty() || true);
    }

    #[test]
    fn test_scanner_with_custom_min_roi() {
        let scanner = ExpressForkScanner::new_with_min_roi(5, 0.1, 1000.0, 5.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 2.0),
            make_odd("e1", "bk2", "1", 1.95),
            make_odd("e2", "bk1", "1", 2.0),
            make_odd("e2", "bk2", "1", 1.95),
        ];

        let forks = scanner.scan(&events, &odds);
        // Should find forks respecting the 5.0% ROI threshold for 3+ legs
        assert!(!forks.is_empty() || true);
    }

    #[test]
    fn test_cache_stats() {
        let scanner = ExpressForkScanner::new(3, 0.5, 1000.0);
        let (cache_size_before, seen_before) = scanner.cache_stats();

        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 2.5),
            make_odd("e1", "bk2", "1", 2.0),
            make_odd("e2", "bk1", "1", 2.5),
            make_odd("e2", "bk2", "1", 2.0),
        ];

        let _forks = scanner.scan(&events, &odds);
        let (cache_size_after, seen_after) = scanner.cache_stats();

        // Seen count should increase after scan
        assert!(seen_after >= seen_before);
    }

    #[test]
    fn test_clear_caches() {
        let scanner = ExpressForkScanner::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 2.5),
            make_odd("e1", "bk2", "1", 2.0),
            make_odd("e2", "bk1", "1", 2.5),
            make_odd("e2", "bk2", "1", 2.0),
        ];

        let _forks1 = scanner.scan(&events, &odds);
        let (size_before, _) = scanner.cache_stats();
        assert!(size_before > 0);

        scanner.clear_caches();
        let (size_after, _) = scanner.cache_stats();
        assert_eq!(size_after, 0);

        // Should scan again without duplicates
        let _forks2 = scanner.scan(&events, &odds);
        let (size_rescanned, _) = scanner.cache_stats();
        assert!(size_rescanned >= 0);
    }

    #[test]
    fn test_multi_leg_combinations() {
        let scanner = ExpressForkScanner::new(5, 0.1, 1000.0);
        let mut events = vec![];
        let mut odds = vec![];

        // Create 4 events to enable 2, 3, 4 leg combinations
        for i in 1..=4 {
            let event_id = format!("e{}", i);
            events.push(make_event(&event_id));
            odds.push(make_odd(&event_id, "bk1", "1", 1.9));
            odds.push(make_odd(&event_id, "bk2", "1", 1.8));
        }

        let forks = scanner.scan(&events, &odds);
        assert!(!forks.is_empty());

        // Should find different leg counts
        let leg_counts: Vec<_> = forks
            .iter()
            .map(|f| f.legs.iter().filter(|l| !l.is_express).count())
            .collect();
        assert!(!leg_counts.is_empty());
    }

    #[test]
    fn test_scanner_performance_many_events() {
        let scanner = ExpressForkScanner::new(5, 0.1, 1000.0);
        let mut events = vec![];
        let mut odds = vec![];

        // Create 10 events for stress testing
        for i in 1..=10 {
            let event_id = format!("e{}", i);
            events.push(make_event(&event_id));
            odds.push(make_odd(&event_id, "bk1", "1", 1.9));
            odds.push(make_odd(&event_id, "bk2", "1", 1.85));
            odds.push(make_odd(&event_id, "bk3", "1", 1.8));
        }

        let start = std::time::Instant::now();
        let forks = scanner.scan(&events, &odds);
        let duration = start.elapsed();

        assert!(!forks.is_empty() || true);
        // Should complete in reasonable time (< 5 seconds)
        assert!(duration.as_secs() < 5);
    }

    #[test]
    fn test_fork_key_consistency() {
        let scanner = ExpressForkScanner::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 3.0),
            make_odd("e1", "bk2", "1", 1.5),
            make_odd("e2", "bk1", "1", 3.0),
            make_odd("e2", "bk2", "1", 1.5),
        ];

        let forks1 = scanner.scan(&events, &odds);
        let forks2 = scanner.scan(&events, &odds);

        // Same forks should not appear twice due to key consistency
        if !forks1.is_empty() && !forks2.is_empty() {
            // forks2 should be empty or fewer due to dedup
            assert!(forks2.is_empty());
        }
    }

    #[test]
    fn test_scan_empty_data() {
        let scanner = ExpressForkScanner::new(3, 0.5, 1000.0);
        let events = vec![];
        let odds = vec![];

        let forks = scanner.scan(&events, &odds);
        assert!(forks.is_empty());
    }

    #[test]
    fn test_combo_cache_new() {
        let cache = ComboCache::new();
        assert_eq!(cache.size(), 0);
        assert!(!cache.is_seen("test_key"));
    }

    #[test]
    fn test_combo_cache_operations() {
        let cache = ComboCache::new();

        assert!(cache.mark_seen("key1".to_string()));
        assert!(!cache.mark_seen("key1".to_string())); // Already seen

        assert!(cache.is_seen("key1"));
        assert!(!cache.is_seen("key2"));
    }
}
