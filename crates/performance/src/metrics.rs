//! Performance metrics tracking and analysis

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info};

/// Performance metrics collector
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub total_operations: u64,
    pub total_duration_ms: u64,
    pub operations_by_category: Arc<DashMap<String, OperationStats>>,
    pub peak_throughput: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone)]
pub struct OperationStats {
    pub count: u64,
    pub total_duration_ms: u64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl OperationStats {
    pub fn avg_duration_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.count as f64
        }
    }

    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMetrics {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            total_duration_ms: 0,
            operations_by_category: Arc::new(DashMap::new()),
            peak_throughput: 0.0,
            avg_latency_ms: 0.0,
        }
    }

    /// Record an operation
    pub fn record_operation(&mut self, duration_ms: u64, category: &str) {
        self.total_operations += 1;
        self.total_duration_ms += duration_ms;
        self.avg_latency_ms = self.total_duration_ms as f64 / self.total_operations as f64;

        self.operations_by_category
            .entry(category.to_string())
            .or_insert_with(|| OperationStats {
                count: 0,
                total_duration_ms: 0,
                min_duration_ms: u64::MAX,
                max_duration_ms: 0,
                cache_hits: 0,
                cache_misses: 0,
            })
            .and_modify(|stats| {
                stats.count += 1;
                stats.total_duration_ms += duration_ms;
                stats.min_duration_ms = stats.min_duration_ms.min(duration_ms);
                stats.max_duration_ms = stats.max_duration_ms.max(duration_ms);
            });
    }

    /// Record cache hit
    pub fn record_cache_hit(&self, category: &str) {
        if let Some(mut stats) = self.operations_by_category.get_mut(category) {
            stats.cache_hits += 1;
        }
    }

    /// Record cache miss
    pub fn record_cache_miss(&self, category: &str) {
        if let Some(mut stats) = self.operations_by_category.get_mut(category) {
            stats.cache_misses += 1;
        }
    }

    /// Get operation stats by category
    pub fn get_stats(&self, category: &str) -> Option<OperationStats> {
        self.operations_by_category
            .get(category)
            .map(|stats| stats.clone())
    }

    /// Get all categories
    pub fn categories(&self) -> Vec<String> {
        self.operations_by_category
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Calculate cache hit rate
    pub fn cache_hit_rate(&self) -> f64 {
        let mut total_hits = 0u64;
        let mut total_misses = 0u64;

        for entry in self.operations_by_category.iter() {
            total_hits += entry.value().cache_hits;
            total_misses += entry.value().cache_misses;
        }

        let total = total_hits + total_misses;
        if total == 0 {
            0.0
        } else {
            total_hits as f64 / total as f64
        }
    }

    /// Calculate throughput (operations per second)
    pub fn throughput_ops_per_sec(&self) -> f64 {
        if self.total_duration_ms == 0 {
            0.0
        } else {
            (self.total_operations as f64 / self.total_duration_ms as f64) * 1000.0
        }
    }

    /// Get performance summary
    pub fn summary(&self) -> String {
        format!(
            "Metrics Summary:\n\
             - Total Operations: {}\n\
             - Total Duration: {}ms\n\
             - Avg Latency: {:.2}ms\n\
             - Throughput: {:.2} ops/sec\n\
             - Cache Hit Rate: {:.1}%",
            self.total_operations,
            self.total_duration_ms,
            self.avg_latency_ms,
            self.throughput_ops_per_sec(),
            self.cache_hit_rate() * 100.0
        )
    }
}

/// Performance tracker for time-sensitive operations
pub struct PerformanceTracker {
    start: Instant,
    category: String,
}

impl PerformanceTracker {
    /// Create a new performance tracker
    pub fn new(category: &str) -> Self {
        Self {
            start: Instant::now(),
            category: category.to_string(),
        }
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Get elapsed time in microseconds
    pub fn elapsed_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }

    /// Check if a threshold was exceeded
    pub fn exceeded_threshold_ms(&self, threshold_ms: u64) -> bool {
        self.elapsed_ms() > threshold_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = PerformanceMetrics::new();
        assert_eq!(metrics.total_operations, 0);
        assert_eq!(metrics.total_duration_ms, 0);
    }

    #[test]
    fn test_record_operation() {
        let mut metrics = PerformanceMetrics::new();
        metrics.record_operation(50, "test_category");
        
        assert_eq!(metrics.total_operations, 1);
        assert_eq!(metrics.total_duration_ms, 50);
    }

    #[test]
    fn test_throughput_calculation() {
        let mut metrics = PerformanceMetrics::new();
        metrics.record_operation(100, "test");
        metrics.record_operation(100, "test");
        metrics.record_operation(100, "test");
        
        let throughput = metrics.throughput_ops_per_sec();
        assert!(throughput > 0.0);
    }

    #[test]
    fn test_performance_tracker() {
        let tracker = PerformanceTracker::new("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        assert!(tracker.elapsed_ms() >= 10);
        assert!(tracker.elapsed_us() >= 10000);
    }

    #[test]
    fn test_cache_hit_rate_calculation() {
        let metrics = PerformanceMetrics::new();
        metrics.record_cache_hit("test");
        metrics.record_cache_hit("test");
        metrics.record_cache_miss("test");
        
        let hit_rate = metrics.cache_hit_rate();
        assert!(hit_rate > 0.0);
        assert!(hit_rate <= 1.0);
    }
}
