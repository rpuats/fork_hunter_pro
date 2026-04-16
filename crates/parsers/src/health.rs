use crate::base::BookmakerParser;
use shared::ParserHealth;
use std::sync::Arc;
use tracing::warn;

#[derive(Clone)]
pub struct HealthChecker {
    #[allow(dead_code)]
    check_interval_secs: u64,
    #[allow(dead_code)]
    max_consecutive_failures: u32,
}

impl HealthChecker {
    pub fn new(check_interval_secs: u64, max_consecutive_failures: u32) -> Self {
        Self {
            check_interval_secs,
            max_consecutive_failures,
        }
    }

    pub async fn check_parser(&self, parser: Arc<dyn BookmakerParser>) -> ParserHealth {
        let start = std::time::Instant::now();
        let readiness = parser.readiness();
        let diagnostics = readiness
            .as_ref()
            .map(|item| item.checks.clone())
            .unwrap_or_default();
        match parser.fetch_all().await {
            Ok(result) => {
                let elapsed = start.elapsed().as_millis() as f64;
                let is_empty = result.is_empty();
                ParserHealth {
                    bookmaker: parser.slug().to_string(),
                    status: if is_empty {
                        shared::HealthStatus::Degraded
                    } else {
                        shared::HealthStatus::Healthy
                    },
                    last_success: Some(chrono::Utc::now()),
                    last_error: is_empty.then(|| {
                        "parser returned no events or odds during health check".to_string()
                    }),
                    consecutive_failures: 0,
                    avg_response_time_ms: elapsed,
                    events_parsed: result.events.len() as u64,
                    uptime_percent: if is_empty { 0.0 } else { 100.0 },
                    readiness,
                    diagnostics,
                }
            }
            Err(e) => {
                let elapsed = start.elapsed().as_millis() as f64;
                warn!(
                    parser = parser.slug(),
                    error = e.to_string(),
                    "Parser health check failed"
                );
                ParserHealth {
                    bookmaker: parser.slug().to_string(),
                    status: shared::HealthStatus::Unhealthy,
                    last_success: None,
                    last_error: Some(e.to_string()),
                    consecutive_failures: 1,
                    avg_response_time_ms: elapsed,
                    events_parsed: 0,
                    uptime_percent: 0.0,
                    readiness,
                    diagnostics,
                }
            }
        }
    }

    pub async fn check_all(&self, parsers: Vec<Arc<dyn BookmakerParser>>) -> Vec<ParserHealth> {
        let mut results = Vec::new();
        for parser in parsers {
            let health = self.check_parser(parser).await;
            results.push(health);
        }
        results
    }
}
