use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .gzip(true)
        .build()?;

    println!("🔍 ПРОВЕРКА ПАРСЕРОВ: сколько реально live событий");

    // Проверим несколько парсеров
    let parsers = vec![
        (
            "Pari",
            "https://line-lb01-w.pb06e2-resources.com/events/list?lang=ru&scopeMarket=2300",
        ),
        (
            "Marathon",
            "https://www.marathonbet.com/su/live/getFeed?partner=195&lang=ru&feedType=1",
        ),
        ("Bettery", "https://bettery.ru/api/v1/live"),
        (
            "Fonbet",
            "https://line02w.bk6bba-resources.com/events/list?lang=ru&scopeMarket=1500&version=2",
        ),
    ];

    for (name, url) in parsers {
        println!("\n📊 {}: {}", name, url);
        println!("─────────────────────────────────────────────────");

        match check_parser(&client, url).await {
            Ok(stats) => {
                println!("✅ LIVE событий: {}", stats.live_count);
                println!("📅 Сегодняшних: {}", stats.today_count);
                println!("⏰ Уже идущих: {}", stats.ongoing_count);
                println!("📈 Всего событий: {}", stats.total_count);

                if !stats.samples.is_empty() {
                    println!("🎯 Примеры событий:");
                    for (i, sample) in stats.samples.iter().enumerate().take(3) {
                        println!("  {}. {}", i + 1, sample);
                    }
                }
            }
            Err(e) => {
                println!("❌ Ошибка: {}", e);
            }
        }
    }

    Ok(())
}

struct ParserStats {
    total_count: usize,
    live_count: usize,
    today_count: usize,
    ongoing_count: usize,
    samples: Vec<String>,
}

async fn check_parser(
    client: &Client,
    url: &str,
) -> Result<ParserStats, Box<dyn std::error::Error>> {
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Accept", "application/json")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()).into());
    }

    let json: Value = resp.json().await?;
    let events = json
        .get("events")
        .or_else(|| json.get("data"))
        .or_else(|| json.get("result"))
        .and_then(|v| v.as_array())
        .ok_or("Нет массива events")?;

    let now = Utc::now().timestamp() * 1000;
    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
        * 1000;
    let today_end = (Utc::now().date_naive() + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
        * 1000;

    let mut live_count = 0;
    let mut today_count = 0;
    let mut ongoing_count = 0;
    let mut samples = Vec::new();

    for event in events {
        if let (Some(team1), Some(team2)) = (
            event
                .get("team1")
                .or_else(|| event.get("home"))
                .and_then(|t| t.as_str()),
            event
                .get("team2")
                .or_else(|| event.get("away"))
                .and_then(|t| t.as_str()),
        ) {
            // Определяем время события
            let event_time = if let Some(ts) = event.get("startTime").and_then(|t| t.as_i64()) {
                ts
            } else if let Some(ts) = event.get("timestamp").and_then(|t| t.as_i64()) {
                ts * 1000 // секунды в миллисекунды
            } else {
                continue;
            };

            let event_str = format!(
                "{} vs {} (время: {})",
                team1,
                team2,
                chrono::DateTime::from_timestamp(event_time / 1000, 0)
                    .map(|dt: DateTime<Utc>| dt.format("%H:%M").to_string())
                    .unwrap_or("?".to_string())
            );

            // Классифицируем события
            if event_time >= today_start && event_time < today_end {
                today_count += 1;
                if event_time <= now {
                    ongoing_count += 1;
                    samples.push(format!("🔴 LIVE: {}", event_str));
                } else {
                    samples.push(format!("🟡 TODAY: {}", event_str));
                }
            } else if event_time < now {
                live_count += 1;
                samples.push(format!("🔴 PAST: {}", event_str));
            }
        }
    }

    Ok(ParserStats {
        total_count: events.len(),
        live_count,
        today_count,
        ongoing_count,
        samples,
    })
}
