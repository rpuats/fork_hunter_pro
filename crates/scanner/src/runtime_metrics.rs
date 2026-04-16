use chrono::{DateTime, Utc};
use shared::{
    DiagnosticSeverity, ParserDiagnosticCheck, ParserResultStatus, ParserRuntimeSnapshot,
    RuntimeCircuitState,
};

#[derive(Debug, Clone)]
pub struct ParserRuntimeStats {
    bookmaker: String,
    last_attempt: Option<DateTime<Utc>>,
    last_success: Option<DateTime<Utc>>,
    last_error: Option<String>,
    last_result_status: ParserResultStatus,
    last_result_message: Option<String>,
    validation_checks: Vec<ParserDiagnosticCheck>,
    consecutive_failures: u32,
    avg_response_time_ms: f64,
    events_parsed: u64,
    odds_parsed: u64,
    total_runs: u64,
    successful_runs: u64,
}

impl ParserRuntimeStats {
    pub fn new(bookmaker: impl Into<String>) -> Self {
        Self {
            bookmaker: bookmaker.into(),
            last_attempt: None,
            last_success: None,
            last_error: None,
            last_result_status: ParserResultStatus::Failed,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 0.0,
            events_parsed: 0,
            odds_parsed: 0,
            total_runs: 0,
            successful_runs: 0,
        }
    }

    pub fn record_success(
        &mut self,
        timestamp: DateTime<Utc>,
        fetch_time_ms: f64,
        events_parsed: u64,
        odds_parsed: u64,
        result_status: ParserResultStatus,
        result_message: Option<String>,
        validation_checks: Vec<ParserDiagnosticCheck>,
    ) {
        self.total_runs += 1;
        self.successful_runs += 1;
        self.last_attempt = Some(timestamp);
        self.last_success = Some(timestamp);
        self.last_error = None;
        self.last_result_status = result_status;
        self.last_result_message = result_message;
        self.validation_checks = validation_checks;
        self.consecutive_failures = 0;
        self.events_parsed = events_parsed;
        self.odds_parsed = odds_parsed;
        self.avg_response_time_ms = rolling_average(
            self.avg_response_time_ms,
            self.total_runs - 1,
            fetch_time_ms,
        );
    }

    pub fn bookmaker(&self) -> &str {
        &self.bookmaker
    }

    pub fn events_parsed(&self) -> u64 {
        self.events_parsed
    }

    pub fn record_failure(&mut self, now: DateTime<Utc>, error: String, elapsed_ms: f64) {
        let validation_checks = vec![ParserDiagnosticCheck {
            code: "fetch_failure".into(),
            severity: DiagnosticSeverity::Fail,
            message: error.clone(),
        }];
        self.record_failed_result(
            now,
            error,
            elapsed_ms,
            Some("fetch failed".into()),
            validation_checks,
        );
    }

    pub fn record_rejected_result(
        &mut self,
        now: DateTime<Utc>,
        error: String,
        elapsed_ms: f64,
        validation_checks: Vec<ParserDiagnosticCheck>,
    ) {
        self.record_failed_result(
            now,
            error.clone(),
            elapsed_ms,
            Some(error),
            validation_checks,
        );
    }

    fn record_failed_result(
        &mut self,
        now: DateTime<Utc>,
        error: String,
        elapsed_ms: f64,
        last_result_message: Option<String>,
        validation_checks: Vec<ParserDiagnosticCheck>,
    ) {
        self.total_runs += 1;
        self.last_attempt = Some(now);
        self.last_error = Some(error);
        self.last_result_status = ParserResultStatus::Failed;
        self.last_result_message = last_result_message;
        self.validation_checks = validation_checks;
        self.consecutive_failures += 1;
        self.avg_response_time_ms =
            rolling_average(self.avg_response_time_ms, self.total_runs - 1, elapsed_ms);
    }

    pub fn snapshot(&self, circuit_state: RuntimeCircuitState) -> ParserRuntimeSnapshot {
        let uptime_percent = if self.total_runs == 0 {
            0.0
        } else {
            (self.successful_runs as f64 / self.total_runs as f64) * 100.0
        };

        ParserRuntimeSnapshot {
            bookmaker: self.bookmaker.clone(),
            last_attempt: self.last_attempt,
            last_success: self.last_success,
            last_error: self.last_error.clone(),
            last_result_status: self.last_result_status.clone(),
            last_result_message: self.last_result_message.clone(),
            validation_checks: self.validation_checks.clone(),
            consecutive_failures: self.consecutive_failures,
            avg_response_time_ms: self.avg_response_time_ms,
            events_parsed: self.events_parsed,
            odds_parsed: self.odds_parsed,
            uptime_percent,
            total_runs: self.total_runs,
            successful_runs: self.successful_runs,
            circuit_state,
        }
    }
}

fn rolling_average(current: f64, samples: u64, next: f64) -> f64 {
    if samples == 0 {
        return next;
    }

    ((current * samples as f64) + next) / (samples as f64 + 1.0)
}

#[cfg(test)]
mod tests {
    use super::ParserRuntimeStats;
    use chrono::Utc;
    use shared::{ParserResultStatus, RuntimeCircuitState};

    #[test]
    fn snapshot_computes_uptime_across_success_and_failure_runs() {
        let mut stats = ParserRuntimeStats::new("pari");
        let now = Utc::now();

        stats.record_success(
            now,
            40.0,
            120,
            360,
            ParserResultStatus::Healthy,
            None,
            Vec::new(),
        );
        stats.record_failure(now, "timeout".into(), 20.0);

        let snapshot = stats.snapshot(RuntimeCircuitState::HalfOpen);

        assert_eq!(snapshot.bookmaker, "pari");
        assert_eq!(snapshot.total_runs, 2);
        assert_eq!(snapshot.successful_runs, 1);
        assert_eq!(snapshot.events_parsed, 120);
        assert_eq!(snapshot.odds_parsed, 360);
        assert_eq!(snapshot.last_error.as_deref(), Some("timeout"));
        assert_eq!(snapshot.last_result_status, ParserResultStatus::Failed);
        assert_eq!(snapshot.consecutive_failures, 1);
        assert!((snapshot.avg_response_time_ms - 30.0).abs() < f64::EPSILON);
        assert!((snapshot.uptime_percent - 50.0).abs() < f64::EPSILON);
        assert_eq!(snapshot.circuit_state, RuntimeCircuitState::HalfOpen);
    }

    #[test]
    fn success_resets_consecutive_failures_and_last_error() {
        let mut stats = ParserRuntimeStats::new("fonbet");
        let now = Utc::now();

        stats.record_failure(now, "boom".into(), 10.0);
        stats.record_success(
            now,
            30.0,
            45,
            120,
            ParserResultStatus::Healthy,
            None,
            Vec::new(),
        );

        let snapshot = stats.snapshot(RuntimeCircuitState::Closed);

        assert_eq!(snapshot.last_error, None);
        assert_eq!(snapshot.last_result_status, ParserResultStatus::Healthy);
        assert_eq!(snapshot.consecutive_failures, 0);
        assert_eq!(snapshot.events_parsed, 45);
        assert_eq!(snapshot.odds_parsed, 120);
        assert_eq!(snapshot.successful_runs, 1);
    }
}
