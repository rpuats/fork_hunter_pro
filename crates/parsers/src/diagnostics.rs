use crate::baltbet::BaltbetParser;
use crate::base::BookmakerParser;
use crate::betboom::BetboomParser;
use crate::betcity::BetcityParser;
use crate::ligastavok::LigaStavokParser;
use crate::winline::WinlineParser;
use crate::zenit::ZenitParser;
use shared::Event;
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS: &[&str] = &[
    "winline",
    "zenit",
    "betcity",
    "baltbet",
    "ligastavok",
    "betboom",
];

const LIVE_THRESHOLD: usize = 100;
const PREMATCH_THRESHOLD: usize = 2000;
const FETCH_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone)]
pub struct RuntimeCountReport {
    pub bookmaker_slug: String,
    pub total_events: usize,
    pub live_events: usize,
    pub prematch_events: usize,
    pub live_threshold_met: bool,
    pub prematch_threshold_met: bool,
    pub passed: bool,
    pub runtime_only: bool,
    pub duration_ms: u128,
    pub error: Option<String>,
}

impl RuntimeCountReport {
    fn from_events(bookmaker_slug: &str, events: &[Event], duration_ms: u128) -> Self {
        let live_events = events.iter().filter(|event| event.is_live).count();
        let prematch_events = events.len().saturating_sub(live_events);
        let live_threshold_met = live_events >= LIVE_THRESHOLD;
        let prematch_threshold_met = prematch_events >= PREMATCH_THRESHOLD;

        Self {
            bookmaker_slug: bookmaker_slug.to_string(),
            total_events: events.len(),
            live_events,
            prematch_events,
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
) -> Vec<RuntimeCountReport> {
    let requested: Vec<String> = if slugs.is_empty() {
        DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS
            .iter()
            .map(|slug| (*slug).to_string())
            .collect()
    } else {
        slugs.iter().map(|slug| slug.to_lowercase()).collect()
    };

    let mut reports = Vec::with_capacity(requested.len());
    for slug in requested {
        reports.push(run_single_runtime_diagnostic(client.clone(), &slug).await);
    }
    reports
}

async fn run_single_runtime_diagnostic(
    client: Arc<reqwest::Client>,
    slug: &str,
) -> RuntimeCountReport {
    let started = std::time::Instant::now();

    let result = match slug {
        "winline" => with_timeout(WinlineParser::new(client).fetch_runtime_data()).await,
        "baltbet" => {
            let parser = BaltbetParser::new(client);
            with_timeout(async {
                let events = parser.fetch_events().await?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((events, Vec::new()))
            }).await
        }
        "betcity" => with_timeout(BetcityParser::new(client).fetch_runtime_data()).await,
        _ => Err(format!("bookmaker {} not supported in diagnostics yet", slug)),
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
    use super::RuntimeCountReport;
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
        assert!(!report.passed);
    }
}
