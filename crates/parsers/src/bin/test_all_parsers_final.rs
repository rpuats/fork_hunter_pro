use parsers::base::BookmakerParser;
use parsers::parser_factory::ParserFactory;
use std::sync::Arc;
use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("=== ФИНАЛЬНЫЙ ТЕСТ ВСЕХ ПАРСЕРОВ ===\n");

    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0")
            .build()
            .unwrap(),
    );

    let factory = ParserFactory::new(client);
    let parsers = factory.get_enabled();

    println!("Активных парсеров: {}\n", parsers.len());

    let mut total_events = 0;
    let mut total_odds = 0;
    let mut results: Vec<(String, usize, usize, u128)> = Vec::new();

    for parser in &parsers {
        let start = Instant::now();
        match tokio::time::timeout(std::time::Duration::from_secs(45), parser.fetch_all()).await {
            Ok(Ok(result)) => {
                let elapsed = start.elapsed().as_millis();
                let events = result.events.len();
                let odds = result.odds.len();
                total_events += events;
                total_odds += odds;
                results.push((parser.name().to_string(), events, odds, elapsed));
                println!("{}: {} событий, {} коэффициентов, {}ms", 
                    parser.name(), events, odds, elapsed);
            }
            Ok(Err(e)) => {
                println!("{}: ОШИБКА - {}", parser.name(), e);
                results.push((parser.name().to_string(), 0, 0, 0));
            }
            Err(_) => {
                println!("{}: ТАЙМАУТ (45s)", parser.name());
                results.push((parser.name().to_string(), 0, 0, 0));
            }
        }
    }

    println!("\n=== ИТОГИ ===");
    println!("Всего событий: {}", total_events);
    println!("Всего коэффициентов: {}", total_odds);
    println!("\nДетализация:");
    for (name, events, odds, time) in &results {
        let status = if *events > 0 { "✅" } else { "❌" };
        println!("  {} {} - {} событий, {} коэффициентов, {}ms", 
            status, name, events, odds, time);
    }

    println!("\n=== ГОТОВО ===");
}
