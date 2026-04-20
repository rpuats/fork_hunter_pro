//! Smart caching system with TTL (Time-To-Live) support
//! Optimized for events, teams, and odds caching

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;
use tracing::{debug, warn};

/// Cache configuration
#[derive(Debug, Clone, Copy)]
pub struct CacheConfig {
    pub max_size: usize,
    pub ttl_seconds: u64,
    pub update_interval_ms: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 50000,
            ttl_seconds: 300,
            update_interval_ms: 100,
        }
    }
}

/// A cache entry with expiration
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub key: String,
    pub data: Arc<Vec<u8>>,
    pub inserted_at: DateTime<Utc>,
    pub ttl_seconds: u64,
    pub access_count: u64,
    pub last_accessed: DateTime<Utc>,
}

impl CacheEntry {
    pub fn new(key: String, data: Vec<u8>, ttl_seconds: u64) -> Self {
        let now = Utc::now();
        Self {
            key,
            data: Arc::new(data),
            inserted_at: now,
            ttl_seconds,
            access_count: 0,
            last_accessed: now,
        }
    }

    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let expiration = self.inserted_at + Duration::seconds(self.ttl_seconds as i64);
        now > expiration
    }

    /// Get remaining TTL in seconds
    pub fn remaining_ttl(&self) -> u64 {
        let now = Utc::now();
        let expiration = self.inserted_at + Duration::seconds(self.ttl_seconds as i64);
        
        if now > expiration {
            0
        } else {
            (expiration.timestamp() - now.timestamp()) as u64
        }
    }

    /// Update access count and last accessed time
    pub fn touch(&mut self) {
        self.access_count += 1;
        self.last_accessed = Utc::now();
    }

    /// Get size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// Multi-tier smart cache for high-performance operations
pub struct SmartCache {
    /// Hot tier - in-memory HashMap for frequently accessed items
    hot_tier: DashMap<String, CacheEntry>,
    
    /// Configuration
    config: RwLock<CacheConfig>,
    
    /// Statistics
    stats: RwLock<CacheStats>,
}

#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub inserts: u64,
    pub total_bytes: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl SmartCache {
    /// Create a new smart cache
    pub fn new(config: CacheConfig) -> Self {
        debug!("Creating SmartCache with max_size={}, ttl={}", config.max_size, config.ttl_seconds);
        
        Self {
            hot_tier: DashMap::new(),
            config: RwLock::new(config),
            stats: RwLock::new(CacheStats {
                hits: 0,
                misses: 0,
                evictions: 0,
                inserts: 0,
                total_bytes: 0,
            }),
        }
    }

    /// Put data in cache with custom TTL
    pub fn put(&self, key: String, data: Vec<u8>, ttl_seconds: Option<u64>) -> bool {
        let config = self.config.read();
        let ttl = ttl_seconds.unwrap_or(config.ttl_seconds);
        drop(config);

        // Check cache size limit
        if self.hot_tier.len() >= self.config.read().max_size {
            self.evict_lru();
        }

        let entry = CacheEntry::new(key.clone(), data, ttl);
        let size = entry.size();

        let mut stats = self.stats.write();
        stats.inserts += 1;
        stats.total_bytes += size;

        self.hot_tier.insert(key, entry);
        true
    }

    /// Get data from cache
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        if let Some(mut entry) = self.hot_tier.get_mut(key) {
            if entry.is_expired() {
                drop(entry);
                self.hot_tier.remove(key);
                
                let mut stats = self.stats.write();
                stats.misses += 1;
                return None;
            }

            entry.touch();
            let mut stats = self.stats.write();
            stats.hits += 1;
            
            return Some((*entry.data).clone());
        }

        let mut stats = self.stats.write();
        stats.misses += 1;
        None
    }

    /// Get data and deserialize
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get(key).and_then(|data| {
            serde_json::from_slice(&data).ok()
        })
    }

    /// Put data as JSON
    pub fn put_json<T: Serialize>(&self, key: String, data: &T, ttl_seconds: Option<u64>) -> bool {
        match serde_json::to_vec(data) {
            Ok(bytes) => self.put(key, bytes, ttl_seconds),
            Err(e) => {
                warn!("Failed to serialize cache data: {}", e);
                false
            }
        }
    }

    /// Remove from cache
    pub fn remove(&self, key: &str) -> bool {
        self.hot_tier.remove(key).is_some()
    }

    /// Clear all cache
    pub fn clear(&self) {
        self.hot_tier.clear();
        let mut stats = self.stats.write();
        stats.total_bytes = 0;
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.hot_tier.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.hot_tier.is_empty()
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        *self.stats.read()
    }

    /// Get cache entry info
    pub fn get_entry(&self, key: &str) -> Option<CacheEntry> {
        self.hot_tier.get(key).map(|e| e.clone())
    }

    /// Cleanup expired entries
    pub fn cleanup_expired(&self) -> usize {
        let mut removed = 0;
        self.hot_tier.retain(|_, entry| {
            if entry.is_expired() {
                removed += 1;
                false
            } else {
                true
            }
        });
        
        if removed > 0 {
            debug!("Cleaned up {} expired cache entries", removed);
        }
        removed
    }

    /// Get remaining TTL for an entry
    pub fn remaining_ttl(&self, key: &str) -> Option<u64> {
        self.hot_tier.get(key).map(|e| e.remaining_ttl())
    }

    /// Update TTL for an entry
    pub fn update_ttl(&self, key: &str, ttl_seconds: u64) -> bool {
        if let Some(mut entry) = self.hot_tier.get_mut(key) {
            entry.ttl_seconds = ttl_seconds;
            true
        } else {
            false
        }
    }

    /// Evict least recently used entry
    fn evict_lru(&self) {
        if let Some((key, _)) = self.hot_tier
            .iter()
            .min_by_key(|entry| entry.value().last_accessed.timestamp()) {
            let key = key.clone();
            self.hot_tier.remove(&key);
            
            let mut stats = self.stats.write();
            stats.evictions += 1;
        }
    }

    /// Get all keys matching a pattern
    pub fn keys_matching(&self, pattern: &str) -> Vec<String> {
        self.hot_tier
            .iter()
            .filter(|entry| entry.key().contains(pattern))
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Batch operations for efficiency
    pub fn batch_get(&self, keys: &[String]) -> Vec<Option<Vec<u8>>> {
        keys.iter()
            .map(|key| self.get(key))
            .collect()
    }

    /// Batch put operations
    pub fn batch_put(&self, entries: Vec<(String, Vec<u8>)>, ttl_seconds: Option<u64>) {
        for (key, data) in entries {
            self.put(key, data, ttl_seconds);
        }
    }

    /// Get memory usage estimate
    pub fn memory_usage(&self) -> usize {
        self.stats.read().total_bytes
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write();
        *stats = CacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            inserts: 0,
            total_bytes: stats.total_bytes,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_creation() {
        let entry = CacheEntry::new("test_key".to_string(), vec![1, 2, 3], 60);
        assert_eq!(entry.key, "test_key");
        assert!(!entry.is_expired());
        assert_eq!(entry.access_count, 0);
    }

    #[test]
    fn test_cache_put_get() {
        let cache = SmartCache::new(CacheConfig::default());
        cache.put("key1".to_string(), vec![1, 2, 3], None);
        
        let result = cache.get("key1");
        assert_eq!(result, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_cache_miss() {
        let cache = SmartCache::new(CacheConfig::default());
        let result = cache.get("nonexistent");
        assert_eq!(result, None);
    }

    #[test]
    fn test_cache_json_operations() {
        let cache = SmartCache::new(CacheConfig::default());
        
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestData {
            value: i32,
            name: String,
        }

        let data = TestData {
            value: 42,
            name: "test".to_string(),
        };

        cache.put_json("test".to_string(), &data, None);
        let retrieved: Option<TestData> = cache.get_json("test");
        
        assert_eq!(retrieved, Some(data));
    }

    #[test]
    fn test_cache_batch_operations() {
        let cache = SmartCache::new(CacheConfig::default());
        
        let entries = vec![
            ("key1".to_string(), vec![1, 2, 3]),
            ("key2".to_string(), vec![4, 5, 6]),
        ];
        
        cache.batch_put(entries, None);
        
        let keys = vec!["key1".to_string(), "key2".to_string()];
        let results = cache.batch_get(&keys);
        
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Some(vec![1, 2, 3]));
        assert_eq!(results[1], Some(vec![4, 5, 6]));
    }

    #[test]
    fn test_cache_cleanup_expired() {
        let cache = SmartCache::new(CacheConfig {
            max_size: 1000,
            ttl_seconds: 1,
            update_interval_ms: 100,
        });

        cache.put("key1".to_string(), vec![1, 2, 3], Some(1));
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        let removed = cache.cleanup_expired();
        assert_eq!(removed, 1);
    }

    #[test]
    fn test_cache_memory_usage() {
        let cache = SmartCache::new(CacheConfig::default());
        cache.put("key1".to_string(), vec![1, 2, 3], None);
        cache.put("key2".to_string(), vec![4, 5, 6], None);
        
        assert!(cache.memory_usage() >= 6);
    }
}
