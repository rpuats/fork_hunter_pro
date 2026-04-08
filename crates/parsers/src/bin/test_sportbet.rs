use parsers::sportbet::SportbetParser;
use parsers::base::BookmakerParser;
use std::sync::Arc;
use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("=== ТЕСТ SPORTBET ===\n");

    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0")
            .build()
            .unwrap(),
    );

    let parser = SportbetParser::new(client);
    let start = Instant::now();

    match parser.fetch_all().await {
        Ok(result) => {
            let elapsed = start.elapsed().as_millis();
            println!("Событий: {}", result.events.len());
            println!("Коэффициентов: {}", result.odds.len());
            println!("Время: {}ms\n", elapsed);

            // Группировка по видам спорта
            let mut by_sport: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for e in &result.events {
                let s = format!("{:?}", e.sport);
                *by_sport.entry(s).or_insert(0) += 1;
            }
            println!("По видам спорта:");
            for (sport, count) in &by_sport {
                println!("  {}: {}", sport, count);
            }

            println!("\nПервые 5 событий:");
            for event in result.events.iter().take(5) {
                println!("  > {} vs {} (L: {}, Sport: {:?})",
                    event.home_team, event.away_team, event.league, event.sport);
            }

            println!("\nПервые 5 коэффициентов:");
            for odd in result.odds.iter().take(5) {
                println!("  {} @ {} ({})", odd.selection, odd.odds, odd.market);
            }
        }
        Err(e) => println!("Ошибка: {}", e),
    }

    println!("\n=== ГОТОВО ===");
}
