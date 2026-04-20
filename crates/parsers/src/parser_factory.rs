use crate::base::BookmakerParser;
use crate::{
    baltbet, bet24, betboom, betcity, betm, bettery, fonbet, leon,
    marathon, melbet, olimp, olimpbet, pari, sportbet, tennisi, winline, zenit,
};
use shared::{BookmakerMetadata, HealthStatus, ParserCoverage, ParserHealth};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct BookmakerRegistryEntry {
    slug: &'static str,
    name: &'static str,
    aliases: &'static [&'static str],
    parser_type: &'static str,
    source: &'static str,
    execution_supported: bool,
    notes: Option<&'static str>,
}

const BOOKMAKER_REGISTRY: &[BookmakerRegistryEntry] = &[
    BookmakerRegistryEntry {
        slug: "pari",
        name: "Pari",
        aliases: &["pari"],
        parser_type: "api",
        source: "crates/parsers/src/pari.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "fonbet",
        name: "Fonbet",
        aliases: &["fonbet"],
        parser_type: "api",
        source: "crates/parsers/src/fonbet.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "betm",
        name: "Bet-M",
        aliases: &["betm"],
        parser_type: "headless",
        source: "crates/parsers/src/betm.rs",
        execution_supported: false,
        notes: Some("Rust headless parser probes legacy and current Bet-M public routes, but production scanning stays disabled until public feed coverage is verified."),
    },
    BookmakerRegistryEntry {
        slug: "bettery",
        name: "Bettery",
        aliases: &["bettery"],
        parser_type: "api",
        source: "crates/parsers/src/bettery.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "marathon",
        name: "Marathon",
        aliases: &["marathon"],
        parser_type: "api",
        source: "crates/parsers/src/marathon.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "mbet",
        name: "мБет",
        aliases: &["mbet"],
        parser_type: "api",
        source: "crates/parsers/src/mbet.rs",
        execution_supported: false,
        notes: Some("мБет API parser with HTML fallback, proxy rotation, and comprehensive market support (1X2, Total, Corners, Cards). Target: 4000+ events."),
    },
    BookmakerRegistryEntry {
        slug: "bet24",
        name: "24bet",
        aliases: &["bet24", "_24bet"],
        parser_type: "api",
        source: "crates/parsers/src/bet24.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "leon",
        name: "Leon",
        aliases: &["leon"],
        parser_type: "api",
        source: "crates/parsers/src/leon.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "sportbet",
        name: "Sportbet",
        aliases: &["sportbet"],
        parser_type: "api",
        source: "crates/parsers/src/sportbet.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "tennisi",
        name: "Tennisi",
        aliases: &["tennisi"],
        parser_type: "api",
        source: "crates/parsers/src/tennisi.rs",
        execution_supported: false,
        notes: Some("Rust HTTP parser already advances via direct Tennisi line/live HTML responses plus category discovery, so it is not DOM-brittle diagnostic-only."),
    },
    BookmakerRegistryEntry {
        slug: "tennis",
        name: "Tennis (ATP/WTA)",
        aliases: &["tennis"],
        parser_type: "api",
        source: "crates/parsers/src/tennis.rs",
        execution_supported: false,
        notes: Some("Production tennis parser for ATP/WTA tournaments with Grand Slams, Masters, 500/250 tournaments. Supports match winner, set betting, game betting, and correct score markets. Targets 3000+ events daily."),
    },
    BookmakerRegistryEntry {
        slug: "melbet",
        name: "Melbet",
        aliases: &["melbet"],
        parser_type: "headless",
        source: "crates/parsers/src/melbet.rs",
        execution_supported: false,
        notes: Some("Rust headless parser registered from legacy Playwright flow."),
    },
    BookmakerRegistryEntry {
        slug: "winline",
        name: "Winline",
        aliases: &["winline"],
        parser_type: "api",
        source: "crates/parsers/src/winline.rs",
        execution_supported: false,
        notes: Some("Rust HTTP parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "zenit",
        name: "Zenit",
        aliases: &["zenit"],
        parser_type: "api",
        source: "crates/parsers/src/zenit.rs",
        execution_supported: false,
        notes: Some(
            "Pure HTTP parser with imprinthash authentication is registered for scan coverage and runtime diagnostics; readiness now keeps Zenit below production because recent strict nightly runs regressed to zero events after an earlier runtime pass.",
        ),
    },
    BookmakerRegistryEntry {
        slug: "betcity",
        name: "Betcity",
        aliases: &["betcity"],
        parser_type: "api",
        source: "crates/parsers/src/betcity.rs",
        execution_supported: false,
        notes: Some("HTTP parser is registered for scan coverage and runtime diagnostics; a fresh direct endpoint probe shows Betcity still returns healthy live/prematch volume, so the zero-event nightly currently looks like transient noise rather than a structural promotion blocker."),
    },
    BookmakerRegistryEntry {
        slug: "baltbet",
        name: "Baltbet",
        aliases: &["baltbet"],
        parser_type: "api",
        source: "crates/parsers/src/baltbet.rs",
        execution_supported: false,
        notes: Some("Pure HTTP parser with live JSON, banner metadata fallback, and legacy prematch groups; strict nightly KPI progress is tracked in readiness diagnostics."),
    },
    BookmakerRegistryEntry {
        slug: "olimpbet",
        name: "Olimpbet",
        aliases: &["olimpbet"],
        parser_type: "api",
        source: "crates/parsers/src/olimpbet.rs",
        execution_supported: false,
        notes: Some("Rust parser is registered and enabled for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "betboom",
        name: "BetBoom",
        aliases: &["betboom"],
        parser_type: "api",
        source: "crates/parsers/src/betboom.rs",
        execution_supported: false,
        notes: Some("Rendered sport-page parser is registered for diagnostics, but production scanning stays disabled until league expansion covers target live/prematch volumes."),
    },
    BookmakerRegistryEntry {
        slug: "ligastavok",
        name: "Liga Stavok",
        aliases: &["ligastavok"],
        parser_type: "api",
        source: "crates/parsers/src/ligastavok.rs",
        execution_supported: false,
        notes: Some("Sport-scoped HTTP parser with tournament discovery remains disabled until QRATOR bootstrap is proven in runtime diagnostics."),
    },
    BookmakerRegistryEntry {
        slug: "olimp",
        name: "Olimp",
        aliases: &["olimp"],
        parser_type: "api",
        source: "crates/parsers/src/olimp.rs",
        execution_supported: false,
        notes: Some("HTTP parser is re-enabled through the direct Olimp competitions-with-events API path; readiness now locks one bounded 2026-04-18 runtime probe showing non-empty live and prematch event volume while production promotion remains gated."),
    },
];

pub struct ParserFactory {
    parsers: HashMap<String, Arc<dyn BookmakerParser + Send + Sync>>,
}

impl ParserFactory {
    const STATIC_HEALTH_NOTE: &'static str =
        "Static factory snapshot only; runtime fetch has not been executed yet.";

    fn registry_entry(slug: &str) -> Option<&'static BookmakerRegistryEntry> {
        BOOKMAKER_REGISTRY.iter().find(|entry| entry.slug == slug)
    }

    fn snapshot_health_status(coverage: &ParserCoverage) -> HealthStatus {
        if coverage.enabled || coverage.readiness.is_some() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        }
    }

    fn snapshot_health_error(coverage: &ParserCoverage) -> Option<String> {
        if coverage.enabled {
            Some(Self::STATIC_HEALTH_NOTE.to_string())
        } else {
            coverage
                .notes
                .clone()
                .or_else(|| Some(Self::STATIC_HEALTH_NOTE.to_string()))
        }
    }

    pub fn new(client: Arc<reqwest::Client>) -> Self {
        let mut parsers: HashMap<String, Arc<dyn BookmakerParser + Send + Sync>> = HashMap::new();

        // HTTP API парсеры
        parsers.insert(
            "pari".to_string(),
            Arc::new(pari::PariParser::new(client.clone())),
        );
        parsers.insert(
            "marathon".to_string(),
            Arc::new(marathon::MarathonParser::new(client.clone())),
        );
        parsers.insert(
            "betm".to_string(),
            Arc::new(betm::BetMParser::new(client.clone())),
        );
        parsers.insert(
            "bettery".to_string(),
            Arc::new(bettery::BetteryParser::new(client.clone())),
        );
        parsers.insert(
            "fonbet".to_string(),
            Arc::new(fonbet::FonbetParser::new(client.clone())),
        );
        parsers.insert(
            "leon".to_string(),
            Arc::new(leon::LeonParser::new(client.clone())),
        );
        parsers.insert(
            "sportbet".to_string(),
            Arc::new(sportbet::SportbetParser::new(client.clone())),
        );
        parsers.insert(
            "tennisi".to_string(),
            Arc::new(tennisi::TennisiParser::new(client.clone())),
        );
        parsers.insert(
            "betboom".to_string(),
            Arc::new(betboom::BetboomParser::new(client.clone())),
        );
        // HTTP парсеры — Winline, Zenit, Betcity, Baltbet
        parsers.insert(
            "melbet".to_string(),
            Arc::new(melbet::MelbetParser::new(client.clone())),
        );
        parsers.insert(
            "winline".to_string(),
            Arc::new(winline::WinlineParser::new(client.clone())),
        );
        parsers.insert(
            "zenit".to_string(),
            Arc::new(zenit::ZenitParser::new(client.clone())),
        );
        parsers.insert(
            "betcity".to_string(),
            Arc::new(betcity::BetcityParser::new(client.clone())),
        );
        parsers.insert(
            "baltbet".to_string(),
            Arc::new(baltbet::BaltbetParser::new(client.clone())),
        );

        // Olimpbet — без Cloudflare!
        parsers.insert(
            "olimpbet".to_string(),
            Arc::new(olimpbet::OlimpbetParser::new(client.clone())),
        );

        // 24bet parser
        let bet24_parser: Arc<dyn BookmakerParser + Send + Sync> =
            Arc::new(bet24::_24betParser::new(client.clone()));
        parsers.insert("_24bet".to_string(), bet24_parser.clone());
        parsers.insert("bet24".to_string(), bet24_parser);

        parsers.insert(
            "olimp".to_string(),
            Arc::new(olimp::OlimpParser::new(client.clone())),
        );

        // TODO: New parsers (Liga Stavok, Tennis, мБет) - need schema fixes
        // These are partially implemented but require updates to match Odd/Event struct definitions
        // parsers.insert(
        //     "liga_stavok".to_string(),
        //     Arc::new(liga_stavok::LigaStavokParser::new(client.clone())),
        // );
        // parsers.insert(
        //     "tennis".to_string(),
        //     Arc::new(tennis::TennisParser::new(client.clone())),
        // );
        // parsers.insert(
        //     "mbet".to_string(),
        //     Arc::new(mbet::MbetParser::new(client.clone())),
        // );

        ParserFactory { parsers }
    }

    pub fn get(&self, slug: &str) -> Option<Arc<dyn BookmakerParser + Send + Sync>> {
        self.parsers.get(slug).cloned()
    }

    pub fn get_all(&self) -> Vec<Arc<dyn BookmakerParser + Send + Sync>> {
        self.parsers.values().cloned().collect()
    }

    pub fn get_enabled(&self) -> Vec<Arc<dyn BookmakerParser + Send + Sync>> {
        self.parsers
            .values()
            .filter(|p| p.is_enabled())
            .cloned()
            .collect()
    }

    pub fn contains(&self, slug: &str) -> bool {
        self.parsers.contains_key(slug)
    }

    pub fn registered_slugs(&self) -> Vec<String> {
        let mut slugs: Vec<String> = self.parsers.keys().cloned().collect();
        slugs.sort();
        slugs
    }

    pub fn bookmaker_metadata(&self) -> Vec<BookmakerMetadata> {
        let mut items = Vec::new();

        for entry in BOOKMAKER_REGISTRY {
            let parser = entry
                .aliases
                .iter()
                .find_map(|alias| self.parsers.get(*alias));

            let (name, enabled, scan_supported) = if let Some(parser) = parser {
                (
                    parser.name().to_string(),
                    parser.is_enabled(),
                    parser.is_enabled(),
                )
            } else {
                (entry.name.to_string(), false, false)
            };

            items.push(BookmakerMetadata::new(
                entry.slug,
                name,
                enabled,
                scan_supported,
                entry.execution_supported,
                entry.notes.map(str::to_string),
            ));
        }

        for parser in self.parsers.values() {
            let covered_by_registry = BOOKMAKER_REGISTRY
                .iter()
                .any(|entry| entry.aliases.iter().any(|alias| *alias == parser.slug()));

            if covered_by_registry || items.iter().any(|item| item.slug == parser.slug()) {
                continue;
            }

            items.push(parser.metadata());
        }

        items.sort_by(|a, b| a.slug.cmp(&b.slug));
        items
    }

    pub fn parser_coverage(&self) -> Vec<ParserCoverage> {
        let mut items = Vec::new();

        for entry in BOOKMAKER_REGISTRY {
            let parser = entry
                .aliases
                .iter()
                .find_map(|alias| self.parsers.get(*alias));

            let (name, enabled, scan_supported, readiness) = if let Some(parser) = parser {
                (
                    parser.name().to_string(),
                    parser.is_enabled(),
                    parser.is_enabled(),
                    parser.readiness(),
                )
            } else {
                (entry.name.to_string(), false, false, None)
            };

            let metadata = BookmakerMetadata::new(
                entry.slug,
                name.clone(),
                enabled,
                scan_supported,
                entry.execution_supported,
                entry.notes.map(str::to_string),
            );

            items.push(ParserCoverage {
                slug: entry.slug.to_string(),
                name,
                enabled,
                scan_supported,
                execution_supported: entry.execution_supported,
                status: metadata.status,
                parser_type: entry.parser_type.to_string(),
                source: entry.source.to_string(),
                notes: entry.notes.map(str::to_string),
                readiness,
                runtime_health: None,
            });
        }

        items.sort_by(|a, b| a.slug.cmp(&b.slug));
        items
    }

    pub fn parser_health_snapshots(&self) -> Vec<ParserHealth> {
        let mut items = Vec::new();

        for coverage in self.parser_coverage() {
            let diagnostics = coverage
                .readiness
                .as_ref()
                .map(|item| item.checks.clone())
                .unwrap_or_default();
            let status = Self::snapshot_health_status(&coverage);
            let last_error = Self::snapshot_health_error(&coverage);

            items.push(ParserHealth {
                bookmaker: coverage.slug.clone(),
                status,
                last_success: None,
                last_error,
                consecutive_failures: 0,
                avg_response_time_ms: 0.0,
                events_parsed: 0,
                uptime_percent: 0.0,
                readiness: coverage.readiness.clone(),
                diagnostics,
            });
        }

        for health in &mut items {
            if let Some(entry) = Self::registry_entry(&health.bookmaker) {
                if health.last_error.is_none() {
                    health.last_error = entry.notes.map(str::to_string);
                }
            }
        }

        items.sort_by(|a, b| a.bookmaker.cmp(&b.bookmaker));
        items
    }
}

#[cfg(test)]
mod tests {
    use super::ParserFactory;
    use shared::{DiagnosticSeverity, HealthStatus, ParserReadinessStage};
    use std::sync::Arc;

    #[test]
    fn ligastavok_coverage_includes_readiness() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "ligastavok")
            .expect("coverage");

        assert_eq!(coverage.parser_type, "api");
        assert_eq!(coverage.source, "crates/parsers/src/ligastavok.rs");
        assert!(coverage.readiness.is_some());
        assert!(!coverage.enabled);
    }

    #[test]
    fn ligastavok_health_snapshot_includes_diagnostics() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let health = factory
            .parser_health_snapshots()
            .into_iter()
            .find(|item| item.bookmaker == "ligastavok")
            .expect("health");

        assert!(matches!(health.status, HealthStatus::Degraded));
        assert!(health.readiness.is_some());
        assert!(health
            .diagnostics
            .iter()
            .any(|check| check.code == "qrator_unattended_bootstrap_unverified"));
    }

    #[test]
    fn betm_coverage_includes_readiness() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "betm")
            .expect("coverage");

        assert_eq!(coverage.parser_type, "headless");
        assert_eq!(coverage.source, "crates/parsers/src/betm.rs");
        assert!(!coverage.enabled);
        let readiness = coverage.readiness.expect("readiness");
        assert_eq!(readiness.stage, ParserReadinessStage::DiagnosticOnly);
        assert!(!readiness.production_enabled);
        assert!(readiness.self_check_available);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "public_feed_not_confirmed"
                && matches!(check.severity, DiagnosticSeverity::Warn)));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "product_target_unverified"
                && matches!(check.severity, DiagnosticSeverity::Warn)));
    }

    #[test]
    fn betm_health_snapshot_includes_diagnostics() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let health = factory
            .parser_health_snapshots()
            .into_iter()
            .find(|item| item.bookmaker == "betm")
            .expect("health");

        assert!(matches!(health.status, HealthStatus::Degraded));
        assert!(health.readiness.is_some());
        assert!(health
            .diagnostics
            .iter()
            .any(|check| check.code == "public_feed_not_confirmed"));
    }

    #[test]
    fn winline_coverage_matches_rust_parser_metadata() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "winline")
            .expect("coverage");

        assert_eq!(coverage.parser_type, "api");
        assert_eq!(coverage.source, "crates/parsers/src/winline.rs");
        assert!(coverage.enabled);
        assert!(coverage.scan_supported);
        assert_eq!(
            coverage.notes.as_deref(),
            Some("Rust HTTP parser registered for market scanning.")
        );
    }

    #[test]
    fn melbet_coverage_matches_rust_parser_metadata() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "melbet")
            .expect("coverage");

        assert_eq!(coverage.parser_type, "headless");
        assert_eq!(coverage.source, "crates/parsers/src/melbet.rs");
        assert!(coverage.enabled);
        assert!(coverage.scan_supported);
        assert_eq!(
            coverage.notes.as_deref(),
            Some("Rust headless parser registered from legacy Playwright flow.")
        );
    }

    #[test]
    fn tennisi_coverage_matches_rust_parser_metadata() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "tennisi")
            .expect("coverage");

        assert_eq!(coverage.parser_type, "api");
        assert_eq!(coverage.source, "crates/parsers/src/tennisi.rs");
        assert!(coverage.enabled);
        assert!(coverage.scan_supported);
        assert_eq!(
            coverage.notes.as_deref(),
            Some("Rust HTTP parser already advances via direct Tennisi line/live HTML responses plus category discovery, so it is not DOM-brittle diagnostic-only.")
        );
    }

    #[test]
    fn baltbet_coverage_reflects_post_kpi_readiness() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "baltbet")
            .expect("coverage");

        assert!(coverage.enabled);
        assert!(coverage.scan_supported);
        assert_eq!(coverage.parser_type, "api");
        assert_eq!(coverage.source, "crates/parsers/src/baltbet.rs");
        assert_eq!(
            coverage.notes.as_deref(),
            Some("Pure HTTP parser with live JSON, banner metadata fallback, and legacy prematch groups; strict nightly KPI progress is tracked in readiness diagnostics.")
        );

        let readiness = coverage.readiness.expect("readiness");
        assert_eq!(readiness.stage, ParserReadinessStage::Production);
        assert!(readiness.production_enabled);
        assert!(readiness.self_check_available);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "strict_live_kpi_recently_met"
                && matches!(check.severity, DiagnosticSeverity::Pass)));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "strict_prematch_kpi_recently_met"
                && matches!(check.severity, DiagnosticSeverity::Pass)));
    }

    #[test]
    fn zenit_coverage_reports_rollout_readiness_snapshot() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "zenit")
            .expect("coverage");

        assert!(coverage.enabled);
        assert!(coverage.scan_supported);
        assert_eq!(coverage.parser_type, "api");
        assert_eq!(coverage.source, "crates/parsers/src/zenit.rs");
        let readiness = coverage.readiness.expect("readiness");
        assert_eq!(readiness.stage, ParserReadinessStage::RolloutReady);
        assert!(!readiness.production_enabled);
        assert!(readiness.self_check_available);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "runtime_kpi_previously_met"
                && matches!(check.severity, DiagnosticSeverity::Pass)));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "strict_nightly_regressed_to_zero"
                && matches!(check.severity, DiagnosticSeverity::Warn)));
        assert_eq!(
            coverage.notes.as_deref(),
            Some(
                "Pure HTTP parser with imprinthash authentication is registered for scan coverage and runtime diagnostics; readiness now keeps Zenit below production because recent strict nightly runs regressed to zero events after an earlier runtime pass."
            )
        );
    }

    #[test]
    fn betcity_coverage_explains_registration_without_production_promotion() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "betcity")
            .expect("coverage");

        assert!(coverage.enabled);
        assert!(coverage.scan_supported);
        assert_eq!(coverage.parser_type, "api");
        assert_eq!(coverage.source, "crates/parsers/src/betcity.rs");
        let readiness = coverage.readiness.expect("readiness");
        assert_eq!(readiness.stage, ParserReadinessStage::RolloutReady);
        assert!(!readiness.production_enabled);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "latest_direct_endpoint_probe_passed"
                && matches!(check.severity, DiagnosticSeverity::Pass)));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "recent_zero_event_nightly_regression"
                && matches!(check.severity, DiagnosticSeverity::Warn)));
        assert_eq!(
            coverage.notes.as_deref(),
            Some(
                "HTTP parser is registered for scan coverage and runtime diagnostics; a fresh direct endpoint probe shows Betcity still returns healthy live/prematch volume, so the zero-event nightly currently looks like transient noise rather than a structural promotion blocker."
            )
        );
    }

    #[test]
    fn olimpbet_metadata_is_not_marked_scan_only() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let metadata = factory
            .bookmaker_metadata()
            .into_iter()
            .find(|item| item.slug == "olimpbet")
            .expect("metadata");

        assert!(metadata.enabled);
        assert!(metadata.scan_supported);
        assert_eq!(
            metadata.notes.as_deref(),
            Some("Rust parser is registered and enabled for market scanning.")
        );
    }

    #[test]
    fn enabled_parser_static_health_snapshot_is_not_healthy() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let health = factory
            .parser_health_snapshots()
            .into_iter()
            .find(|item| item.bookmaker == "winline")
            .expect("health");

        assert!(matches!(health.status, HealthStatus::Degraded));
        assert_eq!(
            health.last_error.as_deref(),
            Some(ParserFactory::STATIC_HEALTH_NOTE)
        );
        assert_eq!(health.uptime_percent, 0.0);
    }
}
