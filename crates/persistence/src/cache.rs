use moka::future::Cache;
use std::time::Duration;

#[derive(Clone)]
pub struct TtlCache<V> {
    cache: Cache<String, V>,
}

impl<V: Clone + Send + Sync + 'static> TtlCache<V> {
    pub fn new(max_capacity: u64, ttl_secs: u64) -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(max_capacity)
                .time_to_live(Duration::from_secs(ttl_secs))
                .build(),
        }
    }

    pub async fn get(&self, key: &str) -> Option<V> {
        self.cache.get(key).await
    }

    pub async fn insert(&self, key: &str, value: V) {
        self.cache.insert(key.to_string(), value).await;
    }

    pub async fn remove(&self, key: &str) {
        self.cache.invalidate(key).await;
    }

    pub fn len(&self) -> u64 {
        self.cache.entry_count()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.entry_count() == 0
    }
}
