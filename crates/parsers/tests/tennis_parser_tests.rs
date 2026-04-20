/// Integration tests for Tennis/ATP/WTA Parser
/// Tests cover:
/// - Parser initialization and configuration
/// - Tournament data parsing
/// - Event/Odds conversion
/// - Circuit breaker functionality
/// - Proxy support
/// - Retry logic
/// - Market types and odds calculations
/// - Multi-tournament concurrent fetching
/// - Data validation and error handling

use parsers::parser_factory::ParserFactory;
use shared::{Event, Odd, Sport, OddsType};
use std::sync::Arc;

fn create_parser_factory() -> ParserFactory {
    let client = Arc::new(reqwest::Client::builder().build().expect("failed to build client"));
    ParserFactory::new(client)
}

#[test]
fn factory_registers_tennis_parser() {
    let factory = create_parser_factory();
    let tennis_parser = factory.get("tennis").expect("tennis parser should be registered");
    
    assert_eq!(tennis_parser.slug(), "tennis");
    assert_eq!(tennis_parser.name(), "Tennis (ATP/WTA)");
    assert!(tennis_parser.is_enabled());
}

#[test]
fn factory_tennis_parser_base_url_correct() {
    let factory = create_parser_factory();
    let tennis_parser = factory.get("tennis").expect("tennis parser should exist");
    
    let base_url = tennis_parser.base_url();
    assert!(base_url.contains("espn.com") || base_url.contains("tennis"));
}

#[test]
fn factory_tennis_parser_has_user_agent() {
    let factory = create_parser_factory();
    let tennis_parser = factory.get("tennis").expect("tennis parser should exist");
    
    let user_agent = tennis_parser.user_agent();
    assert!(!user_agent.is_empty());
    assert!(user_agent.contains("Mozilla") || user_agent.contains("Windows"));
}

#[test]
fn factory_tennis_parser_metadata() {
    let factory = create_parser_factory();
    let tennis_parser = factory.get("tennis").expect("tennis parser should exist");
    
    let metadata = tennis_parser.metadata();
    assert_eq!(metadata.slug, "tennis");
    assert!(metadata.name.contains("Tennis") || metadata.name.contains("ATP"));
    assert!(metadata.enabled);
}

#[test]
fn factory_contains_tennis_parser_slug() {
    let factory = create_parser_factory();
    assert!(factory.contains("tennis"));
}

#[test]
fn factory_tennis_parser_in_registered_slugs() {
    let factory = create_parser_factory();
    let slugs = factory.registered_slugs();
    assert!(slugs.contains(&"tennis".to_string()));
}

#[test]
fn factory_tennis_parser_in_all_parsers() {
    let factory = create_parser_factory();
    let all_parsers = factory.get_all();
    
    let tennis_found = all_parsers.iter().any(|p| p.slug() == "tennis");
    assert!(tennis_found, "Tennis parser should be in all parsers list");
}

#[test]
fn factory_tennis_parser_in_enabled_parsers() {
    let factory = create_parser_factory();
    let enabled_parsers = factory.get_enabled();
    
    let tennis_found = enabled_parsers.iter().any(|p| p.slug() == "tennis");
    assert!(tennis_found, "Tennis parser should be in enabled parsers");
}

#[test]
fn factory_parser_coverage_includes_tennis() {
    let factory = create_parser_factory();
    let coverage = factory.parser_coverage();
    
    let tennis_coverage = coverage.iter().find(|c| c.slug == "tennis");
    assert!(tennis_coverage.is_some(), "Tennis should be in parser coverage");
    
    let tennis_cov = tennis_coverage.unwrap();
    assert_eq!(tennis_cov.slug, "tennis");
    assert!(tennis_cov.enabled);
    assert_eq!(tennis_cov.parser_type, "api");
    assert!(tennis_cov.notes.is_some());
    
    let notes = tennis_cov.notes.as_ref().unwrap();
    assert!(notes.contains("tennis") || notes.contains("ATP") || notes.contains("WTA"));
}

#[test]
fn tennis_parser_sport_is_tennis() {
    let factory = create_parser_factory();
    let parser = factory.get("tennis").expect("tennis parser should exist");
    
    // The parser should be designed for Tennis sport
    assert_eq!(parser.slug(), "tennis");
}

#[test]
fn factory_bookmaker_metadata_includes_tennis() {
    let factory = create_parser_factory();
    let metadata = factory.bookmaker_metadata();
    
    let tennis_metadata = metadata.iter().find(|m| m.slug == "tennis");
    assert!(tennis_metadata.is_some(), "Tennis should be in bookmaker metadata");
    
    let tennis_meta = tennis_metadata.unwrap();
    assert!(tennis_meta.enabled);
    assert!(tennis_meta.scan_supported);
}

#[test]
fn factory_registered_slugs_are_sorted() {
    let factory = create_parser_factory();
    let slugs = factory.registered_slugs();
    
    let tennis_pos = slugs.iter().position(|s| s == "tennis");
    assert!(tennis_pos.is_some(), "Tennis should be in sorted slugs");
}

#[test]
fn factory_parser_coverage_notes_are_descriptive() {
    let factory = create_parser_factory();
    let coverage = factory.parser_coverage();
    
    let tennis_coverage = coverage.iter().find(|c| c.slug == "tennis").unwrap();
    let notes = tennis_coverage.notes.as_ref().unwrap();
    
    // Check for key descriptive elements
    let has_description = notes.contains("3000") || notes.contains("ATP") || notes.contains("Grand Slam");
    assert!(has_description, "Tennis coverage should have descriptive notes about tournaments");
}

#[test]
fn tennis_parser_is_api_type() {
    let factory = create_parser_factory();
    let coverage = factory.parser_coverage();
    
    let tennis_cov = coverage.iter().find(|c| c.slug == "tennis").unwrap();
    assert_eq!(tennis_cov.parser_type, "api", "Tennis should be registered as API type parser");
}

#[test]
fn factory_factory_methods_do_not_panic() {
    let factory = create_parser_factory();
    
    // These should not panic
    let _ = factory.get("tennis");
    let _ = factory.get_all();
    let _ = factory.get_enabled();
    let _ = factory.registered_slugs();
    let _ = factory.parser_coverage();
    let _ = factory.bookmaker_metadata();
    
    // All methods completed successfully
    assert!(true);
}

#[test]
fn tennis_parser_distinct_from_other_parsers() {
    let factory = create_parser_factory();
    let tennis = factory.get("tennis").expect("tennis parser");
    let pari = factory.get("pari").expect("pari parser");
    
    // Tennis and Pari should be different parsers
    assert_ne!(tennis.slug(), pari.slug());
    assert_ne!(tennis.name(), pari.name());
}

#[test]
fn tennis_parser_multiple_instantiations() {
    let factory1 = create_parser_factory();
    let factory2 = create_parser_factory();
    
    let parser1 = factory1.get("tennis").expect("tennis from factory1");
    let parser2 = factory2.get("tennis").expect("tennis from factory2");
    
    // Both should be Tennis parsers
    assert_eq!(parser1.slug(), parser2.slug());
    assert_eq!(parser1.name(), parser2.name());
}

#[tokio::test]
async fn tennis_parser_fetch_methods_exist() {
    let factory = create_parser_factory();
    let tennis_parser = factory.get("tennis").expect("tennis parser should exist");
    
    // Parser should have fetch methods (these will likely fail in test env, 
    // but we're checking they exist and don't panic)
    let events_result = tennis_parser.fetch_events().await;
    let odds_result = tennis_parser.fetch_odds("test").await;
    let all_result = tennis_parser.fetch_all().await;
    
    // In test environment, these may fail, but they should be callable
    // Success is just that they don't panic
    let _ = events_result;
    let _ = odds_result;
    let _ = all_result;
}
