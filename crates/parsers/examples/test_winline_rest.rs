use parsers::winline_rest::WinlineRestParser;
use reqwest::Client;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    println!("Testing Winline REST Parser...\n");

    let client = Arc::new(Client::new());
    let parser = WinlineRestParser::new(client);

    match parser.fetch_events().await {
        Ok(events) => {
            println!("✅ SUCCESS: Fetched {} events", events.len());
            if !events.is_empty() {
                println!("\nFirst event:");
                let e = &events[0];
                println!("  ID: {}", e.id);
                println!("  Match: {} vs {}", e.home_team, e.away_team);
                println!("  League: {}", e.league);
            }
        }
        Err(e) => {
            println!("❌ FAILED: {}", e);
        }
    }
}
