use parsers::base::BookmakerParser;
use parsers::sportbet::SportbetParser;
use std::sync::Arc;
use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("=== ТЕСТ ПАРСЕРОВ ===\n");

    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0")
            .build()
            .unwrap(),
    );

    // Тест Sportbet
    {
        println!("--- Sportbet ---");
        let parser = SportbetParser::new(client.clone());
        let start = Instant::now();

        match parser.fetch_all().await {
            Ok(result) => {
                let elapsed = start.elapsed().as_millis();
                println!("  Событий: {}", result.events.len());
                println!("  Коэффициентов: {}", result.odds.len());
                println!("  Время: {}ms", elapsed);

                // Группировка по видам спорта
                let mut by_sport: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                for e in &result.events {
                    let s = format!("{:?}", e.sport);
                    *by_sport.entry(s).or_insert(0) += 1;
                }
                println!("  По видам спорта:");
                for (sport, count) in &by_sport {
                    println!("    {}: {}", sport, count);
                }
            }
            Err(e) => println!("  Ошибка: {}", e),
        }
        println!();
    }

    // Тест Olimp
    {
        println!("--- Olimp ---");
        use parsers::olimp::OlimpParser;
        let parser = OlimpParser::new(client.clone());
        let start = Instant::now();

        match parser.fetch_all().await {
            Ok(result) => {
                let elapsed = start.elapsed().as_millis();
                println!("  Событий: {}", result.events.len());
                println!("  Коэффициентов: {}", result.odds.len());
                println!("  Время: {}ms", elapsed);
            }
            Err(e) => println!("  Ошибка: {}", e),
        }
        println!();
    }

    println!("=== ГОТОВО ===");
}
