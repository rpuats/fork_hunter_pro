/// Comprehensive test for all new REST parsers
/// Tests Winline, BetBoom, 1xBet, Marathon, Melbet

use parsers::winline_rest::WinlineRestParser;
use parsers::betboom_rest::BetboomRestParser;
use parsers::onexbet_rest::OnexbetRestParser;
use parsers::marathon_rest::MarathonRestParser;
use parsers::melbet_rest::MelbetRestParser;
use reqwest::Client;
use std::sync::Arc;
use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║      COMPREHENSIVE REST API PARSER TEST SUITE             ║");
    println!("║  Testing all new bookmaker parsers                         ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let client = Arc::new(Client::new());
    
    let parsers: Vec<(&str, Box<dyn Fn(Arc<Client>) -> Box<dyn std::future::Future<Output = Result<Vec<shared::Event>, String>> + Send + 'static> + Send + Sync>)> = vec![
        // Note: This is a simplified structure for demonstration
        // In real code, you'd use trait objects or a factory pattern
    ];

    // Test each parser individually
    test_winline(client.clone()).await;
    test_betboom(client.clone()).await;
    test_onexbet(client.clone()).await;
    test_marathon(client.clone()).await;
    test_melbet(client).await;

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║                   TEST SUITE COMPLETED                    ║");
    println!("╚════════════════════════════════════════════════════════════╝");
}

async fn test_winline(client: Arc<Client>) {
    println!("\n📊 Testing Winline REST Parser");
    println!("─────────────────────────────────────────");
    let start = Instant::now();
    let parser = WinlineRestParser::new(client);
    
    match parser.fetch_events().await {
        Ok(events) => {
            let duration = start.elapsed();
            println!("  ✅ Status: SUCCESS");
            println!("  📈 Events found: {}", events.len());
            println!("  ⏱️  Time taken: {:.2}s", duration.as_secs_f64());
            if !events.is_empty() {
                let e = &events[0];
                println!("  🏀 First event: {} vs {}", e.home_team, e.away_team);
            }
        }
        Err(e) => {
            println!("  ❌ Status: FAILED");
            println!("  📝 Error: {}", e);
        }
    }
}

async fn test_betboom(client: Arc<Client>) {
    println!("\n📊 Testing BetBoom REST Parser");
    println!("─────────────────────────────────────────");
    let start = Instant::now();
    let parser = BetboomRestParser::new(client);
    
    match parser.fetch_events().await {
        Ok(events) => {
            let duration = start.elapsed();
            println!("  ✅ Status: SUCCESS");
            println!("  📈 Events found: {}", events.len());
            println!("  ⏱️  Time taken: {:.2}s", duration.as_secs_f64());
            if !events.is_empty() {
                let e = &events[0];
                println!("  🏀 First event: {} vs {}", e.home_team, e.away_team);
            }
        }
        Err(e) => {
            println!("  ❌ Status: FAILED");
            println!("  📝 Error: {}", e);
        }
    }
}

async fn test_onexbet(client: Arc<Client>) {
    println!("\n📊 Testing 1xBet REST Parser");
    println!("─────────────────────────────────────────");
    let start = Instant::now();
    let parser = OnexbetRestParser::new(client);
    
    match parser.fetch_events().await {
        Ok(events) => {
            let duration = start.elapsed();
            println!("  ✅ Status: SUCCESS");
            println!("  📈 Events found: {}", events.len());
            println!("  ⏱️  Time taken: {:.2}s", duration.as_secs_f64());
            if !events.is_empty() {
                let e = &events[0];
                println!("  🏀 First event: {} vs {}", e.home_team, e.away_team);
            }
        }
        Err(e) => {
            println!("  ❌ Status: FAILED");
            println!("  📝 Error: {}", e);
        }
    }
}

async fn test_marathon(client: Arc<Client>) {
    println!("\n📊 Testing Marathon REST Parser");
    println!("─────────────────────────────────────────");
    let start = Instant::now();
    let parser = MarathonRestParser::new(client);
    
    match parser.fetch_events().await {
        Ok(events) => {
            let duration = start.elapsed();
            println!("  ✅ Status: SUCCESS");
            println!("  📈 Events found: {}", events.len());
            println!("  ⏱️  Time taken: {:.2}s", duration.as_secs_f64());
            if !events.is_empty() {
                let e = &events[0];
                println!("  🏀 First event: {} vs {}", e.home_team, e.away_team);
            }
        }
        Err(e) => {
            println!("  ❌ Status: FAILED");
            println!("  📝 Error: {}", e);
        }
    }
}

async fn test_melbet(client: Arc<Client>) {
    println!("\n📊 Testing Melbet REST Parser");
    println!("─────────────────────────────────────────");
    let start = Instant::now();
    let parser = MelbetRestParser::new(client);
    
    match parser.fetch_events().await {
        Ok(events) => {
            let duration = start.elapsed();
            println!("  ✅ Status: SUCCESS");
            println!("  📈 Events found: {}", events.len());
            println!("  ⏱️  Time taken: {:.2}s", duration.as_secs_f64());
            if !events.is_empty() {
                let e = &events[0];
                println!("  🏀 First event: {} vs {}", e.home_team, e.away_team);
            }
        }
        Err(e) => {
            println!("  ❌ Status: FAILED");
            println!("  📝 Error: {}", e);
        }
    }
}
