//! Parallel parser execution using tokio::join_all
//! Orchestrates concurrent fetching from multiple bookmakers

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures::future::join_all;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Parser execution configuration
#[derive(Debug, Clone)]
pub struct ParserExecutionConfig {
    pub max_concurrent_parsers: usize,
    pub request_timeout_ms: u64,
    pub retry_attempts: usize,
    pub enable_circuit_breaker: bool,
    pub circuit_breaker_threshold: f64,
}

impl Default for ParserExecutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_parsers: 16,
            request_timeout_ms: 30000,
            retry_attempts: 3,
            enable_circuit_breaker: true,
            circuit_breaker_threshold: 0.5,
        }
    }
}

/// Result of a parser execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserExecutionResult {
    pub parser_name: String,
    pub success: bool,
    pub events_count: usize,
    pub odds_count: usize,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub retry_count: usize,
}

/// Parser execution statistics
#[derive(Debug, Clone, Copy)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub avg_duration_ms: f64,
    pub total_events: usize,
    pub total_odds: usize,
}

impl ExecutionStats {
    pub fn success_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.successful_executions as f64 / self.total_executions as f64
        }
    }
}

/// Parallel parser executor
pub struct ParallelParserExecutor {
    config: RwLock<ParserExecutionConfig>,
    execution_history: DashMap<String, Vec<ParserExecutionResult>>,
    stats: RwLock<ExecutionStats>,
    active_tasks: RwLock<usize>,
}

impl ParallelParserExecutor {
    /// Create a new parallel parser executor
    pub fn new(config: ParserExecutionConfig) -> Self {
        info!(
            "Creating ParallelParserExecutor with max_concurrent_parsers={}",
            config.max_concurrent_parsers
        );

        Self {
            config: RwLock::new(config),
            execution_history: DashMap::new(),
            stats: RwLock::new(ExecutionStats {
                total_executions: 0,
                successful_executions: 0,
                failed_executions: 0,
                avg_duration_ms: 0.0,
                total_events: 0,
                total_odds: 0,
            }),
            active_tasks: RwLock::new(0),
        }
    }

    /// Execute multiple parsers in parallel
    pub async fn execute_parallel<F, Fut>(&self, tasks: Vec<(String, F)>) -> Vec<ParserExecutionResult>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<(usize, usize)>>,
    {
        let config = self.config.read().clone();
        let mut tasks_to_run = Vec::new();

        for (parser_name, task_fn) in tasks {
            let start = Instant::now();
            let parser_name_clone = parser_name.clone();

            tasks_to_run.push(async move {
                let result = match tokio::time::timeout(
                    std::time::Duration::from_millis(config.request_timeout_ms),
                    task_fn(),
                )
                .await
                {
                    Ok(Ok((events_count, odds_count))) => {
                        ParserExecutionResult {
                            parser_name: parser_name_clone.clone(),
                            success: true,
                            events_count,
                            odds_count,
                            duration_ms: start.elapsed().as_millis() as u64,
                            error: None,
                            timestamp: Utc::now(),
                            retry_count: 0,
                        }
                    }
                    Ok(Err(e)) => {
                        ParserExecutionResult {
                            parser_name: parser_name_clone.clone(),
                            success: false,
                            events_count: 0,
                            odds_count: 0,
                            duration_ms: start.elapsed().as_millis() as u64,
                            error: Some(e.to_string()),
                            timestamp: Utc::now(),
                            retry_count: 0,
                        }
                    }
                    Err(_) => {
                        ParserExecutionResult {
                            parser_name: parser_name_clone.clone(),
                            success: false,
                            events_count: 0,
                            odds_count: 0,
                            duration_ms: start.elapsed().as_millis() as u64,
                            error: Some("Timeout".to_string()),
                            timestamp: Utc::now(),
                            retry_count: 0,
                        }
                    }
                };

                result
            });
        }

        // Execute all tasks concurrently using join_all
        let results = join_all(tasks_to_run).await;

        // Update statistics
        {
            let mut stats = self.stats.write();
            let mut total_duration = 0u64;
            let mut total_events = 0;
            let mut total_odds = 0;

            for result in &results {
                stats.total_executions += 1;
                total_duration += result.duration_ms;
                total_events += result.events_count;
                total_odds += result.odds_count;

                if result.success {
                    stats.successful_executions += 1;
                } else {
                    stats.failed_executions += 1;
                }
            }

            stats.total_events += total_events;
            stats.total_odds += total_odds;
            stats.avg_duration_ms = total_duration as f64 / results.len() as f64;
        }

        // Record in history
        for result in &results {
            self.execution_history
                .entry(result.parser_name.clone())
                .or_insert_with(Vec::new)
                .push(result.clone());
        }

        info!(
            "Executed {} parsers with {} successes, {} failures",
            results.len(),
            results.iter().filter(|r| r.success).count(),
            results.iter().filter(|r| !r.success).count()
        );

        results
    }

    /// Execute parser with automatic retry
    pub async fn execute_with_retry<F, Fut>(&self, parser_name: String, task: F) -> ParserExecutionResult
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<(usize, usize)>>,
    {
        let config = self.config.read().clone();
        drop(config);

        for attempt in 0..self.config.read().retry_attempts {
            let start = Instant::now();

            let result = match tokio::time::timeout(
                std::time::Duration::from_millis(self.config.read().request_timeout_ms),
                task(),
            )
            .await
            {
                Ok(Ok((events_count, odds_count))) => {
                    return ParserExecutionResult {
                        parser_name: parser_name.clone(),
                        success: true,
                        events_count,
                        odds_count,
                        duration_ms: start.elapsed().as_millis() as u64,
                        error: None,
                        timestamp: Utc::now(),
                        retry_count: attempt,
                    }
                }
                Ok(Err(e)) => {
                    warn!(
                        "Parser {} failed on attempt {}: {}",
                        parser_name, attempt, e
                    );
                    e.to_string()
                }
                Err(_) => {
                    warn!("Parser {} timeout on attempt {}", parser_name, attempt);
                    "Timeout".to_string()
                }
            };

            if attempt < self.config.read().retry_attempts - 1 {
                tokio::time::sleep(std::time::Duration::from_millis(100 * (attempt as u64 + 1)))
                    .await;
            } else {
                return ParserExecutionResult {
                    parser_name: parser_name.clone(),
                    success: false,
                    events_count: 0,
                    odds_count: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error: Some(result),
                    timestamp: Utc::now(),
                    retry_count: attempt,
                };
            }
        }

        unreachable!()
    }

    /// Get execution statistics
    pub fn stats(&self) -> ExecutionStats {
        *self.stats.read()
    }

    /// Get execution history for a parser
    pub fn history(&self, parser_name: &str) -> Vec<ParserExecutionResult> {
        self.execution_history
            .get(parser_name)
            .map(|h| h.clone())
            .unwrap_or_default()
    }

    /// Clear execution history
    pub fn clear_history(&self) {
        self.execution_history.clear();
    }

    /// Get all parser names in history
    pub fn parser_names(&self) -> Vec<String> {
        self.execution_history
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get recent execution results (last N)
    pub fn recent_results(&self, limit: usize) -> Vec<ParserExecutionResult> {
        let mut all_results = Vec::new();
        
        for entry in self.execution_history.iter() {
            all_results.extend(entry.value().clone());
        }

        all_results.sort_by_key(|r| std::cmp::Reverse(r.timestamp));
        all_results.into_iter().take(limit).collect()
    }

    /// Get success rate for a parser
    pub fn parser_success_rate(&self, parser_name: &str) -> f64 {
        if let Some(results) = self.execution_history.get(parser_name) {
            let total = results.len();
            if total == 0 {
                0.0
            } else {
                let successful = results.iter().filter(|r| r.success).count();
                successful as f64 / total as f64
            }
        } else {
            0.0
        }
    }

    /// Get average duration for a parser
    pub fn parser_avg_duration(&self, parser_name: &str) -> f64 {
        if let Some(results) = self.execution_history.get(parser_name) {
            if results.is_empty() {
                0.0
            } else {
                let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();
                total_duration as f64 / results.len() as f64
            }
        } else {
            0.0
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write();
        *stats = ExecutionStats {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            avg_duration_ms: 0.0,
            total_events: 0,
            total_odds: 0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());
        assert_eq!(executor.stats().total_executions, 0);
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());

        let tasks = vec![
            ("parser1".to_string(), || async {
                Ok::<_, anyhow::Error>((10, 20))
            }),
            ("parser2".to_string(), || async {
                Ok::<_, anyhow::Error>((15, 25))
            }),
        ];

        let results = executor.execute_parallel(tasks).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
    }

    #[tokio::test]
    async fn test_execution_with_retry() {
        let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());

        let result = executor
            .execute_with_retry("test_parser".to_string(), || async {
                Ok::<_, anyhow::Error>((5, 10))
            })
            .await;

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_history_tracking() {
        let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());

        let tasks = vec![(
            "parser1".to_string(),
            || async { Ok::<_, anyhow::Error>((10, 20)) },
        )];

        executor.execute_parallel(tasks).await;

        let history = executor.history("parser1");
        assert_eq!(history.len(), 1);
    }
}
