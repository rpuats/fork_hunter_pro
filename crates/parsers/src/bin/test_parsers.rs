use parsers::parser_factory::ParserFactory;
use std::sync::Arc;
use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("=== ТЕСТ ПАРСЕРОВ ===\n");

    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap(),
    );

    let factory = ParserFactory::new(client);
    let parsers = factory.get_enabled();

    println!("Найдено {} парсеров\n", parsers.len());

    for parser in &parsers {
        println!("--- {} ---", parser.name());
        let start = Instant::now();

        match tokio::time::timeout(std::time::Duration::from_secs(60), parser.fetch_all()).await {
            Ok(Ok(result)) => {
                let elapsed = start.elapsed().as_millis();
                println!("  Событий: {}", result.events.len());
                println!("  Коэффициентов: {}", result.odds.len());
                println!("  Время: {}ms", elapsed);

                // Покажу первые 2 события
                for event in result.events.iter().take(2) {
                    println!("  > {} vs {} (L: {}, Live: {})",
                        event.home_team, event.away_team, event.league, event.is_live);
                }

                // Покажу первые 3 коэффициента
                for odd in result.odds.iter().take(3) {
                    println!("    Odd: {} @ {} ({})", odd.selection, odd.odds, odd.market);
                }
            }
            Ok(Err(e)) => {
                println!("  ОШИБКА: {}", e);
            }
            Err(_) => {
                println!("  ТАЙМАУТ (60s)");
            }
        }
        println!();
    }

    println!("=== ГОТОВО ===");
}
