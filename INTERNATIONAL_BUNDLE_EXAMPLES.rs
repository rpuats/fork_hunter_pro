// Implementation Examples for international_bundle.rs
// These examples show how to use the multi-BK parser system

// ============================================================================
// EXAMPLE 1: SIMPLE SINGLE PARSER
// ============================================================================

use parsers::international_bundle::{
    SBobetParser,
    InternationalConfig,
};
use std::sync::Arc;
use reqwest::Client;
use parsers::base::BookmakerParser;

#[tokio::main]
async fn example_single_parser() -> Result<(), Box<dyn std::error::Error>> {
    // Create HTTP client
    let client = Arc::new(Client::new());
    let config = InternationalConfig::default();

    // Create parser
    let parser = SBobetParser::new(client, config, None);

    // Fetch data
    let result = parser.fetch_all().await?;
    
    println!("Bookmaker: {}", result.bookmaker);
    println!("Events: {}", result.events.len());
    println!("Odds: {}", result.odds.len());
    println!("Time: {}ms", result.fetch_time_ms);

    Ok(())
}

// ============================================================================
// EXAMPLE 2: ALL PARSERS WITH FACTORY
// ============================================================================

use parsers::international_bundle::InternationalBundleFactory;

#[tokio::main]
async fn example_factory_all_parsers() -> Result<(), Box<dyn std::error::Error>> {
    let config = InternationalConfig::default();
    let client = Arc::new(Client::new());

    // Create factory
    let factory = InternationalBundleFactory::new(config, None, client);

    // Create all 3 parsers
    let parsers = factory.create_all("betscope_api_key_here".to_string());
    
    println!("Created {} parsers", parsers.len());

    // Fetch from all in parallel
    let mut handles = vec![];
    for parser in parsers {
        let handle = tokio::spawn(async move {
            match parser.fetch_all().await {
                Ok(result) => Some((result.bookmaker, result.events.len(), result.odds.len())),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    None
                }
            }
        });
        handles.push(handle);
    }

    // Collect and display results
    let mut total_events = 0;
    let mut total_odds = 0;
    
    for handle in handles {
        if let Ok(Some((bk, events, odds))) = handle.await {
            println!("{}: {} events, {} odds", bk, events, odds);
            total_events += events;
            total_odds += odds;
        }
    }

    println!("\nTotal: {} events, {} odds", total_events, total_odds);
    Ok(())
}

// ============================================================================
// EXAMPLE 3: ADVANCED CONFIG WITH PROXIES
// ============================================================================

#[tokio::main]
async fn example_with_proxies() -> Result<(), Box<dyn std::error::Error>> {
    // Custom configuration
    let mut config = InternationalConfig::default();
    config.max_retries = 5;
    config.retry_delay_ms = 200;
    config.backoff_multiplier = 1.5;
    config.timeout_secs = 45;
    config.proxy_rotation_enabled = true;

    // Proxy list
    let proxies = vec![
        "proxy1.example.com:8080".to_string(),
        "proxy2.example.com:8080".to_string(),
        "proxy3.example.com:8080".to_string(),
    ];

    let client = Arc::new(Client::new());
    let factory = InternationalBundleFactory::new(
        config,
        Some(proxies),
        client,
    );

    // Use the parsers with proxy rotation
    let sbobet = factory.create_sbobet();
    let result = sbobet.fetch_all().await?;
    
    println!("SBObet fetched: {} events in {}ms", 
        result.events.len(), 
        result.fetch_time_ms
    );

    Ok(())
}

// ============================================================================
// EXAMPLE 4: EVENT POOL WITH DEDUPLICATION
// ============================================================================

use parsers::international_bundle::EventPool;

fn example_event_pool() {
    // Create pool with max 1000 events
    let pool = EventPool::new(1000);

    // Create sample events
    let mut events = vec![];
    for i in 0..100 {
        let event = shared::Event {
            id: format!("event_{}", i),
            home: Some(format!("Home{}", i)),
            away: Some(format!("Away{}", i)),
            league: Some("Premier League".to_string()),
            sport: shared::Sport::Football,
            start_time: chrono::Utc::now(),
            status: "scheduled".to_string(),
            bookmaker: "sbobet".to_string(),
        };
        events.push(event);
    }

    // Add to pool
    pool.add_events(events.clone());
    println!("Pool size: {}", pool.size());

    // Try to add duplicates - they will be filtered
    pool.add_events(events.clone());
    println!("Pool size after duplicates: {}", pool.size()); // Still 100

    // Get events from pool
    let pooled = pool.get_events();
    println!("Retrieved {} events from pool", pooled.len());

    // Clear pool
    pool.clear();
    println!("Pool size after clear: {}", pool.size()); // 0
}

// ============================================================================
// EXAMPLE 5: CUSTOM RETRY POLICY
// ============================================================================

use parsers::international_bundle::RetryPolicy;
use std::time::Duration;

fn example_retry_policy() {
    let policy = RetryPolicy::new(
        4,      // max 4 attempts (0, 1, 2, 3)
        100,    // initial delay 100ms
        2.0,    // double on each retry
    );

    // Simulate retry loop
    for attempt in 0..5 {
        if policy.should_retry(attempt) {
            let delay = policy.calculate_delay(attempt);
            println!("Attempt {}: delay = {}ms", attempt, delay.as_millis());
        } else {
            println!("Attempt {}: giving up", attempt);
            break;
        }
    }
    
    // Output:
    // Attempt 0: delay = 100ms
    // Attempt 1: delay = 200ms
    // Attempt 2: delay = 400ms
    // Attempt 3: delay = 800ms
    // Attempt 4: giving up
}

// ============================================================================
// EXAMPLE 6: PROXY ROTATION
// ============================================================================

use parsers::international_bundle::ProxyRotator;

fn example_proxy_rotation() {
    let proxies = vec![
        "proxy1:8080".to_string(),
        "proxy2:8080".to_string(),
        "proxy3:8080".to_string(),
    ];

    let rotator = ProxyRotator::new(proxies, Duration::from_secs(300));

    // Get rotating proxies
    for i in 0..10 {
        let proxy = rotator.get_next();
        println!("Request {}: using {:?}", i, proxy);
    }

    // Ban a proxy
    if let Some(proxy) = rotator.get_next() {
        println!("Banning proxy: {}", proxy);
        rotator.ban_proxy(proxy);
    }

    // Subsequent calls will skip banned proxies
    for i in 0..5 {
        let proxy = rotator.get_next();
        println!("Request {}: using {:?}", i, proxy);
    }
}

// ============================================================================
// EXAMPLE 7: MONITORING & METRICS
// ============================================================================

#[tokio::main]
async fn example_monitoring() -> Result<(), Box<dyn std::error::Error>> {
    let config = InternationalConfig::default();
    let client = Arc::new(Client::new());
    let factory = InternationalBundleFactory::new(config, None, client);

    let parsers = factory.create_all("api_key".to_string());

    // Track performance
    let mut times = vec![];
    let mut event_counts = vec![];
    let mut names = vec![];

    for parser in &parsers {
        let start = std::time::Instant::now();
        match parser.fetch_all().await {
            Ok(result) => {
                let elapsed = start.elapsed().as_millis();
                times.push(elapsed);
                event_counts.push(result.events.len());
                names.push(result.bookmaker);
                
                println!("{}: {:.1} events/sec", 
                    result.bookmaker,
                    (result.events.len() as f64) / (result.fetch_time_ms as f64 / 1000.0)
                );
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    // Calculate statistics
    let avg_time: u128 = times.iter().sum::<u128>() / times.len() as u128;
    let total_events: usize = event_counts.iter().sum();
    let slowest = times.iter().max().unwrap_or(&0);
    let fastest = times.iter().min().unwrap_or(&0);

    println!("\n=== STATISTICS ===");
    println!("Total events: {}", total_events);
    println!("Average fetch time: {}ms", avg_time);
    println!("Fastest: {}ms", fastest);
    println!("Slowest: {}ms", slowest);
    println!("Combined throughput: {:.1} events/sec",
        (total_events as f64) / (avg_time as f64 / 1000.0)
    );

    Ok(())
}

// ============================================================================
// EXAMPLE 8: ERROR RECOVERY
// ============================================================================

#[tokio::main]
async fn example_error_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let config = InternationalConfig {
        max_retries: 3,
        retry_delay_ms: 100,
        backoff_multiplier: 2.0,
        ..Default::default()
    };

    let client = Arc::new(Client::new());
    let factory = InternationalBundleFactory::new(config, None, client);

    // Get all parsers
    let parsers = factory.create_all("api_key".to_string());

    // Try to fetch with error handling
    for parser in parsers {
        match parser.fetch_all().await {
            Ok(result) if !result.events.is_empty() => {
                println!("✓ {} - {} events", 
                    result.bookmaker, 
                    result.events.len()
                );
            }
            Ok(result) => {
                println!("⚠ {} - no events (using fallback cache)", 
                    result.bookmaker
                );
            }
            Err(e) => {
                println!("✗ {} - error: {}", parser.name(), e);
            }
        }
    }

    Ok(())
}

// ============================================================================
// EXAMPLE 9: FILTERING & PROCESSING
// ============================================================================

use shared::Sport;

#[tokio::main]
async fn example_filtering() -> Result<(), Box<dyn std::error::Error>> {
    let config = InternationalConfig::default();
    let client = Arc::new(Client::new());
    let factory = InternationalBundleFactory::new(config, None, client);

    let sbobet = factory.create_sbobet();
    let result = sbobet.fetch_all().await?;

    // Filter events
    let football_events: Vec<_> = result.events
        .iter()
        .filter(|e| e.sport == Sport::Football)
        .collect();

    println!("Football events: {}", football_events.len());

    // Filter odds by type
    let one_x_two_odds: Vec<_> = result.odds
        .iter()
        .filter(|o| o.odds_type == shared::odds::OddsType::OneXTwo)
        .collect();

    println!("1X2 odds: {}", one_x_two_odds.len());

    // Group by bookmaker
    let by_bk = result.events
        .iter()
        .fold(std::collections::HashMap::new(), |mut acc, e| {
            acc.entry(e.bookmaker.clone())
                .or_insert_with(Vec::new)
                .push(e.id.clone());
            acc
        });

    for (bk, ids) in by_bk {
        println!("{}: {} events", bk, ids.len());
    }

    Ok(())
}

// ============================================================================
// EXAMPLE 10: BATCH PROCESSING WITH INTERVALS
// ============================================================================

use tokio::time::{interval, Duration};

#[tokio::main]
async fn example_batch_processing() -> Result<(), Box<dyn std::error::Error>> {
    let config = InternationalConfig::default();
    let client = Arc::new(Client::new());
    let factory = InternationalBundleFactory::new(config, None, client);
    let pool = EventPool::new(10000);

    // Create ticker for periodic fetching
    let mut ticker = interval(Duration::from_secs(60));

    // Fetch every 60 seconds
    for i in 0..3 {
        ticker.tick().await;
        
        println!("=== Cycle {} ===", i + 1);
        let parsers = factory.create_all("api_key".to_string());

        for parser in parsers {
            if let Ok(result) = parser.fetch_all().await {
                pool.add_events(result.events);
                println!("Fetched {} new events from {}",
                    result.events.len(),
                    result.bookmaker
                );
            }
        }

        println!("Pool total: {} unique events\n", pool.size());
    }

    Ok(())
}

// ============================================================================
// TESTING: Unit Test Examples
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = InternationalConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_event_pool_dedup() {
        let pool = EventPool::new(100);
        
        let event = shared::Event {
            id: "1".to_string(),
            home: Some("A".to_string()),
            away: Some("B".to_string()),
            league: Some("L".to_string()),
            sport: shared::Sport::Football,
            start_time: chrono::Utc::now(),
            status: "active".to_string(),
            bookmaker: "test".to_string(),
        };

        pool.add_events(vec![event.clone()]);
        pool.add_events(vec![event]);
        assert_eq!(pool.size(), 1);
    }
}
