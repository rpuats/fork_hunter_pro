use bloomfilter::Bloom;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use shared::Event;
use std::sync::Arc;
use tracing::debug;

#[derive(Clone)]
pub struct EventPool {
    events: Arc<DashMap<String, EventEntry>>,
    bloom: Arc<RwLock<Bloom<[u8]>>>,
    max_size: usize,
}

#[derive(Debug, Clone)]
struct EventEntry {
    event: Event,
    #[allow(dead_code)]
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    bookmakers: Vec<String>,
    access_count: u64,
}

impl EventPool {
    pub fn new(capacity: usize, error_rate: f64, max_size: usize) -> Self {
        Self {
            events: Arc::new(DashMap::new()),
            bloom: Arc::new(RwLock::new(Bloom::new_for_fp_rate(capacity, error_rate))),
            max_size,
        }
    }

    pub fn insert(&self, event: Event) -> bool {
        let key = self.event_key(&event);
        let key_bytes = key.as_bytes();

        if self.bloom.read().check(key_bytes) {
            if let Some(mut entry) = self.events.get_mut(&key) {
                entry.last_seen = Utc::now();
                entry.access_count += 1;
                if !entry.bookmakers.contains(&event.bookmaker_slug) {
                    entry.bookmakers.push(event.bookmaker_slug.clone());
                }
                debug!(key, "Event updated");
                return false;
            }
        }

        self.bloom.write().set(key_bytes);

        if self.events.len() >= self.max_size {
            self.evict_oldest();
        }

        self.events.insert(
            key.clone(),
            EventEntry {
                event,
                first_seen: Utc::now(),
                last_seen: Utc::now(),
                bookmakers: Vec::new(),
                access_count: 1,
            },
        );

        debug!(key, "New event added to pool");
        true
    }

    pub fn get(&self, key: &str) -> Option<Event> {
        self.events.get(key).map(|e| e.value().event.clone())
    }

    pub fn get_all(&self) -> Vec<Event> {
        self.events
            .iter()
            .map(|e| e.value().event.clone())
            .collect()
    }

    pub fn get_bookmakers_for_event(&self, key: &str) -> Vec<String> {
        self.events
            .get(key)
            .map(|e| e.value().bookmakers.clone())
            .unwrap_or_default()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.events.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn remove_expired(&self, ttl_secs: u64) -> usize {
        let cutoff = Utc::now() - chrono::Duration::seconds(ttl_secs as i64);
        let expired_keys: Vec<String> = self
            .events
            .iter()
            .filter(|e| e.value().last_seen < cutoff)
            .map(|e| e.key().clone())
            .collect();

        let count = expired_keys.len();
        for key in expired_keys {
            self.events.remove(&key);
        }
        count
    }

    pub fn stats(&self) -> EventPoolStats {
        EventPoolStats {
            total_events: self.events.len(),
            max_size: self.max_size,
        }
    }

    fn event_key(&self, event: &Event) -> String {
        format!(
            "{}|{}|{}|{}",
            event.sport, event.home_team, event.away_team, event.league
        )
    }

    fn evict_oldest(&self) {
        let mut oldest_key = None;
        let mut oldest_time = Utc::now();

        for entry in self.events.iter() {
            if entry.value().last_seen < oldest_time {
                oldest_time = entry.value().last_seen;
                oldest_key = Some(entry.key().clone());
            }
        }

        if let Some(key) = oldest_key {
            self.events.remove(&key);
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventPoolStats {
    pub total_events: usize,
    pub max_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Sport;
    use std::collections::HashMap;

    fn make_event(id: &str, home: &str, away: &str) -> Event {
        Event {
            id: id.to_string(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: home.into(),
            away_team: away.into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_insert_new_event() {
        let pool = EventPool::new(1000, 0.01, 100);
        let event = make_event("1", "Team A", "Team B");
        assert!(pool.insert(event));
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_insert_duplicate() {
        let pool = EventPool::new(1000, 0.01, 100);
        let event1 = make_event("1", "Team A", "Team B");
        let event2 = make_event("2", "Team A", "Team B");
        assert!(pool.insert(event1));
        assert!(!pool.insert(event2));
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_get_event() {
        let pool = EventPool::new(1000, 0.01, 100);
        let event = make_event("1", "Team A", "Team B");
        pool.insert(event.clone());
        let key = "football|Team A|Team B|Test".to_string();
        let retrieved = pool.get(&key);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().home_team, "Team A");
    }

    #[test]
    fn test_get_all() {
        let pool = EventPool::new(1000, 0.01, 100);
        pool.insert(make_event("1", "Team A", "Team B"));
        pool.insert(make_event("2", "Team C", "Team D"));
        let all = pool.get_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_remove_expired() {
        let pool = EventPool::new(1000, 0.01, 100);
        pool.insert(make_event("1", "Team A", "Team B"));
        let removed = pool.remove_expired(0);
        assert_eq!(removed, 1);
    }
}
