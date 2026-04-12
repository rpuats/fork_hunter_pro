use reqwest::Client;
use std::sync::Arc;

use parsers::parser_factory::ParserFactory;

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
    assert!(slugs.contains(&"fonbet".to_string()));
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
        .any(|check| check.code == "session_bootstrap_pending"));
}
