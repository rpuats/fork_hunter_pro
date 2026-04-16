use parsers::base::ParserResult;
use shared::config::{ParserResultCaps, RuntimeProfile};
use shared::{DiagnosticSeverity, ParserDiagnosticCheck, ParserResultStatus};
use std::collections::HashSet;

const LOW_ODDS_COVERAGE_THRESHOLD: f64 = 0.25;
const BASELINE_DROP_THRESHOLD: f64 = 0.2;
const BASELINE_MIN_EVENTS: u64 = 20;

#[derive(Debug, Clone)]
pub struct ParserResultValidation {
    pub status: ParserResultStatus,
    pub summary: Option<String>,
    pub diagnostics: Vec<ParserDiagnosticCheck>,
}

#[derive(Debug, Clone)]
pub struct ValidatedParserResult {
    pub result: ParserResult,
    pub validation: ParserResultValidation,
}

impl ParserResultValidation {
    pub fn accepts_result(&self) -> bool {
        !matches!(self.status, ParserResultStatus::Failed)
    }
}

pub fn validate_parser_result(
    mut result: ParserResult,
    runtime_profile: RuntimeProfile,
    caps: ParserResultCaps,
    previous_events_parsed: Option<u64>,
) -> ValidatedParserResult {
    let original_event_count = result.events.len();
    let original_odds_count = result.odds.len();
    let mut cap_diagnostics = Vec::new();
    let mut cap_status = ParserResultStatus::Healthy;
    let mut cap_summary = None;

    if original_event_count > caps.max_events {
        result.events.truncate(caps.max_events);
        let event_ids = result
            .events
            .iter()
            .map(|event| event.id.as_str())
            .collect::<HashSet<_>>();
        result
            .odds
            .retain(|odd| event_ids.contains(odd.event_id.as_str()));
        apply_cap_diagnostic(
            &mut cap_status,
            &mut cap_summary,
            &mut cap_diagnostics,
            runtime_profile,
            "result_cap_events",
            format!(
                "events exceeded cap: original={}, kept={}, profile={:?}",
                original_event_count,
                result.events.len(),
                runtime_profile,
            ),
        );
    }

    if result.odds.len() > caps.max_odds {
        let original_kept_odds = result.odds.len();
        result.odds.truncate(caps.max_odds);
        apply_cap_diagnostic(
            &mut cap_status,
            &mut cap_summary,
            &mut cap_diagnostics,
            runtime_profile,
            "result_cap_odds",
            format!(
                "odds exceeded cap: original={}, kept={}, profile={:?}",
                original_kept_odds,
                result.odds.len(),
                runtime_profile,
            ),
        );
    }

    let event_count = result.events.len();
    let odds_count = result.odds.len();
    let event_ids = result
        .events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<HashSet<_>>();
    let odds_with_known_events = result
        .odds
        .iter()
        .filter(|odd| event_ids.contains(odd.event_id.as_str()))
        .count();
    let covered_events = result
        .odds
        .iter()
        .filter_map(|odd| {
            event_ids
                .contains(odd.event_id.as_str())
                .then_some(odd.event_id.as_str())
        })
        .collect::<HashSet<_>>()
        .len();
    let odds_coverage = if event_count == 0 {
        0.0
    } else {
        covered_events as f64 / event_count as f64
    };
    let orphan_odds = odds_count.saturating_sub(odds_with_known_events);

    let mut status = ParserResultStatus::Healthy;
    let mut summary = cap_summary;
    let mut diagnostics = vec![ParserDiagnosticCheck {
        code: "result_volume".into(),
        severity: DiagnosticSeverity::Info,
        message: format!(
            "events={}, odds={}, covered_events={}, odds_coverage={:.2}, caps=events:{}/odds:{}, original_events={}, original_odds={}",
            event_count,
            odds_count,
            covered_events,
            odds_coverage,
            caps.max_events,
            caps.max_odds,
            original_event_count,
            original_odds_count,
        ),
    }];
    diagnostics.extend(cap_diagnostics);
    status = worsen_status(status, cap_status);

    if event_count == 0 && odds_count == 0 {
        let degraded_in_dev = matches!(runtime_profile, RuntimeProfile::Dev);
        status = if degraded_in_dev {
            ParserResultStatus::Degraded
        } else {
            ParserResultStatus::Failed
        };
        summary = Some(if degraded_in_dev {
            "parser returned an empty payload in dev mode".into()
        } else {
            "parser returned an empty payload".into()
        });
        diagnostics.push(ParserDiagnosticCheck {
            code: "empty_payload".into(),
            severity: if degraded_in_dev {
                DiagnosticSeverity::Warn
            } else {
                DiagnosticSeverity::Fail
            },
            message: "no events and no odds were returned".into(),
        });
    }

    if event_count > 0 && odds_count == 0 {
        status = worsen_status(status, ParserResultStatus::Degraded);
        summary.get_or_insert_with(|| "parser returned events without odds".into());
        diagnostics.push(ParserDiagnosticCheck {
            code: "events_without_odds".into(),
            severity: DiagnosticSeverity::Warn,
            message: format!("{event_count} events were returned without any odds"),
        });
    }

    if orphan_odds > 0 {
        status = worsen_status(status, ParserResultStatus::Failed);
        summary = Some("parser returned odds that do not match fetched events".into());
        diagnostics.push(ParserDiagnosticCheck {
            code: "orphan_odds".into(),
            severity: DiagnosticSeverity::Fail,
            message: format!("{orphan_odds} odds reference missing events"),
        });
    }

    if event_count >= 4 && odds_count > 0 && odds_coverage < LOW_ODDS_COVERAGE_THRESHOLD {
        status = worsen_status(status, ParserResultStatus::Degraded);
        summary.get_or_insert_with(|| "parser returned suspiciously low odds coverage".into());
        diagnostics.push(ParserDiagnosticCheck {
            code: "low_odds_coverage".into(),
            severity: DiagnosticSeverity::Warn,
            message: format!(
                "only {:.0}% of events contain at least one odd",
                odds_coverage * 100.0
            ),
        });
    }

    if let Some(previous_events) =
        previous_events_parsed.filter(|count| *count >= BASELINE_MIN_EVENTS)
    {
        let current_events = event_count as u64;
        if current_events > 0
            && (current_events as f64) < (previous_events as f64 * BASELINE_DROP_THRESHOLD)
        {
            status = worsen_status(status, ParserResultStatus::Degraded);
            summary.get_or_insert_with(|| {
                "parser returned far fewer events than its recent baseline".into()
            });
            diagnostics.push(ParserDiagnosticCheck {
                code: "sharp_event_drop".into(),
                severity: DiagnosticSeverity::Warn,
                message: format!(
                    "events dropped from {previous_events} to {current_events} compared with the last accepted run"
                ),
            });
        }
    }

    if matches!(status, ParserResultStatus::Healthy) {
        diagnostics.push(ParserDiagnosticCheck {
            code: "post_fetch_validation".into(),
            severity: DiagnosticSeverity::Pass,
            message: "post-fetch validation passed".into(),
        });
    }

    ValidatedParserResult {
        result,
        validation: ParserResultValidation {
            status,
            summary,
            diagnostics,
        },
    }
}

fn apply_cap_diagnostic(
    status: &mut ParserResultStatus,
    summary: &mut Option<String>,
    diagnostics: &mut Vec<ParserDiagnosticCheck>,
    runtime_profile: RuntimeProfile,
    code: &str,
    message: String,
) {
    let degraded_in_dev = matches!(runtime_profile, RuntimeProfile::Dev);
    *status = worsen_status(
        status.clone(),
        if degraded_in_dev {
            ParserResultStatus::Degraded
        } else {
            ParserResultStatus::Failed
        },
    );
    summary.get_or_insert_with(|| {
        if degraded_in_dev {
            "parser result exceeded per-parser safety caps and was truncated in dev mode".into()
        } else {
            "parser result exceeded per-parser safety caps".into()
        }
    });
    diagnostics.push(ParserDiagnosticCheck {
        code: code.into(),
        severity: if degraded_in_dev {
            DiagnosticSeverity::Warn
        } else {
            DiagnosticSeverity::Fail
        },
        message,
    });
}

fn worsen_status(current: ParserResultStatus, next: ParserResultStatus) -> ParserResultStatus {
    use ParserResultStatus::{Degraded, Failed, Healthy};

    match (current, next) {
        (Failed, _) | (_, Failed) => Failed,
        (Degraded, _) | (_, Degraded) => Degraded,
        _ => Healthy,
    }
}

#[cfg(test)]
mod tests {
    use super::validate_parser_result;
    use chrono::Utc;
    use parsers::base::ParserResult;
    use shared::config::{ParserResultCaps, RuntimeProfile};
    use shared::odds::OddsType;
    use shared::{ParserResultStatus, Sport};
    use std::collections::HashMap;

    fn event(id: &str) -> shared::Event {
        shared::Event {
            id: id.into(),
            sport: Sport::Football,
            league: "League".into(),
            home_team: format!("{id}-home"),
            away_team: format!("{id}-away"),
            start_time: None,
            is_live: false,
            bookmaker_slug: "pari".into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

    fn odd(event_id: &str, id: &str) -> shared::Odd {
        shared::Odd {
            id: id.into(),
            event_id: event_id.into(),
            bookmaker_slug: "pari".into(),
            market: "1x2".into(),
            selection: "1".into(),
            odds: 2.0,
            odds_type: OddsType::Home,
            line: None,
            timestamp: Utc::now(),
        }
    }

    fn caps(max_events: usize, max_odds: usize) -> ParserResultCaps {
        ParserResultCaps {
            max_events,
            max_odds,
        }
    }

    #[test]
    fn empty_payload_is_failure_in_production_and_degraded_in_dev() {
        let result = ParserResult::new("pari", Vec::new(), Vec::new(), 10);

        let prod = validate_parser_result(
            result.clone(),
            RuntimeProfile::Production,
            caps(10, 10),
            None,
        );
        let dev = validate_parser_result(result, RuntimeProfile::Dev, caps(10, 10), None);

        assert_eq!(prod.validation.status, ParserResultStatus::Failed);
        assert_eq!(dev.validation.status, ParserResultStatus::Degraded);
    }

    #[test]
    fn orphan_odds_fail_validation() {
        let result = ParserResult::new("pari", vec![event("evt-1")], vec![odd("evt-2", "o-1")], 10);

        let validation =
            validate_parser_result(result, RuntimeProfile::Production, caps(10, 10), None);

        assert_eq!(validation.validation.status, ParserResultStatus::Failed);
        assert!(validation
            .validation
            .diagnostics
            .iter()
            .any(|check| check.code == "orphan_odds"));
    }

    #[test]
    fn sharp_event_drop_marks_result_degraded() {
        let result = ParserResult::new(
            "pari",
            (0..3).map(|idx| event(&format!("evt-{idx}"))).collect(),
            vec![odd("evt-0", "o-0")],
            10,
        );

        let validation =
            validate_parser_result(result, RuntimeProfile::Production, caps(10, 10), Some(40));

        assert_eq!(validation.validation.status, ParserResultStatus::Degraded);
        assert!(validation
            .validation
            .diagnostics
            .iter()
            .any(|check| check.code == "sharp_event_drop"));
    }

    #[test]
    fn dev_mode_truncates_capped_results_and_keeps_them_degraded() {
        let result = ParserResult::new(
            "pari",
            (0..4).map(|idx| event(&format!("evt-{idx}"))).collect(),
            (0..8)
                .map(|idx| odd(&format!("evt-{}", idx / 2), &format!("o-{idx}")))
                .collect(),
            10,
        );

        let validated = validate_parser_result(result, RuntimeProfile::Dev, caps(2, 3), None);

        assert_eq!(validated.validation.status, ParserResultStatus::Degraded);
        assert_eq!(validated.result.events.len(), 2);
        assert_eq!(validated.result.odds.len(), 3);
        assert!(validated
            .validation
            .diagnostics
            .iter()
            .any(|check| check.code == "result_cap_events"));
        assert!(validated
            .validation
            .diagnostics
            .iter()
            .any(|check| check.code == "result_cap_odds"));
    }

    #[test]
    fn production_mode_rejects_capped_results() {
        let result = ParserResult::new(
            "pari",
            (0..3).map(|idx| event(&format!("evt-{idx}"))).collect(),
            vec![odd("evt-0", "o-0")],
            10,
        );

        let validated =
            validate_parser_result(result, RuntimeProfile::Production, caps(2, 10), None);

        assert_eq!(validated.validation.status, ParserResultStatus::Failed);
        assert!(!validated.validation.accepts_result());
        assert_eq!(
            validated.validation.summary.as_deref(),
            Some("parser result exceeded per-parser safety caps")
        );
    }
}
