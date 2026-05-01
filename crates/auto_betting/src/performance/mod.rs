//! Performance Monitor - Tracks and optimizes critical paths

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Performance metrics for a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetrics {
    pub name: String,
    pub count: u64,
    pub total_duration_ms: f64,
    pub avg_duration_ms: f64,
    pub min_duration_ms: f64,
    pub max_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub last_executed: Option<DateTime<Utc>>,
    pub success_rate: f64,
    pub errors: u64,
}

impl OperationMetrics {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            count: 0,
            total_duration_ms: 0.0,
            avg_duration_ms: 0.0,
            min_duration_ms: f64::MAX,
            max_duration_ms: 0.0,
            p95_duration_ms: 0.0,
            p99_duration_ms: 0.0,
            last_executed: None,
            success_rate: 100.0,
            errors: 0,
        }
    }

    pub fn record(&mut self, duration_ms: f64, success: bool) {
        self.count += 1;
        self.total_duration_ms += duration_ms;
        self.avg_duration_ms = self.total_duration_ms / self.count as f64;
        self.min_duration_ms = self.min_duration_ms.min(duration_ms);
        self.max_duration_ms = self.max_duration_ms.max(duration_ms);
        self.last_executed = Some(Utc::now());

        if !success {
            self.errors += 1;
        }
        self.success_rate = ((self.count - self.errors) as f64 / self.count as f64) * 100.0;
    }
}

/// Performance targets
#[derive(Debug, Clone, Copy)]
pub struct PerformanceTargets {
    pub scan_cycle_ms: u64,           // 300-500ms
    pub fork_to_display_ms: u64,      // < 1s
    pub auto_bet_ms: u64,             // < 5s
    pub semi_auto_bet_ms: u64,        // < 10s
    pub ui_fps: u32,                  // 60
}

impl Default for PerformanceTargets {
    fn default() -> Self {
        Self {
            scan_cycle_ms: 500,
            fork_to_display_ms: 1000,
            auto_bet_ms: 5000,
            semi_auto_bet_ms: 10000,
            ui_fps: 60,
        }
    }
}

/// Performance monitor
pub struct PerformanceMonitor {
    metrics: Arc<RwLock<HashMap<String, OperationMetrics>>>,
    targets: PerformanceTargets,
    latencies: Arc<RwLock<HashMap<String, Vec<f64>>>>, // For percentile calculation
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            targets: PerformanceTargets::default(),
            latencies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_targets(targets: PerformanceTargets) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            targets,
            latencies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Time an operation and record metrics
    pub async fn time_operation<F, Fut, T>(
        &self,
        name: &str,
        f: F,
    ) -> T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        let duration_ms = duration.as_secs_f64() * 1000.0;

        self.record(name, duration_ms, true).await;

        // Check against targets
        self.check_target(name, duration_ms).await;

        result
    }

    /// Time an operation that can fail
    pub async fn time_operation_result<F, Fut, T, E>(
        &self,
        name: &str,
        f: F,
    ) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        let duration_ms = duration.as_secs_f64() * 1000.0;

        let success = result.is_ok();
        self.record(name, duration_ms, success).await;

        if success {
            self.check_target(name, duration_ms).await;
        }

        result
    }

    /// Record a metric
    async fn record(&self, name: &str, duration_ms: f64, success: bool) {
        let mut metrics = self.metrics.write().await;
        let metric = metrics.entry(name.to_string()).or_insert_with(|| {
            OperationMetrics::new(name)
        });
        metric.record(duration_ms, success);

        // Store latency for percentile calculation
        drop(metrics);
        let mut latencies = self.latencies.write().await;
        let latency_list = latencies.entry(name.to_string()).or_insert_with(Vec::new);
        latency_list.push(duration_ms);

        // Keep only last 1000 measurements
        if latency_list.len() > 1000 {
            latency_list.remove(0);
        }
    }

    /// Check if operation meets target
    async fn check_target(&self, name: &str, duration_ms: f64) {
        let target_ms = match name {
            "scan_cycle" => self.targets.scan_cycle_ms as f64,
            "fork_to_display" => self.targets.fork_to_display_ms as f64,
            "auto_bet" => self.targets.auto_bet_ms as f64,
            "semi_auto_bet" => self.targets.semi_auto_bet_ms as f64,
            _ => return,
        };

        if duration_ms > target_ms {
            warn!(
                operation = name,
                duration_ms = duration_ms,
                target_ms = target_ms,
                "Performance target exceeded"
            );
        }
    }

    /// Get all metrics
    pub async fn get_metrics(&self) -> Vec<OperationMetrics> {
        let metrics = self.metrics.read().await;
        let latencies = self.latencies.read().await;

        let mut result = Vec::new();
        for (name, metric) in metrics.iter() {
            let mut m = metric.clone();

            // Calculate percentiles
            if let Some(latency_list) = latencies.get(name) {
                if !latency_list.is_empty() {
                    let mut sorted = latency_list.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

                    let p95_idx = (sorted.len() as f64 * 0.95) as usize;
                    let p99_idx = (sorted.len() as f64 * 0.99) as usize;

                    m.p95_duration_ms = sorted[p95_idx.min(sorted.len() - 1)];
                    m.p99_duration_ms = sorted[p99_idx.min(sorted.len() - 1)];
                }
            }

            result.push(m);
        }

        result
    }

    /// Get metric by name
    pub async fn get_metric(&self, name: &str) -> Option<OperationMetrics> {
        let metrics = self.metrics.read().await;
        metrics.get(name).cloned()
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.clear();
        let mut latencies = self.latencies.write().await;
        latencies.clear();
    }

    /// Check if system meets all targets
    pub async fn check_health(&self) -> PerformanceHealth {
        let metrics = self.get_metrics().await;
        let mut health = PerformanceHealth::Healthy;
        let mut violations = Vec::new();

        for metric in metrics {
            match metric.name.as_str() {
                "scan_cycle" => {
                    if metric.avg_duration_ms > self.targets.scan_cycle_ms as f64 {
                        health = PerformanceHealth::Degraded;
                        violations.push(format!(
                            "Scan cycle: {:.0}ms (target: {}ms)",
                            metric.avg_duration_ms, self.targets.scan_cycle_ms
                        ));
                    }
                }
                "fork_to_display" => {
                    if metric.p95_duration_ms > self.targets.fork_to_display_ms as f64 {
                        health = PerformanceHealth::Degraded;
                        violations.push(format!(
                            "Fork to display p95: {:.0}ms (target: {}ms)",
                            metric.p95_duration_ms, self.targets.fork_to_display_ms
                        ));
                    }
                }
                "auto_bet" => {
                    if metric.p95_duration_ms > self.targets.auto_bet_ms as f64 {
                        health = PerformanceHealth::Critical;
                        violations.push(format!(
                            "Auto bet p95: {:.0}ms (target: {}ms)",
                            metric.p95_duration_ms, self.targets.auto_bet_ms
                        ));
                    }
                }
                _ => {}
            }
        }

        PerformanceReport { health, violations }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance health status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerformanceHealth {
    Healthy,
    Degraded,
    Critical,
}

/// Performance report
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub health: PerformanceHealth,
    pub violations: Vec<String>,
}

/// Timer guard for automatic timing
pub struct TimerGuard {
    name: String,
    start: Instant,
    monitor: Arc<PerformanceMonitor>,
}

impl TimerGuard {
    pub fn new(name: &str, monitor: Arc<PerformanceMonitor>) -> Self {
        Self {
            name: name.to_string(),
            start: Instant::now(),
            monitor,
        }
    }

    pub async fn finish(self, success: bool) {
        let duration = self.start.elapsed();
        let duration_ms = duration.as_secs_f64() * 1000.0;
        self.monitor.record(&self.name, duration_ms, success).await;
    }
}

/// Global performance monitor instance
use std::sync::OnceLock;

static GLOBAL_MONITOR: OnceLock<Arc<PerformanceMonitor>> = OnceLock::new();

pub fn init_global_monitor(targets: PerformanceTargets) -> Arc<PerformanceMonitor> {
    let monitor = Arc::new(PerformanceMonitor::with_targets(targets));
    GLOBAL_MONITOR.set(monitor.clone()).ok();
    monitor
}

pub fn get_global_monitor() -> Option<Arc<PerformanceMonitor>> {
    GLOBAL_MONITOR.get().cloned()
}

/// Macro for timing operations
#[macro_export]
macro_rules! time_op {
    ($name:expr, $block:block) => {
        {
            let _monitor = $crate::performance::get_global_monitor();
            let _start = std::time::Instant::now();
            let _result = $block;
            let _duration = _start.elapsed();
            let _duration_ms = _duration.as_secs_f64() * 1000.0;

            if let Some(m) = _monitor {
                let _ = m.record($name, _duration_ms, true);
            }

            _result
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new();

        // Record some operations
        monitor.record("test_op", 100.0, true).await;
        monitor.record("test_op", 200.0, true).await;
        monitor.record("test_op", 150.0, false).await;

        let metric = monitor.get_metric("test_op").await.unwrap();
        assert_eq!(metric.count, 3);
        assert_eq!(metric.errors, 1);
        assert!(metric.success_rate < 100.0);
    }

    #[test]
    fn test_performance_targets() {
        let targets = PerformanceTargets::default();
        assert_eq!(targets.scan_cycle_ms, 500);
        assert_eq!(targets.fork_to_display_ms, 1000);
        assert_eq!(targets.auto_bet_ms, 5000);
    }
}
