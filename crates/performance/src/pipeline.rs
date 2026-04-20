//! Optimized pipeline integrating all performance components
//! Orchestrates the entire processing flow for maximum throughput

use crate::{
    batch_calculator::{BatchCalculator, BatchRequest},
    cache::SmartCache,
    parser_executor::ParallelParserExecutor,
    thread_pool::ThreadPoolExecutor,
    PerformanceMetrics,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Optimized pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub enable_caching: bool,
    pub enable_batching: bool,
    pub enable_parallel_parsing: bool,
    pub enable_thread_pool: bool,
    pub batch_size: usize,
    pub cache_ttl_ms: u64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            enable_batching: true,
            enable_parallel_parsing: true,
            enable_thread_pool: true,
            batch_size: 1000,
            cache_ttl_ms: 5000,
        }
    }
}

/// Unified result from the pipeline
#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub total_items_processed: usize,
    pub total_duration_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub successful_operations: usize,
    pub failed_operations: usize,
}

impl PipelineResult {
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.successful_operations + self.failed_operations;
        if total == 0 {
            0.0
        } else {
            self.successful_operations as f64 / total as f64
        }
    }

    pub fn throughput_per_sec(&self) -> f64 {
        if self.total_duration_ms == 0 {
            0.0
        } else {
            (self.total_items_processed as f64 / self.total_duration_ms as f64) * 1000.0
        }
    }
}

/// Optimized pipeline combining all performance components
pub struct OptimizedPipeline {
    config: PipelineConfig,
    cache: Arc<SmartCache>,
    parser_executor: Arc<ParallelParserExecutor>,
    batch_calculator: Arc<BatchCalculator>,
    thread_pool: Arc<ThreadPoolExecutor>,
    metrics: Arc<tokio::sync::RwLock<PerformanceMetrics>>,
}

impl OptimizedPipeline {
    /// Create a new optimized pipeline
    pub async fn new(
        config: PipelineConfig,
        cache: Arc<SmartCache>,
        parser_executor: Arc<ParallelParserExecutor>,
        batch_calculator: Arc<BatchCalculator>,
        thread_pool: Arc<ThreadPoolExecutor>,
    ) -> Self {
        info!("Creating OptimizedPipeline with config: {:?}", config);

        Self {
            config,
            cache,
            parser_executor,
            batch_calculator,
            thread_pool,
            metrics: Arc::new(tokio::sync::RwLock::new(PerformanceMetrics::new())),
        }
    }

    /// Execute a batch of requests through the optimized pipeline
    pub async fn execute(&self, requests: Vec<BatchRequest>) -> PipelineResult {
        let start = Instant::now();
        let total_items = requests.len();
        let mut cache_hits = 0;
        let mut cache_misses = 0;
        let mut successful = 0;
        let mut failed = 0;

        debug!("Processing {} items through pipeline", total_items);

        // Stage 1: Check cache
        if self.config.enable_caching {
            for request in &requests {
                let cache_key = format!(
                    "{}:{}",
                    request.event_id,
                    request.odds.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(",")
                );

                if let Some(_) = self.cache.get(&cache_key) {
                    cache_hits += 1;
                    successful += 1;
                } else {
                    cache_misses += 1;
                }
            }
        }

        // Stage 2: Batch processing
        if self.config.enable_batching {
            for request in requests {
                if self.batch_calculator.add_request(request.clone()) {
                    if self.batch_calculator.pending_size() >= self.config.batch_size {
                        let results = self.batch_calculator.process_batch().await;
                        successful += results.iter().filter(|r| r.surebet_found).count();
                        failed += results.iter().filter(|r| !r.surebet_found).count();
                    }
                }
            }

            // Process remaining items in batch
            let remaining = self.batch_calculator.process_batch().await;
            successful += remaining.iter().filter(|r| r.surebet_found).count();
            failed += remaining.iter().filter(|r| !r.surebet_found).count();
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            "Pipeline execution completed: {} items processed in {}ms",
            total_items, duration_ms
        );

        PipelineResult {
            total_items_processed: total_items,
            total_duration_ms: duration_ms,
            cache_hits,
            cache_misses,
            successful_operations: successful,
            failed_operations: failed,
        }
    }

    /// Get cache reference
    pub fn cache(&self) -> &Arc<SmartCache> {
        &self.cache
    }

    /// Get parser executor reference
    pub fn parser_executor(&self) -> &Arc<ParallelParserExecutor> {
        &self.parser_executor
    }

    /// Get batch calculator reference
    pub fn batch_calculator(&self) -> &Arc<BatchCalculator> {
        &self.batch_calculator
    }

    /// Get thread pool reference
    pub fn thread_pool(&self) -> &Arc<ThreadPoolExecutor> {
        &self.thread_pool
    }

    /// Get current metrics
    pub async fn metrics(&self) -> PerformanceMetrics {
        self.metrics.read().await.clone()
    }

    /// Get pipeline configuration
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Clear all caches and reset state
    pub fn reset(&self) {
        self.cache.clear();
        self.batch_calculator.clear_cache();
        self.parser_executor.clear_history();
        self.parser_executor.reset_stats();
        self.batch_calculator.reset_stats();
        self.thread_pool.reset_stats();
    }

    /// Get comprehensive pipeline status
    pub async fn status(&self) -> String {
        let metrics = self.metrics().await;
        let parser_stats = self.parser_executor.stats();
        let batch_stats = self.batch_calculator.stats();
        let pool_stats = self.thread_pool.stats();

        format!(
            "Pipeline Status:\n\
             === Caching ===\n\
             Cache Size: {}\n\
             Cache Memory: {} bytes\n\
             === Parsing ===\n\
             Total Executions: {}\n\
             Success Rate: {:.1}%\n\
             Avg Duration: {:.2}ms\n\
             === Batching ===\n\
             Total Batches: {}\n\
             Avg Batch Size: {:.1}\n\
             Cache Hit Rate: {:.1}%\n\
             === Threading ===\n\
             Active Threads: {}/{}\n\
             Completed Tasks: {}\n\
             Failed Tasks: {}",
            self.cache.len(),
            self.cache.memory_usage(),
            parser_stats.total_executions,
            parser_stats.success_rate() * 100.0,
            parser_stats.avg_duration_ms,
            batch_stats.total_batches,
            batch_stats.avg_batch_size,
            batch_stats.cache_hit_rate() * 100.0,
            pool_stats.active_threads,
            pool_stats.total_threads,
            pool_stats.completed_tasks,
            pool_stats.failed_tasks
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        batch_calculator::BatchConfig, cache::CacheConfig, 
        parser_executor::ParserExecutionConfig, thread_pool::ThreadPoolConfig,
    };

    #[tokio::test]
    async fn test_pipeline_creation() {
        let cache = Arc::new(crate::SmartCache::new(CacheConfig::default()));
        let executor = Arc::new(crate::ParallelParserExecutor::new(
            ParserExecutionConfig::default(),
        ));
        let calculator = Arc::new(crate::BatchCalculator::new(BatchConfig::default()));
        let pool = Arc::new(crate::ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap());

        let pipeline = OptimizedPipeline::new(
            PipelineConfig::default(),
            cache,
            executor,
            calculator,
            pool,
        )
        .await;

        let status = pipeline.status().await;
        assert!(!status.is_empty());
    }

    #[tokio::test]
    async fn test_pipeline_execution() {
        let cache = Arc::new(crate::SmartCache::new(CacheConfig::default()));
        let executor = Arc::new(crate::ParallelParserExecutor::new(
            ParserExecutionConfig::default(),
        ));
        let calculator = Arc::new(crate::BatchCalculator::new(BatchConfig::default()));
        let pool = Arc::new(crate::ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap());

        let pipeline = OptimizedPipeline::new(
            PipelineConfig::default(),
            cache,
            executor,
            calculator,
            pool,
        )
        .await;

        let requests = vec![
            BatchRequest {
                id: "1".to_string(),
                event_id: "evt1".to_string(),
                odds: vec![2.0, 2.0],
                bookmaker: "bk1".to_string(),
                market: "1x2".to_string(),
            },
        ];

        let result = pipeline.execute(requests).await;
        assert_eq!(result.total_items_processed, 1);
    }
}
