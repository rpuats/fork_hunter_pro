use chrono::Utc;
use reqwest::Client;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .gzip(true)
        .build()?;

    let live_url = "https://line-lb01-w.pb06e2-resources.com/events/list?lang=ru&scopeMarket=2300";
    println!("Проверяем LIVE endpoint: {}", live_url);

    let resp = client
        .get(live_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .header("Accept", "application/json")
        .send()
        .await?;

    if resp.status().is_success() {
        let json: Value = resp.json().await?;
        if let Some(events) = json.get("events").and_then(|e| e.as_array()) {
            println!("LIVE events count: {}", events.len());

            let now = Utc::now().timestamp() * 1000;
            let mut live_count = 0;
            let mut upcoming_count = 0;

            for event in events {
                if let Some(start_time) = event.get("startTime").and_then(|t| t.as_i64()) {
                    if start_time <= now {
                        live_count += 1;
                    } else {
                        upcoming_count += 1;
                    }
                }
            }

            println!("Уже идущих матчей: {}", live_count);
            println!("Ожидаемых сегодня: {}", upcoming_count);
        } else {
            println!("Нет events в ответе");
        }
    } else {
        println!("HTTP error: {}", resp.status());
    }

    Ok(())
}
