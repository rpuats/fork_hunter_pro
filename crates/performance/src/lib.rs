//! Performance optimization module for fork_hunter
//! 
//! Features:
//! - SmartCache with TTL for events, teams, odds
//! - Parallel parser execution with tokio::join_all
//! - Request batching in calculator
//! - Thread pooling with rayon for CPU-bound operations
//! - Comprehensive benchmarking suite (25+ benchmarks)
//! 
//! Target: 10x throughput improvement

pub mod cache;
pub mod parser_executor;
pub mod batch_calculator;
pub mod thread_pool;
pub mod metrics;
pub mod pipeline;

pub use cache::{SmartCache, CacheEntry, CacheConfig};
pub use parser_executor::{ParallelParserExecutor, ParserExecutionConfig};
pub use batch_calculator::{BatchCalculator, BatchRequest, BatchConfig};
pub use thread_pool::{ThreadPoolExecutor, ThreadPoolConfig, PoolStats};
pub use metrics::PerformanceMetrics;
pub use pipeline::OptimizedPipeline;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Main performance manager orchestrating all optimization components
#[derive(Clone)]
pub struct PerformanceManager {
    cache: Arc<SmartCache>,
    parser_executor: Arc<ParallelParserExecutor>,
    batch_calculator: Arc<BatchCalculator>,
    thread_pool: Arc<ThreadPoolExecutor>,
    metrics: Arc<RwLock<PerformanceMetrics>>,
}

impl PerformanceManager {
    /// Create a new performance manager with default configurations
    pub async fn new() -> anyhow::Result<Self> {
        info!("Initializing PerformanceManager");
        
        let cache = Arc::new(SmartCache::new(CacheConfig::default()));
        let parser_executor = Arc::new(ParallelParserExecutor::new(
            ParserExecutionConfig::default(),
        ));
        let batch_calculator = Arc::new(BatchCalculator::new(BatchConfig::default()));
        let thread_pool = Arc::new(ThreadPoolExecutor::new(ThreadPoolConfig::default())?);
        let metrics = Arc::new(RwLock::new(PerformanceMetrics::new()));
        
        Ok(Self {
            cache,
            parser_executor,
            batch_calculator,
            thread_pool,
            metrics,
        })
    }

    /// Create with custom configurations
    pub async fn with_config(
        cache_config: CacheConfig,
        parser_config: ParserExecutionConfig,
        batch_config: BatchConfig,
        pool_config: ThreadPoolConfig,
    ) -> anyhow::Result<Self> {
        info!("Initializing PerformanceManager with custom config");
        
        let cache = Arc::new(SmartCache::new(cache_config));
        let parser_executor = Arc::new(ParallelParserExecutor::new(parser_config));
        let batch_calculator = Arc::new(BatchCalculator::new(batch_config));
        let thread_pool = Arc::new(ThreadPoolExecutor::new(pool_config)?);
        let metrics = Arc::new(RwLock::new(PerformanceMetrics::new()));
        
        Ok(Self {
            cache,
            parser_executor,
            batch_calculator,
            thread_pool,
            metrics,
        })
    }

    /// Get reference to the cache
    pub fn cache(&self) -> &SmartCache {
        &self.cache
    }

    /// Get reference to the parser executor
    pub fn parser_executor(&self) -> &ParallelParserExecutor {
        &self.parser_executor
    }

    /// Get reference to the batch calculator
    pub fn batch_calculator(&self) -> &BatchCalculator {
        &self.batch_calculator
    }

    /// Get reference to the thread pool
    pub fn thread_pool(&self) -> &ThreadPoolExecutor {
        &self.thread_pool
    }

    /// Get current metrics
    pub async fn metrics(&self) -> PerformanceMetrics {
        self.metrics.read().await.clone()
    }

    /// Record an operation metric
    pub async fn record_operation(&self, duration_ms: u64, category: &str) {
        let mut m = self.metrics.write().await;
        m.record_operation(duration_ms, category);
    }

    /// Get performance summary
    pub async fn summary(&self) -> String {
        let metrics = self.metrics.read().await;
        let pool_stats = self.thread_pool.stats();
        
        format!(
            "Performance Summary:\n\
             - Cache Size: {}\n\
             - Total Operations: {}\n\
             - Avg Latency: {:.2}ms\n\
             - Thread Pool Utilization: {:.1}%\n\
             - Cache Hit Rate: {:.1}%",
            self.cache.len(),
            metrics.total_operations,
            metrics.avg_latency_ms(),
            (pool_stats.active_threads as f64 / pool_stats.total_threads as f64) * 100.0,
            metrics.cache_hit_rate() * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_manager_creation() {
        let manager = PerformanceManager::new().await;
        assert!(manager.is_ok());
        let manager = manager.unwrap();
        assert!(manager.cache.len() >= 0);
    }

    #[tokio::test]
    async fn test_performance_manager_with_config() {
        let cache_config = CacheConfig {
            max_size: 10000,
            ttl_seconds: 300,
            update_interval_ms: 100,
        };
        let parser_config = ParserExecutionConfig::default();
        let batch_config = BatchConfig::default();
        let pool_config = ThreadPoolConfig::default();

        let manager = PerformanceManager::with_config(
            cache_config,
            parser_config,
            batch_config,
            pool_config,
        )
        .await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let manager = PerformanceManager::new().await.unwrap();
        manager.record_operation(50, "test").await;
        manager.record_operation(75, "test").await;
        
        let metrics = manager.metrics().await;
        assert_eq!(metrics.total_operations, 2);
    }
}
