use reqwest::Client;
use std::sync::Arc;

use parsers::parser_factory::ParserFactory;
use shared::{DiagnosticSeverity, HealthStatus, ParserReadinessStage};

#[test]
fn factory_builds_parsers() {
    // Simple in-repo test to ensure factory can be constructed
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);
    let parsers = factory.get_all();
    assert!(
        !parsers.is_empty(),
        "ParserFactory should provide at least one parser"
    );
}

#[test]
fn factory_keeps_24bet_canonical_slug_and_legacy_alias() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let canonical = factory
        .get("_24bet")
        .expect("canonical _24bet slug should resolve");
    let legacy = factory
        .get("bet24")
        .expect("legacy bet24 alias should resolve");

    assert_eq!(canonical.slug(), "_24bet");
    assert_eq!(legacy.slug(), "_24bet");
    assert!(factory.contains("_24bet"));
    assert!(factory.contains("bet24"));
}

#[test]
fn factory_reports_sorted_registered_slugs() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);
    let slugs = factory.registered_slugs();

    assert!(slugs.windows(2).all(|w| w[0] <= w[1]));
    assert!(slugs.contains(&"_24bet".to_string()));
    assert!(slugs.contains(&"bet24".to_string()));
    assert!(slugs.contains(&"betm".to_string()));
    assert!(slugs.contains(&"fonbet".to_string()));
    assert!(slugs.contains(&"melbet".to_string()));
}

#[test]
fn factory_registers_melbet_parser() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let parser = factory.get("melbet").expect("melbet parser should resolve");
    assert_eq!(parser.slug(), "melbet");
    assert_eq!(parser.name(), "Melbet");
    assert!(parser.is_enabled());
}

#[test]
fn factory_registers_olimp_parser() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let parser = factory.get("olimp").expect("olimp parser should resolve");
    assert_eq!(parser.slug(), "olimp");
    assert_eq!(parser.name(), "Olimp");
    assert!(parser.is_enabled());
    assert!(parser.readiness().is_some());
}

#[test]
fn factory_locks_olimp_runtime_readiness_snapshot() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let coverage = factory
        .parser_coverage()
        .into_iter()
        .find(|item| item.slug == "olimp")
        .expect("Olimp coverage should be present");

    assert!(coverage.enabled);
    assert_eq!(coverage.parser_type, "api");

    let notes = coverage
        .notes
        .expect("Olimp coverage notes should be present");
    assert!(notes.contains("2026-04-18 runtime probe"));
    assert!(notes.contains("non-empty live and prematch event volume"));

    let readiness = coverage
        .readiness
        .expect("Olimp readiness should be present");
    assert_eq!(readiness.stage, ParserReadinessStage::RolloutReady);
    assert!(!readiness.production_enabled);
    assert!(readiness
        .checks
        .iter()
        .any(|check| check.code == "runtime_event_volume_observed"
            && matches!(check.severity, DiagnosticSeverity::Pass)
            && check.message.contains("445 live parseable events")
            && check.message.contains("1110 prematch parseable events")));
    assert!(readiness
        .checks
        .iter()
        .any(|check| check.code == "production_volume_still_unlocked"
            && matches!(check.severity, DiagnosticSeverity::Warn)));
}

#[test]
fn factory_registers_betm_parser() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let parser = factory.get("betm").expect("betm parser should resolve");
    assert_eq!(parser.slug(), "betm");
    assert_eq!(parser.name(), "Bet-M");
    assert!(!parser.is_enabled());
    assert!(parser.readiness().is_some());
}

#[test]
fn factory_surfaces_ligastavok_readiness() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let coverage = factory
        .parser_coverage()
        .into_iter()
        .find(|item| item.slug == "ligastavok")
        .expect("Liga Stavok coverage should be present");

    assert_eq!(coverage.parser_type, "api");
    assert_eq!(coverage.source, "crates/parsers/src/ligastavok.rs");
    assert!(coverage.readiness.is_some());
    assert!(!coverage.enabled);
}

#[test]
fn factory_surfaces_ligastavok_health_snapshot() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let health = factory
        .parser_health_snapshots()
        .into_iter()
        .find(|item| item.bookmaker == "ligastavok")
        .expect("Liga Stavok health should be present");

    assert!(matches!(health.status, shared::HealthStatus::Degraded));
    assert!(health.readiness.is_some());
    assert!(health
        .diagnostics
        .iter()
        .any(|check| check.code == "qrator_unattended_bootstrap_unverified"));
}

#[test]
fn factory_surfaces_zenit_readiness() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let coverage = factory
        .parser_coverage()
        .into_iter()
        .find(|item| item.slug == "zenit")
        .expect("Zenit coverage should be present");

    let readiness = coverage
        .readiness
        .expect("Zenit readiness should be present");
    assert!(matches!(
        readiness.stage,
        shared::ParserReadinessStage::RolloutReady
    ));
    assert!(!readiness.production_enabled);
    assert!(readiness
        .checks
        .iter()
        .any(|check| check.code == "strict_nightly_regressed_to_zero"));
}

#[test]
fn factory_locks_tennisi_as_direct_response_html_coverage() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let coverage = factory
        .parser_coverage()
        .into_iter()
        .find(|item| item.slug == "tennisi")
        .expect("Tennisi coverage should be present");

    let notes = coverage
        .notes
        .expect("Tennisi coverage notes should be present");
    assert_eq!(coverage.parser_type, "api");
    assert!(coverage.enabled);
    assert!(notes.contains("direct Tennisi line/live HTML responses"));
    assert!(notes.contains("category discovery"));
    assert!(!notes.contains("headless"));
    assert!(!notes.contains("Playwright"));
    assert!(!notes.contains("intercept"));
}

#[test]
fn factory_locks_baltbet_production_readiness_snapshot() {
    let client = Arc::new(Client::builder().build().expect("failed to build client"));
    let factory = ParserFactory::new(client);

    let coverage = factory
        .parser_coverage()
        .into_iter()
        .find(|item| item.slug == "baltbet")
        .expect("Baltbet coverage should be present");

    assert!(coverage.enabled);
    assert_eq!(coverage.parser_type, "api");

    let readiness = coverage
        .readiness
        .expect("Baltbet readiness should be present");
    assert_eq!(readiness.stage, ParserReadinessStage::Production);
    assert!(readiness.production_enabled);
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

    let health = factory
        .parser_health_snapshots()
        .into_iter()
        .find(|item| item.bookmaker == "baltbet")
        .expect("Baltbet health should be present");

    assert!(matches!(health.status, HealthStatus::Degraded));
    assert!(health
        .diagnostics
        .iter()
        .any(|check| check.code == "strict_live_kpi_recently_met"));
}
