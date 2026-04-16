use chrono::Utc;
use shared::{
    DiagnosticSeverity, HealthStatus, ParserCoverage, ParserDiagnosticCheck, ParserHealth,
    ParserResultStatus, ParserRuntimeSnapshot, RuntimeCircuitState,
};

pub const STATIC_PARSER_HEALTH_NOTE: &str =
    "Static factory snapshot only; runtime fetch has not been executed yet.";

fn parser_health_status(runtime: &ParserRuntimeSnapshot, fallback: &ParserHealth) -> HealthStatus {
    parser_health_status_with_freshness(runtime, fallback, Utc::now(), 0)
}

fn parser_health_status_with_freshness(
    runtime: &ParserRuntimeSnapshot,
    fallback: &ParserHealth,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> HealthStatus {
    match runtime.circuit_state {
        RuntimeCircuitState::Open => HealthStatus::CircuitOpen,
        RuntimeCircuitState::Closed | RuntimeCircuitState::HalfOpen => {
            if runtime.total_runs == 0 {
                return fallback.status.clone();
            }

            if runtime.is_stale(now, stale_after_secs) {
                return HealthStatus::Unhealthy;
            }

            if runtime.last_success.is_none()
                || matches!(runtime.last_result_status, ParserResultStatus::Failed)
            {
                HealthStatus::Unhealthy
            } else if runtime.consecutive_failures == 0
                && matches!(runtime.last_result_status, ParserResultStatus::Healthy)
            {
                HealthStatus::Healthy
            } else {
                HealthStatus::Degraded
            }
        }
    }
}

pub fn merge_parser_health(
    fallback: &ParserHealth,
    runtime: Option<&ParserRuntimeSnapshot>,
) -> ParserHealth {
    merge_parser_health_with_freshness(fallback, runtime, Utc::now(), 0)
}

fn merge_parser_health_with_freshness(
    fallback: &ParserHealth,
    runtime: Option<&ParserRuntimeSnapshot>,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> ParserHealth {
    let Some(runtime) = runtime else {
        return fallback.clone();
    };

    let has_live_runtime = runtime.last_attempt.is_some() || runtime.total_runs > 0;
    let diagnostics = if has_live_runtime {
        runtime_health_diagnostics(runtime, now, stale_after_secs)
            .into_iter()
            .collect()
    } else {
        let mut diagnostics = fallback.diagnostics.clone();
        diagnostics.extend(runtime_health_diagnostics(runtime, now, stale_after_secs));
        diagnostics
    };

    ParserHealth {
        bookmaker: fallback.bookmaker.clone(),
        status: parser_health_status_with_freshness(runtime, fallback, now, stale_after_secs),
        last_success: runtime.last_success.or(fallback.last_success),
        last_error: runtime.last_error.clone().or_else(|| {
            if has_live_runtime {
                None
            } else {
                fallback.last_error.clone()
            }
        }),
        consecutive_failures: runtime.consecutive_failures,
        avg_response_time_ms: runtime.avg_response_time_ms,
        events_parsed: runtime.events_parsed,
        uptime_percent: runtime.uptime_percent,
        readiness: fallback.readiness.clone(),
        diagnostics,
    }
}

pub fn runtime_only_parser_health(runtime: &ParserRuntimeSnapshot) -> ParserHealth {
    runtime_only_parser_health_with_freshness(runtime, Utc::now(), 0)
}

fn runtime_only_parser_health_with_freshness(
    runtime: &ParserRuntimeSnapshot,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> ParserHealth {
    ParserHealth {
        bookmaker: runtime.bookmaker.clone(),
        status: if matches!(runtime.circuit_state, RuntimeCircuitState::Open) {
            HealthStatus::CircuitOpen
        } else if runtime.total_runs == 0 {
            HealthStatus::Degraded
        } else if runtime.is_stale(now, stale_after_secs) {
            HealthStatus::Unhealthy
        } else if runtime.last_success.is_none()
            || matches!(runtime.last_result_status, ParserResultStatus::Failed)
        {
            HealthStatus::Unhealthy
        } else if runtime.consecutive_failures == 0
            && matches!(runtime.last_result_status, ParserResultStatus::Healthy)
        {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded
        },
        last_success: runtime.last_success,
        last_error: runtime
            .last_error
            .clone()
            .or_else(|| (runtime.total_runs == 0).then(|| STATIC_PARSER_HEALTH_NOTE.to_string())),
        consecutive_failures: runtime.consecutive_failures,
        avg_response_time_ms: runtime.avg_response_time_ms,
        events_parsed: runtime.events_parsed,
        uptime_percent: runtime.uptime_percent,
        readiness: None,
        diagnostics: runtime_health_diagnostics(runtime, now, stale_after_secs)
            .into_iter()
            .collect(),
    }
}

fn runtime_health_diagnostics(
    runtime: &ParserRuntimeSnapshot,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> Vec<ParserDiagnosticCheck> {
    let state = match runtime.circuit_state {
        RuntimeCircuitState::Closed => "closed",
        RuntimeCircuitState::HalfOpen => "half_open",
        RuntimeCircuitState::Open => "open",
    };
    let last_attempt = runtime
        .last_attempt
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| "never".to_string());

    let mut diagnostics = vec![
        ParserDiagnosticCheck {
            code: "runtime_state".into(),
            severity: match runtime.circuit_state {
                RuntimeCircuitState::Closed => DiagnosticSeverity::Pass,
                RuntimeCircuitState::HalfOpen => DiagnosticSeverity::Warn,
                RuntimeCircuitState::Open => DiagnosticSeverity::Fail,
            },
            message: format!(
                "runtime circuit={state}, total_runs={}, successful_runs={}, last_attempt={last_attempt}",
                runtime.total_runs, runtime.successful_runs,
            ),
        },
        ParserDiagnosticCheck {
            code: "runtime_throughput".into(),
            severity: if runtime.total_runs == 0 {
                DiagnosticSeverity::Info
            } else if matches!(runtime.last_result_status, ParserResultStatus::Healthy) {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Warn
            },
            message: format!(
                "runtime avg_response_time_ms={:.1}, events_parsed={}, odds_parsed={}, uptime_percent={:.1}",
                runtime.avg_response_time_ms,
                runtime.events_parsed,
                runtime.odds_parsed,
                runtime.uptime_percent,
            ),
        },
    ];
    diagnostics.push(ParserDiagnosticCheck {
        code: "runtime_staleness".into(),
        severity: if runtime.total_runs == 0 {
            DiagnosticSeverity::Info
        } else if runtime.is_stale(now, stale_after_secs) {
            DiagnosticSeverity::Fail
        } else {
            DiagnosticSeverity::Pass
        },
        message: match runtime.staleness_age_secs(now) {
            Some(age_secs) if stale_after_secs > 0 => {
                format!("runtime age_secs={age_secs}, stale_after_secs={stale_after_secs}")
            }
            Some(age_secs) => format!("runtime age_secs={age_secs}, stale_after_secs=disabled"),
            None => "runtime freshness unavailable until first fetch attempt".into(),
        },
    });
    diagnostics.push(ParserDiagnosticCheck {
        code: "runtime_validation".into(),
        severity: match runtime.last_result_status {
            ParserResultStatus::Healthy => DiagnosticSeverity::Pass,
            ParserResultStatus::Degraded => DiagnosticSeverity::Warn,
            ParserResultStatus::Failed => DiagnosticSeverity::Fail,
        },
        message: runtime.last_result_message.clone().unwrap_or_else(|| {
            format!(
                "last_result_status={:?}, validation_checks={}",
                runtime.last_result_status,
                runtime.validation_checks.len()
            )
        }),
    });
    diagnostics.extend(runtime.validation_checks.clone());
    diagnostics
}

pub fn build_live_parsers_health(
    fallback_health: Vec<ParserHealth>,
    runtime_snapshots: Vec<ParserRuntimeSnapshot>,
) -> Vec<ParserHealth> {
    build_live_parsers_health_with_freshness(fallback_health, runtime_snapshots, Utc::now(), 0)
}

pub fn build_live_parsers_health_with_freshness(
    fallback_health: Vec<ParserHealth>,
    runtime_snapshots: Vec<ParserRuntimeSnapshot>,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> Vec<ParserHealth> {
    let mut runtime = runtime_snapshots
        .into_iter()
        .map(|entry| (entry.bookmaker.clone(), entry))
        .collect::<std::collections::HashMap<_, _>>();

    let mut items = fallback_health
        .into_iter()
        .map(|fallback| {
            let runtime = runtime.remove(&fallback.bookmaker);
            merge_parser_health_with_freshness(&fallback, runtime.as_ref(), now, stale_after_secs)
        })
        .collect::<Vec<_>>();
    items.extend(
        runtime.into_values().map(|runtime| {
            runtime_only_parser_health_with_freshness(&runtime, now, stale_after_secs)
        }),
    );
    items.sort_by(|left, right| left.bookmaker.cmp(&right.bookmaker));
    items
}

pub fn build_live_parsers_coverage(
    fallback_coverage: Vec<ParserCoverage>,
    live_health: Vec<ParserHealth>,
) -> Vec<ParserCoverage> {
    let live_health = live_health
        .into_iter()
        .map(|item| (item.bookmaker.clone(), item))
        .collect::<std::collections::HashMap<_, _>>();

    let mut items = fallback_coverage
        .into_iter()
        .map(|mut coverage| {
            coverage.runtime_health = live_health.get(&coverage.slug).cloned();
            coverage
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.slug.cmp(&right.slug));
    items
}

#[cfg(test)]
mod tests {
    use super::{
        build_live_parsers_coverage, build_live_parsers_health,
        build_live_parsers_health_with_freshness, merge_parser_health,
        merge_parser_health_with_freshness, STATIC_PARSER_HEALTH_NOTE,
    };
    use chrono::{Duration, Utc};
    use shared::{
        BookmakerStatus, DiagnosticSeverity, HealthStatus, ParserCoverage, ParserDiagnosticCheck,
        ParserHealth, ParserReadiness, ParserReadinessStage, ParserResultStatus,
        ParserRuntimeSnapshot, RuntimeCircuitState,
    };

    fn make_snapshot_health(bookmaker: &str) -> ParserHealth {
        ParserHealth {
            bookmaker: bookmaker.into(),
            status: HealthStatus::Degraded,
            last_success: None,
            last_error: Some(STATIC_PARSER_HEALTH_NOTE.into()),
            consecutive_failures: 0,
            avg_response_time_ms: 0.0,
            events_parsed: 0,
            uptime_percent: 0.0,
            readiness: None,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn merge_parser_health_prefers_runtime_success_metrics() {
        let fallback = make_snapshot_health("pari");
        let runtime = ParserRuntimeSnapshot {
            bookmaker: "pari".into(),
            last_attempt: Some(Utc::now()),
            last_success: Some(Utc::now()),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 42.0,
            events_parsed: 123,
            odds_parsed: 456,
            uptime_percent: 100.0,
            total_runs: 1,
            successful_runs: 1,
            circuit_state: RuntimeCircuitState::Closed,
        };

        let merged = merge_parser_health(&fallback, Some(&runtime));

        assert!(matches!(merged.status, HealthStatus::Healthy));
        assert_eq!(merged.avg_response_time_ms, 42.0);
        assert_eq!(merged.events_parsed, 123);
        assert_eq!(merged.uptime_percent, 100.0);
        assert!(merged.last_success.is_some());
        assert_eq!(merged.last_error, None);
        assert!(merged.diagnostics.iter().any(|check| {
            check.code == "runtime_state"
                && matches!(check.severity, DiagnosticSeverity::Pass)
                && check.message.contains("total_runs=1")
                && check.message.contains("successful_runs=1")
        }));
        assert!(merged.diagnostics.iter().any(|check| {
            check.code == "runtime_throughput"
                && matches!(check.severity, DiagnosticSeverity::Pass)
                && check.message.contains("events_parsed=123")
        }));
        assert!(merged
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_validation"));
    }

    #[test]
    fn merge_parser_health_reports_open_circuit() {
        let fallback = make_snapshot_health("pari");
        let runtime = ParserRuntimeSnapshot {
            bookmaker: "pari".into(),
            last_attempt: Some(Utc::now()),
            last_success: None,
            last_error: Some("boom".into()),
            last_result_status: ParserResultStatus::Failed,
            last_result_message: Some("boom".into()),
            validation_checks: Vec::new(),
            consecutive_failures: 5,
            avg_response_time_ms: 12.0,
            events_parsed: 0,
            odds_parsed: 0,
            uptime_percent: 0.0,
            total_runs: 5,
            successful_runs: 0,
            circuit_state: RuntimeCircuitState::Open,
        };

        let merged = merge_parser_health(&fallback, Some(&runtime));

        assert!(matches!(merged.status, HealthStatus::CircuitOpen));
        assert_eq!(merged.last_error.as_deref(), Some("boom"));
        assert_eq!(merged.consecutive_failures, 5);
        assert!(merged.diagnostics.iter().any(|check| {
            check.code == "runtime_state"
                && matches!(check.severity, DiagnosticSeverity::Fail)
                && check.message.contains("circuit=open")
        }));
    }

    #[test]
    fn merge_parser_health_marks_stale_runtime_unhealthy() {
        let now = Utc::now();
        let fallback = make_snapshot_health("pari");
        let runtime = ParserRuntimeSnapshot {
            bookmaker: "pari".into(),
            last_attempt: Some(now - Duration::seconds(121)),
            last_success: Some(now - Duration::seconds(121)),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 42.0,
            events_parsed: 123,
            odds_parsed: 456,
            uptime_percent: 100.0,
            total_runs: 4,
            successful_runs: 4,
            circuit_state: RuntimeCircuitState::Closed,
        };

        let merged = merge_parser_health_with_freshness(&fallback, Some(&runtime), now, 120);

        assert!(matches!(merged.status, HealthStatus::Unhealthy));
        assert!(merged.diagnostics.iter().any(|check| {
            check.code == "runtime_staleness"
                && matches!(check.severity, DiagnosticSeverity::Fail)
                && check.message.contains("stale_after_secs=120")
        }));
    }

    #[test]
    fn build_live_parsers_health_merges_runtime_over_fallbacks() {
        let fallback_health = vec![ParserHealth {
            bookmaker: "winline".into(),
            status: HealthStatus::Degraded,
            last_success: None,
            last_error: Some(STATIC_PARSER_HEALTH_NOTE.into()),
            consecutive_failures: 0,
            avg_response_time_ms: 0.0,
            events_parsed: 0,
            uptime_percent: 0.0,
            readiness: Some(ParserReadiness {
                stage: ParserReadinessStage::Production,
                production_enabled: true,
                self_check_available: true,
                checks: vec![ParserDiagnosticCheck {
                    code: "runtime_ready".into(),
                    severity: DiagnosticSeverity::Pass,
                    message: "runtime ready".into(),
                }],
            }),
            diagnostics: vec![ParserDiagnosticCheck {
                code: "runtime_ready".into(),
                severity: DiagnosticSeverity::Pass,
                message: "runtime ready".into(),
            }],
        }];
        let runtime = vec![ParserRuntimeSnapshot {
            bookmaker: "winline".into(),
            last_attempt: Some(Utc::now()),
            last_success: Some(Utc::now()),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 18.0,
            events_parsed: 77,
            odds_parsed: 231,
            uptime_percent: 100.0,
            total_runs: 1,
            successful_runs: 1,
            circuit_state: RuntimeCircuitState::Closed,
        }];

        let live = build_live_parsers_health(fallback_health, runtime);
        let winline = live
            .into_iter()
            .find(|item| item.bookmaker == "winline")
            .expect("winline health");

        assert!(matches!(winline.status, HealthStatus::Healthy));
        assert_eq!(winline.events_parsed, 77);
        assert_eq!(winline.avg_response_time_ms, 18.0);
        assert_eq!(winline.last_error, None);
        assert!(winline.readiness.is_some());
        assert!(winline
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_validation"));
        assert!(winline
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_state"));
        assert!(winline
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_throughput"));
        assert!(winline
            .diagnostics
            .iter()
            .all(|check| check.code != "runtime_ready"));
    }

    #[test]
    fn merge_parser_health_keeps_snapshot_context_until_runtime_runs() {
        let fallback = ParserHealth {
            bookmaker: "pari".into(),
            status: HealthStatus::Degraded,
            last_success: None,
            last_error: Some(STATIC_PARSER_HEALTH_NOTE.into()),
            consecutive_failures: 0,
            avg_response_time_ms: 0.0,
            events_parsed: 0,
            uptime_percent: 0.0,
            readiness: None,
            diagnostics: vec![ParserDiagnosticCheck {
                code: "boot_snapshot".into(),
                severity: DiagnosticSeverity::Info,
                message: "factory snapshot only".into(),
            }],
        };
        let runtime = ParserRuntimeSnapshot {
            bookmaker: "pari".into(),
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
            uptime_percent: 0.0,
            total_runs: 0,
            successful_runs: 0,
            circuit_state: RuntimeCircuitState::Closed,
        };

        let merged = merge_parser_health(&fallback, Some(&runtime));

        assert_eq!(
            merged.last_error.as_deref(),
            Some(STATIC_PARSER_HEALTH_NOTE)
        );
        assert!(merged
            .diagnostics
            .iter()
            .any(|check| check.code == "boot_snapshot"));
        assert!(merged
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_state"));
    }

    #[test]
    fn build_live_parsers_health_keeps_runtime_only_parsers() {
        let runtime = vec![ParserRuntimeSnapshot {
            bookmaker: "melbet".into(),
            last_attempt: Some(Utc::now()),
            last_success: Some(Utc::now()),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 22.0,
            events_parsed: 41,
            odds_parsed: 120,
            uptime_percent: 100.0,
            total_runs: 1,
            successful_runs: 1,
            circuit_state: RuntimeCircuitState::Closed,
        }];

        let live = build_live_parsers_health(Vec::new(), runtime);
        let melbet = live
            .into_iter()
            .find(|item| item.bookmaker == "melbet")
            .expect("melbet health");

        assert!(matches!(melbet.status, HealthStatus::Healthy));
        assert_eq!(melbet.events_parsed, 41);
        assert!(melbet.readiness.is_none());
        assert_eq!(melbet.last_error, None);
        assert!(melbet
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_state"));
        assert!(melbet
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_throughput"));
    }

    #[test]
    fn build_live_parsers_health_with_freshness_marks_runtime_only_stale_parser() {
        let now = Utc::now();
        let runtime = vec![ParserRuntimeSnapshot {
            bookmaker: "melbet".into(),
            last_attempt: Some(now - Duration::seconds(90)),
            last_success: Some(now - Duration::seconds(90)),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 22.0,
            events_parsed: 41,
            odds_parsed: 120,
            uptime_percent: 100.0,
            total_runs: 1,
            successful_runs: 1,
            circuit_state: RuntimeCircuitState::Closed,
        }];

        let live = build_live_parsers_health_with_freshness(Vec::new(), runtime, now, 60);
        let melbet = live
            .into_iter()
            .find(|item| item.bookmaker == "melbet")
            .expect("melbet health");

        assert!(matches!(melbet.status, HealthStatus::Unhealthy));
        assert!(melbet
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_staleness"));
    }

    #[test]
    fn build_live_parsers_coverage_attaches_runtime_health() {
        let fallback_coverage = vec![ParserCoverage {
            slug: "ligastavok".into(),
            name: "Liga Stavok".into(),
            enabled: false,
            scan_supported: false,
            execution_supported: false,
            status: BookmakerStatus::Disabled,
            parser_type: "api".into(),
            source: "crates/parsers/src/ligastavok.rs".into(),
            notes: Some("disabled for diagnostics".into()),
            readiness: Some(ParserReadiness {
                stage: ParserReadinessStage::DiagnosticOnly,
                production_enabled: false,
                self_check_available: true,
                checks: vec![ParserDiagnosticCheck {
                    code: "qrator_unattended_bootstrap_unverified".into(),
                    severity: DiagnosticSeverity::Warn,
                    message: "bootstrap is unverified".into(),
                }],
            }),
            runtime_health: None,
        }];
        let live_health = vec![ParserHealth {
            bookmaker: "ligastavok".into(),
            status: HealthStatus::CircuitOpen,
            last_success: None,
            last_error: Some("runtime failure".into()),
            consecutive_failures: 5,
            avg_response_time_ms: 31.0,
            events_parsed: 0,
            uptime_percent: 0.0,
            readiness: None,
            diagnostics: Vec::new(),
        }];

        let live = build_live_parsers_coverage(fallback_coverage, live_health);
        let ligastavok = live
            .into_iter()
            .find(|item| item.slug == "ligastavok")
            .expect("ligastavok coverage");

        assert!(ligastavok.readiness.is_some());
        assert!(matches!(
            ligastavok
                .runtime_health
                .as_ref()
                .expect("runtime health")
                .status,
            HealthStatus::CircuitOpen
        ));
        let runtime_health = ligastavok.runtime_health.expect("runtime health");
        assert!(runtime_health.last_error.as_deref() == Some("runtime failure"));
    }
}
