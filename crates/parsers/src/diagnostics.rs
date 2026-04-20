use crate::baltbet::BaltbetParser;
use crate::base::BookmakerParser;
use crate::betboom::BetboomParser;
use crate::betcity::BetcityParser;
// use crate::ligastavok::LigaStavokParser; // TODO: Re-enable once schema is fixed
use crate::melbet::MelbetParser;
use crate::tennisi::TennisiParser;
use crate::winline::WinlineParser;
use crate::zenit::ZenitParser;
use chrono::{DateTime, Utc};
use serde::Serialize;
use shared::Event;
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS: &[&str] = &[
    "winline",
    "melbet",
    "zenit",
    "betcity",
    "baltbet",
    "ligastavok",
    "betboom",
];

pub const LIVE_THRESHOLD: usize = 150;
pub const PREMATCH_THRESHOLD: usize = 3000;
const FETCH_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeKpiCheck {
    pub actual: usize,
    pub target: usize,
    pub missing: usize,
    pub passed: bool,
}

impl RuntimeKpiCheck {
    fn new(actual: usize, target: usize) -> Self {
        Self {
            actual,
            target,
            missing: target.saturating_sub(actual),
            passed: actual >= target,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeCountReport {
    pub bookmaker_slug: String,
    pub total_events: usize,
    pub live_events: usize,
    pub prematch_events: usize,
    pub live_kpi: RuntimeKpiCheck,
    pub prematch_kpi: RuntimeKpiCheck,
    pub live_threshold_met: bool,
    pub prematch_threshold_met: bool,
    pub passed: bool,
    pub runtime_only: bool,
    pub duration_ms: u128,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDiagnosticsSummary {
    pub requested_slugs: Vec<String>,
    pub total_reports: usize,
    pub passed_reports: usize,
    pub failed_reports: usize,
    pub live_threshold: usize,
    pub prematch_threshold: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDiagnosticsNightlySummary {
    pub status: &'static str,
    pub exit_code: i32,
    pub failing_slugs: Vec<String>,
    pub live_gap_total: usize,
    pub prematch_gap_total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDiagnosticsAggregate {
    pub total_events: usize,
    pub live_events: usize,
    pub prematch_events: usize,
    pub live_threshold_passes: usize,
    pub prematch_threshold_passes: usize,
    pub passed_reports: usize,
    pub failed_reports: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDiagnosticsRun {
    pub generated_at: DateTime<Utc>,
    pub summary: RuntimeDiagnosticsSummary,
    pub aggregate: RuntimeDiagnosticsAggregate,
    pub nightly: RuntimeDiagnosticsNightlySummary,
    pub reports: Vec<RuntimeCountReport>,
}

impl RuntimeCountReport {
    fn from_events(bookmaker_slug: &str, events: &[Event], duration_ms: u128) -> Self {
        let live_events = events.iter().filter(|event| event.is_live).count();
        let prematch_events = events.len().saturating_sub(live_events);
        let live_kpi = RuntimeKpiCheck::new(live_events, LIVE_THRESHOLD);
        let prematch_kpi = RuntimeKpiCheck::new(prematch_events, PREMATCH_THRESHOLD);
        let live_threshold_met = live_kpi.passed;
        let prematch_threshold_met = prematch_kpi.passed;

        Self {
            bookmaker_slug: bookmaker_slug.to_string(),
            total_events: events.len(),
            live_events,
            prematch_events,
            live_kpi,
            prematch_kpi,
            live_threshold_met,
            prematch_threshold_met,
            passed: live_threshold_met && prematch_threshold_met,
            runtime_only: true,
            duration_ms,
            error: None,
        }
    }

    fn from_error(bookmaker_slug: &str, error: String, duration_ms: u128) -> Self {
        Self {
            bookmaker_slug: bookmaker_slug.to_string(),
            total_events: 0,
            live_events: 0,
            prematch_events: 0,
            live_kpi: RuntimeKpiCheck::new(0, LIVE_THRESHOLD),
            prematch_kpi: RuntimeKpiCheck::new(0, PREMATCH_THRESHOLD),
            live_threshold_met: false,
            prematch_threshold_met: false,
            passed: false,
            runtime_only: true,
            duration_ms,
            error: Some(error),
        }
    }
}

pub async fn run_runtime_diagnostics(
    client: Arc<reqwest::Client>,
    slugs: &[String],
) -> RuntimeDiagnosticsRun {
    let requested: Vec<String> = if slugs.is_empty() {
        DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS
            .iter()
            .map(|slug| (*slug).to_string())
            .collect()
    } else {
        slugs.iter().map(|slug| slug.to_lowercase()).collect()
    };

    let mut reports = Vec::with_capacity(requested.len());
    for slug in &requested {
        reports.push(run_single_runtime_diagnostic(client.clone(), &slug).await);
    }

    let passed_reports = reports.iter().filter(|report| report.passed).count();
    let total_reports = reports.len();
    let failed_reports = total_reports.saturating_sub(passed_reports);
    let failing_slugs = reports
        .iter()
        .filter(|report| !report.passed)
        .map(|report| report.bookmaker_slug.clone())
        .collect::<Vec<_>>();
    let aggregate = RuntimeDiagnosticsAggregate {
        total_events: reports.iter().map(|report| report.total_events).sum(),
        live_events: reports.iter().map(|report| report.live_events).sum(),
        prematch_events: reports.iter().map(|report| report.prematch_events).sum(),
        live_threshold_passes: reports
            .iter()
            .filter(|report| report.live_threshold_met)
            .count(),
        prematch_threshold_passes: reports
            .iter()
            .filter(|report| report.prematch_threshold_met)
            .count(),
        passed_reports,
        failed_reports,
    };

    RuntimeDiagnosticsRun {
        generated_at: Utc::now(),
        summary: RuntimeDiagnosticsSummary {
            requested_slugs: requested,
            total_reports,
            passed_reports,
            failed_reports,
            live_threshold: LIVE_THRESHOLD,
            prematch_threshold: PREMATCH_THRESHOLD,
        },
        aggregate,
        nightly: RuntimeDiagnosticsNightlySummary {
            status: if failed_reports == 0 { "pass" } else { "fail" },
            exit_code: if failed_reports == 0 { 0 } else { 2 },
            failing_slugs,
            live_gap_total: reports.iter().map(|report| report.live_kpi.missing).sum(),
            prematch_gap_total: reports
                .iter()
                .map(|report| report.prematch_kpi.missing)
                .sum(),
        },
        reports,
    }
}

async fn run_single_runtime_diagnostic(
    client: Arc<reqwest::Client>,
    slug: &str,
) -> RuntimeCountReport {
    let started = std::time::Instant::now();

    let result = match slug {
        "winline" => with_timeout(WinlineParser::new(client).fetch_runtime_data()).await,
        "melbet" => with_timeout(MelbetParser::new(client).fetch_runtime_data()).await,
        "zenit" => with_timeout(ZenitParser::new(client).fetch_runtime_data()).await,
        "betboom" => with_timeout(BetboomParser::new(client).fetch_runtime_data()).await,
        "baltbet" => {
            let parser = BaltbetParser::new(client);
            with_timeout(async {
                let events = parser.fetch_events().await?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((events, Vec::new()))
            })
            .await
        }
        "betcity" => with_timeout(BetcityParser::new(client).fetch_runtime_data()).await,
        // "ligastavok" => with_timeout(LigaStavokParser::new(client).fetch_runtime_data()).await,
        "tennisi" => with_timeout(TennisiParser::new(client).fetch_runtime_data()).await,
        _ => Err(format!(
            "bookmaker {} not supported in diagnostics yet",
            slug
        )),
    };

    let elapsed = started.elapsed().as_millis();
    match result {
        Ok((events, _odds)) => RuntimeCountReport::from_events(slug, &events, elapsed),
        Err(error) => RuntimeCountReport::from_error(slug, error, elapsed),
    }
}

async fn with_timeout<F>(future: F) -> Result<(Vec<Event>, Vec<shared::Odd>), String>
where
    F: std::future::Future<
        Output = Result<(Vec<Event>, Vec<shared::Odd>), Box<dyn std::error::Error + Send + Sync>>,
    >,
{
    match tokio::time::timeout(FETCH_TIMEOUT, future).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err(format!("timeout after {}s", FETCH_TIMEOUT.as_secs())),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeCountReport, RuntimeDiagnosticsAggregate, RuntimeDiagnosticsNightlySummary,
        RuntimeDiagnosticsRun, RuntimeDiagnosticsSummary,
    };
    use chrono::Utc;
    use shared::{Event, Sport};
    use std::collections::HashMap;

    #[test]
    fn splits_live_and_prematch_counts() {
        let events = vec![
            Event {
                id: "1".into(),
                sport: Sport::Football,
                league: "L1".into(),
                home_team: "A".into(),
                away_team: "B".into(),
                start_time: None,
                is_live: true,
                bookmaker_slug: "test".into(),
                raw_url: None,
                extra: HashMap::new(),
            },
            Event {
                id: "2".into(),
                sport: Sport::Football,
                league: "L1".into(),
                home_team: "C".into(),
                away_team: "D".into(),
                start_time: None,
                is_live: false,
                bookmaker_slug: "test".into(),
                raw_url: None,
                extra: HashMap::new(),
            },
        ];

        let report = RuntimeCountReport::from_events("test", &events, 42);
        assert_eq!(report.total_events, 2);
        assert_eq!(report.live_events, 1);
        assert_eq!(report.prematch_events, 1);
        assert_eq!(report.live_kpi.target, 150);
        assert_eq!(report.live_kpi.missing, 149);
        assert_eq!(report.prematch_kpi.target, 3000);
        assert_eq!(report.prematch_kpi.missing, 2999);
        assert!(!report.passed);
    }

    #[test]
    fn runtime_run_serializes_threshold_metadata() {
        let run = RuntimeDiagnosticsRun {
            generated_at: Utc::now(),
            summary: RuntimeDiagnosticsSummary {
                requested_slugs: vec!["test".into()],
                total_reports: 1,
                passed_reports: 0,
                failed_reports: 1,
                live_threshold: 150,
                prematch_threshold: 3000,
            },
            aggregate: RuntimeDiagnosticsAggregate {
                total_events: 0,
                live_events: 0,
                prematch_events: 0,
                live_threshold_passes: 0,
                prematch_threshold_passes: 0,
                passed_reports: 0,
                failed_reports: 1,
            },
            nightly: RuntimeDiagnosticsNightlySummary {
                status: "fail",
                exit_code: 2,
                failing_slugs: vec!["test".into()],
                live_gap_total: 150,
                prematch_gap_total: 3000,
            },
            reports: vec![RuntimeCountReport::from_error("test", "boom".into(), 42)],
        };

        let json = serde_json::to_value(&run).expect("serialize run");
        assert_eq!(json["summary"]["live_threshold"], 150);
        assert_eq!(json["summary"]["prematch_threshold"], 3000);
        assert_eq!(json["summary"]["requested_slugs"][0], "test");
        assert_eq!(json["aggregate"]["failed_reports"], 1);
        assert_eq!(json["nightly"]["status"], "fail");
        assert_eq!(json["reports"][0]["live_kpi"]["target"], 150);
        assert_eq!(json["reports"][0]["bookmaker_slug"], "test");
    }
}
