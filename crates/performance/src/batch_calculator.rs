//! Batch calculator for efficient processing of multiple requests
//! Reduces per-request overhead through batching and caching

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info};

/// Batch calculator configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub max_batch_size: usize,
    pub batch_timeout_ms: u64,
    pub enable_caching: bool,
    pub cache_ttl_ms: u64,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            batch_timeout_ms: 500,
            enable_caching: true,
            cache_ttl_ms: 5000,
        }
    }
}

/// A single request in a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest {
    pub id: String,
    pub event_id: String,
    pub odds: Vec<f64>,
    pub bookmaker: String,
    pub market: String,
}

/// Result of batch calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub request_id: String,
    pub event_id: String,
    pub surebet_found: bool,
    pub profit_percent: f64,
    pub roi: f64,
    pub confidence: f64,
    pub processing_time_us: u64,
}

/// Batch statistics
#[derive(Debug, Clone, Copy)]
pub struct BatchStats {
    pub total_batches: u64,
    pub total_requests: u64,
    pub avg_batch_size: f64,
    pub avg_processing_time_us: f64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl BatchStats {
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
}

/// Batch calculator
pub struct BatchCalculator {
    config: RwLock<BatchConfig>,
    pending_batch: RwLock<Vec<BatchRequest>>,
    result_cache: DashMap<String, BatchResult>,
    stats: RwLock<BatchStats>,
}

impl BatchCalculator {
    /// Create a new batch calculator
    pub fn new(config: BatchConfig) -> Self {
        info!(
            "Creating BatchCalculator with max_batch_size={}, batch_timeout_ms={}",
            config.max_batch_size, config.batch_timeout_ms
        );

        Self {
            config: RwLock::new(config),
            pending_batch: RwLock::new(Vec::new()),
            result_cache: DashMap::new(),
            stats: RwLock::new(BatchStats {
                total_batches: 0,
                total_requests: 0,
                avg_batch_size: 0.0,
                avg_processing_time_us: 0.0,
                cache_hits: 0,
                cache_misses: 0,
            }),
        }
    }

    /// Add a request to the batch
    pub fn add_request(&self, request: BatchRequest) -> bool {
        let mut batch = self.pending_batch.write();
        
        if batch.len() < self.config.read().max_batch_size {
            batch.push(request);
            true
        } else {
            false
        }
    }

    /// Add multiple requests in one call
    pub fn add_requests(&self, requests: Vec<BatchRequest>) -> usize {
        let mut batch = self.pending_batch.write();
        let config = self.config.read();
        
        let available_space = config.max_batch_size.saturating_sub(batch.len());
        let to_add = std::cmp::min(requests.len(), available_space);
        
        batch.extend_from_slice(&requests[..to_add]);
        to_add
    }

    /// Process current batch
    pub async fn process_batch(&self) -> Vec<BatchResult> {
        let mut batch = self.pending_batch.write();
        
        if batch.is_empty() {
            return Vec::new();
        }

        let requests = batch.drain(..).collect::<Vec<_>>();
        drop(batch);

        let start = Instant::now();
        let mut results = Vec::new();

        for request in requests {
            let result = self.calculate_surebet(&request).await;
            results.push(result);
        }

        let processing_time = start.elapsed().as_micros() as u64;
        
        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.total_batches += 1;
            stats.total_requests += results.len() as u64;
            stats.avg_batch_size = results.len() as f64;
            stats.avg_processing_time_us = processing_time as f64 / results.len().max(1) as f64;
        }

        info!(
            "Processed batch of {} requests in {}us",
            results.len(),
            processing_time
        );

        results
    }

    /// Calculate surebet for a single request
    async fn calculate_surebet(&self, request: &BatchRequest) -> BatchResult {
        let start = Instant::now();
        let cache_key = format!("{}:{}", request.event_id, request.odds.join(","));

        // Check cache
        if self.config.read().enable_caching {
            if let Some(cached) = self.result_cache.get(&cache_key) {
                let mut stats = self.stats.write();
                stats.cache_hits += 1;
                return cached.clone();
            }
        }

        // Calculate surebet
        let (profit_percent, roi, confidence) = self.calculate_odds(&request.odds);

        let result = BatchResult {
            request_id: request.id.clone(),
            event_id: request.event_id.clone(),
            surebet_found: profit_percent > 0.0,
            profit_percent,
            roi,
            confidence,
            processing_time_us: start.elapsed().as_micros() as u64,
        };

        // Cache result if enabled
        if self.config.read().enable_caching {
            self.result_cache.insert(cache_key, result.clone());
            let mut stats = self.stats.write();
            stats.cache_misses += 1;
        }

        result
    }

    /// Calculate surebet probability and ROI
    fn calculate_odds(&self, odds: &[f64]) -> (f64, f64, f64) {
        if odds.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        // Calculate implied probability sum
        let inv_sum: f64 = odds.iter().map(|o| 1.0 / o).sum();

        // Profit margin
        let profit_margin = (inv_sum - 1.0) * 100.0;

        // If profitable (negative margin), it's a potential surebet
        let profit_percent = if profit_margin < 0.0 {
            -profit_margin
        } else {
            0.0
        };

        // ROI calculation
        let roi = if inv_sum > 0.0 {
            ((1.0 / inv_sum) - 1.0) * 100.0
        } else {
            0.0
        };

        // Confidence based on number of legs and margin
        let confidence = (1.0 / odds.len() as f64) * (1.0 + profit_margin.abs() / 100.0);

        (profit_percent, roi.max(0.0), confidence.min(1.0).max(0.0))
    }

    /// Get pending batch size
    pub fn pending_size(&self) -> usize {
        self.pending_batch.read().len()
    }

    /// Get statistics
    pub fn stats(&self) -> BatchStats {
        *self.stats.read()
    }

    /// Get cached result
    pub fn get_cached(&self, event_id: &str, odds: &[f64]) -> Option<BatchResult> {
        let key = format!("{}:{}", event_id, odds.join(","));
        self.result_cache.get(&key).map(|r| r.clone())
    }

    /// Clear result cache
    pub fn clear_cache(&self) {
        self.result_cache.clear();
        let mut stats = self.stats.write();
        stats.cache_hits = 0;
        stats.cache_misses = 0;
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write();
        *stats = BatchStats {
            total_batches: 0,
            total_requests: 0,
            avg_batch_size: 0.0,
            avg_processing_time_us: 0.0,
            cache_hits: 0,
            cache_misses: 0,
        };
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.result_cache.len()
    }

    /// Parallel batch processing (multiple batches at once)
    pub async fn process_parallel_batches(&self, num_batches: usize) -> Vec<BatchResult> {
        let mut all_results = Vec::new();

        for _ in 0..num_batches {
            let results = self.process_batch().await;
            all_results.extend(results);
        }

        all_results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_calculator_creation() {
        let calc = BatchCalculator::new(BatchConfig::default());
        assert_eq!(calc.pending_size(), 0);
    }

    #[test]
    fn test_add_request() {
        let calc = BatchCalculator::new(BatchConfig::default());
        let request = BatchRequest {
            id: "1".to_string(),
            event_id: "evt1".to_string(),
            odds: vec![2.0, 2.0],
            bookmaker: "bk1".to_string(),
            market: "1x2".to_string(),
        };

        assert!(calc.add_request(request));
        assert_eq!(calc.pending_size(), 1);
    }

    #[test]
    fn test_calculate_odds() {
        let calc = BatchCalculator::new(BatchConfig::default());
        let (profit, roi, confidence) = calc.calculate_odds(&[2.0, 2.0]);
        
        // For 2.0 and 2.0, inv_sum = 1.0, so no profit
        assert_eq!(profit, 0.0);
        assert!(confidence >= 0.0 && confidence <= 1.0);
    }

    #[test]
    fn test_batch_caching() {
        let calc = BatchCalculator::new(BatchConfig::default());
        
        let cached = calc.get_cached("evt1", &[2.0, 2.0]);
        assert_eq!(cached, None);
    }

    #[test]
    fn test_add_multiple_requests() {
        let calc = BatchCalculator::new(BatchConfig::default());
        
        let requests = vec![
            BatchRequest {
                id: "1".to_string(),
                event_id: "evt1".to_string(),
                odds: vec![2.0, 2.0],
                bookmaker: "bk1".to_string(),
                market: "1x2".to_string(),
            },
            BatchRequest {
                id: "2".to_string(),
                event_id: "evt2".to_string(),
                odds: vec![1.5, 1.5],
                bookmaker: "bk2".to_string(),
                market: "1x2".to_string(),
            },
        ];

        let added = calc.add_requests(requests);
        assert_eq!(added, 2);
    }

    #[tokio::test]
    async fn test_process_batch() {
        let calc = BatchCalculator::new(BatchConfig::default());
        
        let request = BatchRequest {
            id: "1".to_string(),
            event_id: "evt1".to_string(),
            odds: vec![2.0, 2.0],
            bookmaker: "bk1".to_string(),
            market: "1x2".to_string(),
        };

        calc.add_request(request);
        let results = calc.process_batch().await;
        
        assert_eq!(results.len(), 1);
    }
}
