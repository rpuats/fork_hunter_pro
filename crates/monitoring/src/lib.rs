//! Comprehensive Monitoring System for Fork Hunter
//! 
//! This module provides:
//! - Real-time metrics collection (events/sec, latency, errors)
//! - Health dashboards per parser
//! - Alert thresholds and notifications
//! - Historical trends tracking (24 hours)
//! - Anomaly detection using statistical analysis

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Monitoring system errors
#[derive(Error, Debug)]
pub enum MonitoringError {
    #[error("Parser not found: {0}")]
    ParserNotFound(String),
    
    #[error("Invalid threshold: {0}")]
    InvalidThreshold(String),
    
    #[error("Insufficient data for analysis: {0}")]
    InsufficientData(String),
    
    #[error("Metric collection failed: {0}")]
    CollectionFailed(String),
    
    #[error("Anomaly detection error: {0}")]
    AnomalyDetectionError(String),
}

pub type MonitoringResult<T> = Result<T, MonitoringError>;

/// Real-time metric snapshot
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricSnapshot {
    /// Events processed per second
    pub events_per_sec: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Total errors in current period
    pub error_count: u64,
    /// Error rate as percentage (0-100)
    pub error_rate: f64,
    /// Timestamp of snapshot
    pub timestamp: DateTime<Utc>,
    /// P50 latency in milliseconds
    pub p50_latency_ms: f64,
    /// P95 latency in milliseconds
    pub p95_latency_ms: f64,
    /// P99 latency in milliseconds
    pub p99_latency_ms: f64,
}

impl Default for MetricSnapshot {
    fn default() -> Self {
        Self {
            events_per_sec: 0.0,
            avg_latency_ms: 0.0,
            error_count: 0,
            error_rate: 0.0,
            timestamp: Utc::now(),
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
        }
    }
}

/// Individual event metric
#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub struct EventMetric {
    pub latency_ms: f64,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
}

/// Parser health status
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Offline,
}

/// Parser health dashboard
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParserHealthDashboard {
    /// Parser name/identifier
    pub parser_name: String,
    /// Current health status
    pub health_status: HealthStatus,
    /// Current metrics
    pub current_metrics: MetricSnapshot,
    /// Uptime percentage in last 24 hours
    pub uptime_percent: f64,
    /// Events processed in last 24 hours
    pub events_24h: u64,
    /// Average accuracy in last 24 hours
    pub accuracy_24h: f64,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
    /// Active alerts for this parser
    pub active_alerts: Vec<Alert>,
}

/// Alert severity levels
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Alert configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlertThreshold {
    pub accuracy_min: f64,      // Minimum accuracy percentage (0-100)
    pub latency_max_ms: f64,    // Maximum acceptable latency
    pub error_rate_max: f64,    // Maximum error rate (0-100)
    pub uptime_min_percent: f64, // Minimum uptime percentage
}

impl Default for AlertThreshold {
    fn default() -> Self {
        Self {
            accuracy_min: 95.0,
            latency_max_ms: 5000.0,
            error_rate_max: 5.0,
            uptime_min_percent: 95.0,
        }
    }
}

/// Alert notification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub parser_name: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub metric_value: f64,
    pub threshold_value: f64,
}

/// Types of alerts
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AlertType {
    AccuracyLow,
    LatencyHigh,
    ErrorRateHigh,
    UptimeLow,
    ParserOffline,
    AnomalyDetected,
}

/// Historical metric point
#[derive(Clone, Debug, Serialize, Deserialize, Copy)]
pub struct HistoricalPoint {
    pub timestamp: DateTime<Utc>,
    pub events_per_sec: f64,
    pub avg_latency_ms: f64,
    pub error_rate: f64,
    pub accuracy: f64,
}

/// Historical trend data (24 hours)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoricalTrend {
    pub parser_name: String,
    pub points: Vec<HistoricalPoint>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

impl HistoricalTrend {
    /// Calculate trend direction (positive, negative, stable)
    pub fn trend_direction(&self) -> TrendDirection {
        if self.points.len() < 2 {
            return TrendDirection::Stable;
        }
        
        let first = self.points.first().unwrap();
        let last = self.points.last().unwrap();
        let diff = last.error_rate - first.error_rate;
        
        if diff > 0.5 {
            TrendDirection::Negative
        } else if diff < -0.5 {
            TrendDirection::Positive
        } else {
            TrendDirection::Stable
        }
    }
}

/// Trend direction indicator
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TrendDirection {
    Positive,
    Negative,
    Stable,
}

/// Anomaly detection result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnomalyResult {
    pub is_anomaly: bool,
    pub anomaly_score: f64,  // 0.0 to 1.0
    pub confidence: f64,      // 0.0 to 1.0
    pub reason: String,
}

/// Statistical anomaly detector using Z-score and IQR methods
#[derive(Clone)]
pub struct AnomalyDetector {
    z_score_threshold: f64,
    iqr_multiplier: f64,
    min_data_points: usize,
}

impl AnomalyDetector {
    pub fn new(z_score_threshold: f64, iqr_multiplier: f64) -> Self {
        Self {
            z_score_threshold,
            iqr_multiplier,
            min_data_points: 10,
        }
    }

    /// Detect anomalies using Z-score method
    pub fn detect_anomaly(&self, value: f64, historical_values: &[f64]) -> AnomalyResult {
        if historical_values.len() < self.min_data_points {
            return AnomalyResult {
                is_anomaly: false,
                anomaly_score: 0.0,
                confidence: 0.0,
                reason: "Insufficient data points".to_string(),
            };
        }

        let mean = historical_values.iter().sum::<f64>() / historical_values.len() as f64;
        let variance = historical_values
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / historical_values.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return AnomalyResult {
                is_anomaly: false,
                anomaly_score: 0.0,
                confidence: 0.5,
                reason: "No variation in data".to_string(),
            };
        }

        let z_score = (value - mean).abs() / std_dev;
        let is_anomaly = z_score > self.z_score_threshold;
        let anomaly_score = (z_score / (self.z_score_threshold * 2.0)).min(1.0);
        let confidence = (z_score / 3.0).min(1.0);

        AnomalyResult {
            is_anomaly,
            anomaly_score,
            confidence,
            reason: format!("Z-score: {:.2} (threshold: {:.2})", z_score, self.z_score_threshold),
        }
    }

    /// Detect anomalies using IQR method
    pub fn detect_anomaly_iqr(&self, value: f64, historical_values: &[f64]) -> AnomalyResult {
        if historical_values.len() < self.min_data_points {
            return AnomalyResult {
                is_anomaly: false,
                anomaly_score: 0.0,
                confidence: 0.0,
                reason: "Insufficient data points".to_string(),
            };
        }

        let mut sorted = historical_values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let q1_idx = sorted.len() / 4;
        let q3_idx = (sorted.len() * 3) / 4;
        let q1 = sorted[q1_idx];
        let q3 = sorted[q3_idx];
        let iqr = q3 - q1;

        let lower_bound = q1 - (self.iqr_multiplier * iqr);
        let upper_bound = q3 + (self.iqr_multiplier * iqr);

        let is_anomaly = value < lower_bound || value > upper_bound;
        let distance = if value < lower_bound {
            (lower_bound - value) / iqr.max(1.0)
        } else if value > upper_bound {
            (value - upper_bound) / iqr.max(1.0)
        } else {
            0.0
        };
        let anomaly_score = (distance / 3.0).min(1.0);

        AnomalyResult {
            is_anomaly,
            anomaly_score,
            confidence: is_anomaly as i32 as f64 * 0.9 + 0.1,
            reason: format!("IQR bounds: [{:.2}, {:.2}], value: {:.2}", lower_bound, upper_bound, value),
        }
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new(2.5, 1.5)
    }
}

/// Per-parser metric collector
struct ParserMetrics {
    parser_name: String,
    recent_events: Vec<EventMetric>,
    alert_threshold: AlertThreshold,
    active_alerts: Vec<Alert>,
    historical_data: Vec<HistoricalPoint>,
    anomaly_detector: AnomalyDetector,
    last_accuracy: f64,
}

impl ParserMetrics {
    fn new(parser_name: String, threshold: AlertThreshold) -> Self {
        Self {
            parser_name,
            recent_events: Vec::new(),
            alert_threshold: threshold,
            active_alerts: Vec::new(),
            historical_data: Vec::new(),
            anomaly_detector: AnomalyDetector::default(),
            last_accuracy: 100.0,
        }
    }

    fn record_event(&mut self, metric: EventMetric) {
        self.recent_events.push(metric);
        // Keep only last 10000 events in memory
        if self.recent_events.len() > 10000 {
            self.recent_events.remove(0);
        }
    }

    fn calculate_current_metrics(&self) -> MetricSnapshot {
        if self.recent_events.is_empty() {
            return MetricSnapshot::default();
        }

        let now = Utc::now();
        let one_minute_ago = now - Duration::minutes(1);

        let recent: Vec<_> = self
            .recent_events
            .iter()
            .filter(|e| e.timestamp > one_minute_ago)
            .collect();

        if recent.is_empty() {
            return MetricSnapshot::default();
        }

        let events_count = recent.len() as f64;
        let events_per_sec = events_count / 60.0;

        let avg_latency = recent.iter().map(|e| e.latency_ms).sum::<f64>() / events_count;

        let error_count = recent.iter().filter(|e| !e.success).count() as u64;
        let error_rate = (error_count as f64 / events_count) * 100.0;

        // Calculate percentiles
        let mut latencies: Vec<_> = recent.iter().map(|e| e.latency_ms).collect();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p50_idx = latencies.len() / 2;
        let p95_idx = (latencies.len() * 95) / 100;
        let p99_idx = (latencies.len() * 99) / 100;

        MetricSnapshot {
            events_per_sec,
            avg_latency_ms: avg_latency,
            error_count,
            error_rate,
            timestamp: now,
            p50_latency_ms: latencies.get(p50_idx).copied().unwrap_or(0.0),
            p95_latency_ms: latencies.get(p95_idx).copied().unwrap_or(0.0),
            p99_latency_ms: latencies.get(p99_idx).copied().unwrap_or(0.0),
        }
    }

    fn add_historical_point(&mut self, point: HistoricalPoint) {
        self.historical_data.push(point);
        // Keep only 24 hours of data
        let cutoff = point.timestamp - Duration::hours(24);
        self.historical_data.retain(|p| p.timestamp > cutoff);
    }

    fn get_health_status(&self) -> HealthStatus {
        if self.recent_events.is_empty() {
            return HealthStatus::Offline;
        }

        let metrics = self.calculate_current_metrics();

        if metrics.error_rate > self.alert_threshold.error_rate_max * 2.0 {
            HealthStatus::Critical
        } else if metrics.avg_latency_ms > self.alert_threshold.latency_max_ms * 1.5 {
            HealthStatus::Critical
        } else if metrics.error_rate > self.alert_threshold.error_rate_max
            || metrics.avg_latency_ms > self.alert_threshold.latency_max_ms
        {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    fn check_thresholds(&mut self) -> Vec<Alert> {
        let metrics = self.calculate_current_metrics();
        let mut new_alerts = Vec::new();

        // Check accuracy threshold
        if self.last_accuracy < self.alert_threshold.accuracy_min {
            new_alerts.push(Alert {
                id: format!("{}-accuracy-{}", self.parser_name, Utc::now().timestamp()),
                parser_name: self.parser_name.clone(),
                alert_type: AlertType::AccuracyLow,
                severity: AlertSeverity::Critical,
                message: format!(
                    "Accuracy {:.2}% below threshold {:.2}%",
                    self.last_accuracy, self.alert_threshold.accuracy_min
                ),
                triggered_at: Utc::now(),
                resolved_at: None,
                metric_value: self.last_accuracy,
                threshold_value: self.alert_threshold.accuracy_min,
            });
        }

        // Check latency threshold
        if metrics.avg_latency_ms > self.alert_threshold.latency_max_ms {
            new_alerts.push(Alert {
                id: format!("{}-latency-{}", self.parser_name, Utc::now().timestamp()),
                parser_name: self.parser_name.clone(),
                alert_type: AlertType::LatencyHigh,
                severity: AlertSeverity::Warning,
                message: format!(
                    "Average latency {:.2}ms exceeds threshold {:.2}ms",
                    metrics.avg_latency_ms, self.alert_threshold.latency_max_ms
                ),
                triggered_at: Utc::now(),
                resolved_at: None,
                metric_value: metrics.avg_latency_ms,
                threshold_value: self.alert_threshold.latency_max_ms,
            });
        }

        // Check error rate threshold
        if metrics.error_rate > self.alert_threshold.error_rate_max {
            new_alerts.push(Alert {
                id: format!("{}-errors-{}", self.parser_name, Utc::now().timestamp()),
                parser_name: self.parser_name.clone(),
                alert_type: AlertType::ErrorRateHigh,
                severity: AlertSeverity::Warning,
                message: format!(
                    "Error rate {:.2}% exceeds threshold {:.2}%",
                    metrics.error_rate, self.alert_threshold.error_rate_max
                ),
                triggered_at: Utc::now(),
                resolved_at: None,
                metric_value: metrics.error_rate,
                threshold_value: self.alert_threshold.error_rate_max,
            });
        }

        self.active_alerts.extend(new_alerts.clone());
        new_alerts
    }
}

/// Main monitoring system
pub struct Monitor {
    parsers: Arc<DashMap<String, Arc<RwLock<ParserMetrics>>>>,
    global_anomaly_detector: AnomalyDetector,
}

impl Monitor {
    /// Create a new monitor instance
    pub fn new() -> Self {
        Self {
            parsers: Arc::new(DashMap::new()),
            global_anomaly_detector: AnomalyDetector::default(),
        }
    }

    /// Register a new parser for monitoring
    pub fn register_parser(&self, parser_name: String) -> MonitoringResult<()> {
        self.register_parser_with_threshold(parser_name, AlertThreshold::default())
    }

    /// Register a parser with custom alert thresholds
    pub fn register_parser_with_threshold(
        &self,
        parser_name: String,
        threshold: AlertThreshold,
    ) -> MonitoringResult<()> {
        self.parsers.insert(
            parser_name.clone(),
            Arc::new(RwLock::new(ParserMetrics::new(parser_name, threshold))),
        );
        Ok(())
    }

    /// Record an event metric
    pub fn record_event(
        &self,
        parser_name: &str,
        latency_ms: f64,
        success: bool,
    ) -> MonitoringResult<()> {
        let parser = self
            .parsers
            .get_mut(parser_name)
            .ok_or_else(|| MonitoringError::ParserNotFound(parser_name.to_string()))?;

        let metric = EventMetric {
            latency_ms,
            success,
            timestamp: Utc::now(),
        };

        parser.write().record_event(metric);
        Ok(())
    }

    /// Get current metrics for a parser
    pub fn get_current_metrics(&self, parser_name: &str) -> MonitoringResult<MetricSnapshot> {
        let parser = self
            .parsers
            .get(parser_name)
            .ok_or_else(|| MonitoringError::ParserNotFound(parser_name.to_string()))?;

        Ok(parser.read().calculate_current_metrics())
    }

    /// Get health dashboard for a parser
    pub fn get_health_dashboard(&self, parser_name: &str) -> MonitoringResult<ParserHealthDashboard> {
        let parser_ref = self
            .parsers
            .get_mut(parser_name)
            .ok_or_else(|| MonitoringError::ParserNotFound(parser_name.to_string()))?;

        let mut parser = parser_ref.write();
        let _ = parser.check_thresholds();

        let current_metrics = parser.calculate_current_metrics();
        let health_status = parser.get_health_status();

        // Calculate 24h stats
        let now = Utc::now();
        let day_ago = now - Duration::hours(24);
        let day_events: Vec<_> = parser
            .recent_events
            .iter()
            .filter(|e| e.timestamp > day_ago)
            .collect();

        let events_24h = day_events.len() as u64;
        let successful_24h = day_events.iter().filter(|e| e.success).count();
        let accuracy_24h = if events_24h > 0 {
            (successful_24h as f64 / events_24h as f64) * 100.0
        } else {
            100.0
        };

        parser.last_accuracy = accuracy_24h;

        let uptime_percent = if events_24h > 0 {
            (successful_24h as f64 / events_24h as f64) * 100.0
        } else {
            100.0
        };

        Ok(ParserHealthDashboard {
            parser_name: parser.parser_name.clone(),
            health_status,
            current_metrics,
            uptime_percent,
            events_24h,
            accuracy_24h,
            last_updated: now,
            active_alerts: parser.active_alerts.clone(),
        })
    }

    /// Get all active alerts
    pub fn get_all_alerts(&self) -> Vec<Alert> {
        let mut all_alerts = Vec::new();
        for parser_ref in self.parsers.iter() {
            let parser = parser_ref.value().read();
            all_alerts.extend(parser.active_alerts.clone());
        }
        all_alerts.sort_by(|a, b| b.severity.cmp(&a.severity));
        all_alerts
    }

    /// Get alerts for a specific parser
    pub fn get_parser_alerts(&self, parser_name: &str) -> MonitoringResult<Vec<Alert>> {
        let parser = self
            .parsers
            .get(parser_name)
            .ok_or_else(|| MonitoringError::ParserNotFound(parser_name.to_string()))?;

        Ok(parser.read().active_alerts.clone())
    }

    /// Clear resolved alerts (older than duration)
    pub fn clear_old_alerts(&self, parser_name: &str, duration: Duration) -> MonitoringResult<()> {
        let parser = self
            .parsers
            .get_mut(parser_name)
            .ok_or_else(|| MonitoringError::ParserNotFound(parser_name.to_string()))?;

        let cutoff = Utc::now() - duration;
        parser.write().active_alerts.retain(|alert| {
            alert.triggered_at > cutoff
        });

        Ok(())
    }

    /// Get historical trends for a parser
    pub fn get_historical_trend(&self, parser_name: &str) -> MonitoringResult<HistoricalTrend> {
        let parser = self
            .parsers
            .get(parser_name)
            .ok_or_else(|| MonitoringError::ParserNotFound(parser_name.to_string()))?;

        let parser_lock = parser.read();
        let points = parser_lock.historical_data.clone();

        if points.is_empty() {
            return Err(MonitoringError::InsufficientData(
                "No historical data available".to_string(),
            ));
        }

        Ok(HistoricalTrend {
            parser_name: parser_name.to_string(),
            start_time: points.first().map(|p| p.timestamp).unwrap_or_else(Utc::now),
            end_time: points.last().map(|p| p.timestamp).unwrap_or_else(Utc::now),
            points,
        })
    }

    /// Update historical data (should be called periodically)
    pub fn update_historical_data(&self, parser_name: &str) -> MonitoringResult<()> {
        let parser = self
            .parsers
            .get_mut(parser_name)
            .ok_or_else(|| MonitoringError::ParserNotFound(parser_name.to_string()))?;

        let mut parser_lock = parser.write();
        let metrics = parser_lock.calculate_current_metrics();

        let point = HistoricalPoint {
            timestamp: Utc::now(),
            events_per_sec: metrics.events_per_sec,
            avg_latency_ms: metrics.avg_latency_ms,
            error_rate: metrics.error_rate,
            accuracy: 100.0 - metrics.error_rate,
        };

        parser_lock.add_historical_point(point);
        Ok(())
    }

    /// Detect anomalies in parser metrics
    pub fn detect_anomaly(&self, parser_name: &str) -> MonitoringResult<AnomalyResult> {
        let parser = self
            .parsers
            .get(parser_name)
            .ok_or_else(|| MonitoringError::ParserNotFound(parser_name.to_string()))?;

        let parser_lock = parser.read();
        if parser_lock.historical_data.len() < 10 {
            return Err(MonitoringError::InsufficientData(
                "Need at least 10 historical points for anomaly detection".to_string(),
            ));
        }

        let current_metrics = parser_lock.calculate_current_metrics();
        let historical_errors: Vec<f64> = parser_lock
            .historical_data
            .iter()
            .map(|p| p.error_rate)
            .collect();

        Ok(parser_lock
            .anomaly_detector
            .detect_anomaly(current_metrics.error_rate, &historical_errors))
    }

    /// Get comprehensive system dashboard
    pub fn get_system_dashboard(&self) -> Vec<ParserHealthDashboard> {
        let mut dashboards = Vec::new();
        for parser_ref in self.parsers.iter() {
            if let Ok(dashboard) = self.get_health_dashboard(&parser_ref.key()) {
                dashboards.push(dashboard);
            }
        }
        dashboards
    }

    /// Get system-wide statistics
    pub fn get_system_stats(&self) -> SystemStats {
        let dashboards = self.get_system_dashboard();
        let all_alerts = self.get_all_alerts();

        let healthy_count = dashboards
            .iter()
            .filter(|d| d.health_status == HealthStatus::Healthy)
            .count();
        let degraded_count = dashboards
            .iter()
            .filter(|d| d.health_status == HealthStatus::Degraded)
            .count();
        let critical_count = dashboards
            .iter()
            .filter(|d| d.health_status == HealthStatus::Critical)
            .count();
        let offline_count = dashboards
            .iter()
            .filter(|d| d.health_status == HealthStatus::Offline)
            .count();

        let avg_uptime = if !dashboards.is_empty() {
            dashboards.iter().map(|d| d.uptime_percent).sum::<f64>() / dashboards.len() as f64
        } else {
            0.0
        };

        let avg_latency = if !dashboards.is_empty() {
            dashboards.iter().map(|d| d.current_metrics.avg_latency_ms).sum::<f64>()
                / dashboards.len() as f64
        } else {
            0.0
        };

        let total_events = dashboards.iter().map(|d| d.events_24h).sum::<u64>();

        SystemStats {
            timestamp: Utc::now(),
            total_parsers: dashboards.len(),
            healthy_count,
            degraded_count,
            critical_count,
            offline_count,
            avg_uptime,
            avg_latency_ms: avg_latency,
            total_events_24h: total_events,
            active_alerts: all_alerts.len(),
            critical_alerts: all_alerts
                .iter()
                .filter(|a| a.severity == AlertSeverity::Critical)
                .count(),
        }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

/// System-wide statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemStats {
    pub timestamp: DateTime<Utc>,
    pub total_parsers: usize,
    pub healthy_count: usize,
    pub degraded_count: usize,
    pub critical_count: usize,
    pub offline_count: usize,
    pub avg_uptime: f64,
    pub avg_latency_ms: f64,
    pub total_events_24h: u64,
    pub active_alerts: usize,
    pub critical_alerts: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_creation() {
        let monitor = Monitor::new();
        assert_eq!(monitor.parsers.len(), 0);
    }

    #[test]
    fn test_register_parser() {
        let monitor = Monitor::new();
        assert!(monitor.register_parser("pari".to_string()).is_ok());
        assert_eq!(monitor.parsers.len(), 1);
    }

    #[test]
    fn test_register_multiple_parsers() {
        let monitor = Monitor::new();
        assert!(monitor.register_parser("pari".to_string()).is_ok());
        assert!(monitor.register_parser("marathon".to_string()).is_ok());
        assert!(monitor.register_parser("betcity".to_string()).is_ok());
        assert_eq!(monitor.parsers.len(), 3);
    }

    #[test]
    fn test_record_event() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        assert!(monitor.record_event("pari", 100.0, true).is_ok());
        assert!(monitor.record_event("pari", 150.0, true).is_ok());
        assert!(monitor.record_event("pari", 200.0, false).is_ok());
    }

    #[test]
    fn test_record_event_unregistered_parser() {
        let monitor = Monitor::new();
        let result = monitor.record_event("unknown", 100.0, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_current_metrics() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        monitor.record_event("pari", 100.0, true).unwrap();
        monitor.record_event("pari", 200.0, true).unwrap();
        monitor.record_event("pari", 150.0, false).unwrap();
        
        let metrics = monitor.get_current_metrics("pari").unwrap();
        assert!(metrics.events_per_sec > 0.0);
        assert!(metrics.avg_latency_ms > 0.0);
        assert_eq!(metrics.error_count, 1);
    }

    #[test]
    fn test_health_status_healthy() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        for _ in 0..100 {
            monitor.record_event("pari", 100.0, true).unwrap();
        }
        
        let dashboard = monitor.get_health_dashboard("pari").unwrap();
        assert_eq!(dashboard.health_status, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_status_degraded() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        for i in 0..100 {
            let success = i % 5 != 0; // 80% success rate
            monitor.record_event("pari", 6000.0, success).unwrap();
        }
        
        let dashboard = monitor.get_health_dashboard("pari").unwrap();
        assert_eq!(dashboard.health_status, HealthStatus::Degraded);
    }

    #[test]
    fn test_health_status_offline() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        let dashboard = monitor.get_health_dashboard("pari").unwrap();
        assert_eq!(dashboard.health_status, HealthStatus::Offline);
    }

    #[test]
    fn test_alert_thresholds() {
        let monitor = Monitor::new();
        let threshold = AlertThreshold {
            accuracy_min: 95.0,
            latency_max_ms: 1000.0,
            error_rate_max: 5.0,
            uptime_min_percent: 95.0,
        };
        
        monitor.register_parser_with_threshold("pari".to_string(), threshold).unwrap();
        
        // Add events that exceed latency threshold
        for _ in 0..50 {
            monitor.record_event("pari", 2000.0, true).unwrap();
        }
        
        let dashboard = monitor.get_health_dashboard("pari").unwrap();
        assert!(!dashboard.active_alerts.is_empty());
    }

    #[test]
    fn test_get_all_alerts() {
        let monitor = Monitor::new();
        let threshold = AlertThreshold {
            accuracy_min: 95.0,
            latency_max_ms: 500.0,
            error_rate_max: 5.0,
            uptime_min_percent: 95.0,
        };
        
        monitor.register_parser_with_threshold("pari".to_string(), threshold).unwrap();
        
        for _ in 0..50 {
            monitor.record_event("pari", 2000.0, true).unwrap();
        }
        
        let _ = monitor.get_health_dashboard("pari").unwrap();
        let alerts = monitor.get_all_alerts();
        assert!(!alerts.is_empty());
    }

    #[test]
    fn test_anomaly_detector_basic() {
        let detector = AnomalyDetector::default();
        let historical = vec![100.0, 101.0, 102.0, 101.0, 100.0, 99.0, 101.0, 100.0, 102.0, 101.0];
        let result = detector.detect_anomaly(500.0, &historical);
        assert!(result.is_anomaly);
    }

    #[test]
    fn test_anomaly_detector_no_anomaly() {
        let detector = AnomalyDetector::default();
        let historical = vec![100.0, 101.0, 102.0, 101.0, 100.0, 99.0, 101.0, 100.0, 102.0, 101.0];
        let result = detector.detect_anomaly(101.0, &historical);
        assert!(!result.is_anomaly);
    }

    #[test]
    fn test_anomaly_detector_iqr() {
        let detector = AnomalyDetector::default();
        let historical = vec![100.0, 101.0, 102.0, 101.0, 100.0, 99.0, 101.0, 100.0, 102.0, 101.0];
        let result = detector.detect_anomaly_iqr(1000.0, &historical);
        assert!(result.is_anomaly);
    }

    #[test]
    fn test_anomaly_detector_insufficient_data() {
        let detector = AnomalyDetector::default();
        let historical = vec![100.0, 101.0];
        let result = detector.detect_anomaly(500.0, &historical);
        assert!(!result.is_anomaly);
    }

    #[test]
    fn test_historical_trend() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        for i in 0..20 {
            let success = i % 3 != 0;
            monitor.record_event("pari", 100.0 + (i as f64), success).unwrap();
        }
        
        for _ in 0..10 {
            monitor.update_historical_data("pari").unwrap();
        }
        
        let trend = monitor.get_historical_trend("pari").unwrap();
        assert!(!trend.points.is_empty());
        assert!(!trend.trend_direction().to_string().is_empty());
    }

    #[test]
    fn test_trend_direction_positive() {
        let points = vec![
            HistoricalPoint {
                timestamp: Utc::now() - Duration::hours(1),
                events_per_sec: 100.0,
                avg_latency_ms: 100.0,
                error_rate: 5.0,
                accuracy: 95.0,
            },
            HistoricalPoint {
                timestamp: Utc::now(),
                events_per_sec: 100.0,
                avg_latency_ms: 100.0,
                error_rate: 2.0,
                accuracy: 98.0,
            },
        ];
        
        let trend = HistoricalTrend {
            parser_name: "test".to_string(),
            start_time: points[0].timestamp,
            end_time: points[1].timestamp,
            points,
        };
        
        assert_eq!(
            format!("{:?}", trend.trend_direction()),
            "Positive"
        );
    }

    #[test]
    fn test_metric_snapshot_default() {
        let snapshot = MetricSnapshot::default();
        assert_eq!(snapshot.events_per_sec, 0.0);
        assert_eq!(snapshot.avg_latency_ms, 0.0);
        assert_eq!(snapshot.error_count, 0);
    }

    #[test]
    fn test_system_dashboard() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        monitor.register_parser("marathon".to_string()).unwrap();
        
        for _ in 0..50 {
            monitor.record_event("pari", 100.0, true).unwrap();
            monitor.record_event("marathon", 150.0, true).unwrap();
        }
        
        let dashboards = monitor.get_system_dashboard();
        assert_eq!(dashboards.len(), 2);
    }

    #[test]
    fn test_system_stats() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        monitor.register_parser("marathon".to_string()).unwrap();
        
        for _ in 0..50 {
            monitor.record_event("pari", 100.0, true).unwrap();
            monitor.record_event("marathon", 150.0, true).unwrap();
        }
        
        let stats = monitor.get_system_stats();
        assert_eq!(stats.total_parsers, 2);
        assert!(stats.total_events_24h > 0);
    }

    #[test]
    fn test_clear_old_alerts() {
        let monitor = Monitor::new();
        let threshold = AlertThreshold {
            accuracy_min: 50.0,  // Very low to trigger alerts
            latency_max_ms: 1.0,
            error_rate_max: 1.0,
            uptime_min_percent: 50.0,
        };
        
        monitor.register_parser_with_threshold("pari".to_string(), threshold).unwrap();
        
        for _ in 0..50 {
            monitor.record_event("pari", 2000.0, true).unwrap();
        }
        
        let _ = monitor.get_health_dashboard("pari").unwrap();
        assert!(monitor.clear_old_alerts("pari", Duration::seconds(0)).is_ok());
    }

    #[test]
    fn test_error_rate_calculation() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        // Add 10 successful and 10 failed events
        for _ in 0..10 {
            monitor.record_event("pari", 100.0, true).unwrap();
            monitor.record_event("pari", 100.0, false).unwrap();
        }
        
        let metrics = monitor.get_current_metrics("pari").unwrap();
        assert!(metrics.error_rate >= 45.0 && metrics.error_rate <= 55.0);
    }

    #[test]
    fn test_percentile_calculation() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        // Add events with varying latencies
        for i in 0..100 {
            monitor.record_event("pari", (i as f64) * 10.0, true).unwrap();
        }
        
        let metrics = monitor.get_current_metrics("pari").unwrap();
        assert!(metrics.p50_latency_ms > 0.0);
        assert!(metrics.p95_latency_ms > metrics.p50_latency_ms);
        assert!(metrics.p99_latency_ms >= metrics.p95_latency_ms);
    }

    #[test]
    fn test_parser_metrics_memory_limit() {
        let monitor = Monitor::new();
        monitor.register_parser("pari".to_string()).unwrap();
        
        // Add more than 10000 events
        for _ in 0..15000 {
            monitor.record_event("pari", 100.0, true).unwrap();
        }
        
        let parser = monitor.parsers.get("pari").unwrap();
        assert!(parser.read().recent_events.len() <= 10000);
    }

    #[test]
    fn test_alert_type_variants() {
        let alert = Alert {
            id: "test".to_string(),
            parser_name: "pari".to_string(),
            alert_type: AlertType::AccuracyLow,
            severity: AlertSeverity::Critical,
            message: "Test".to_string(),
            triggered_at: Utc::now(),
            resolved_at: None,
            metric_value: 50.0,
            threshold_value: 95.0,
        };
        
        assert_eq!(
            format!("{:?}", alert.alert_type),
            "AccuracyLow"
        );
    }
}
