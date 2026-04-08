use crate::base::{BookmakerParser, ParserResult};
use crate::factors_catalog::FactorsCatalog;
use async_trait::async_trait;
use reqwest::Client;
use shared::{Event, Odd};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Bettery API — shared platform, scopeMarket=501
/// live: https://line51.at58f5-resources.com/events/list?lang=ru&scopeMarket=501
/// prematch: https://line01.at58f5-resources.com/events/listBase?lang=ru&scopeMarket=501
#[derive(Debug)]
pub struct BetteryParser {
    client: Arc<Client>,
    live_url: String,
    prematch_url: String,
    factors: Arc<FactorsCatalog>,
}

impl BetteryParser {
    pub fn new(client: Arc<Client>) -> Self {
        let base_url = "https://line51.at58f5-resources.com";
        Self {
            client: client.clone(),
            live_url: format!("{}/events/list?lang=ru&scopeMarket=501", base_url),
            prematch_url: format!("{}/events/listBase?lang=ru&scopeMarket=501", base_url),
            factors: Arc::new(FactorsCatalog::new(
                client,
                base_url,
                501,
            )),
        }
    }

    pub async fn load_factors(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        self.factors.load().await
    }
}

#[async_trait]
impl BookmakerParser for BetteryParser {
    fn name(&self) -> &str { "Bettery" }
    fn slug(&self) -> &str { "bettery" }
    fn is_enabled(&self) -> bool { true }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            eprintln!("[BETTERY] Fetching events: {}", url);
            match self.do_fetch(url, is_live).await {
                Ok(results) => {
                    for (event, _) in results {
                        all_events.push(event);
                    }
                }
                Err(e) => {
                    eprintln!("[BETTERY] Events fetch error: {}", e);
                }
            }
        }
        eprintln!("[BETTERY] Events parsed: {}", all_events.len());
        info!(count = all_events.len(), "Bettery events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(&self, _event_id: &str) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.do_fetch(url, is_live).await {
                Ok(results) => {
                    for (_, odds) in results {
                        all_odds.extend(odds);
                    }
                }
                Err(e) => {
                    eprintln!("[BETTERY] Odds fetch error: {}", e);
                }
            }
        }
        eprintln!("[BETTERY] Odds parsed: {}", all_odds.len());
        Ok(all_odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let events = self.fetch_events().await?;
        let odds = self.fetch_odds("").await?;
        let elapsed = start.elapsed().as_millis() as u64;
        eprintln!("[BETTERY] Fetch complete: {} events, {} odds", events.len(), odds.len());
        debug!(events = events.len(), odds = odds.len(), time_ms = elapsed, "Bettery fetch complete");
        Ok(ParserResult::new("bettery", events, odds, elapsed))
    }

    fn base_url(&self) -> &str { "https://bettery.ru" }
    fn user_agent(&self) -> &str { "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" }
}

impl BetteryParser {
    async fn do_fetch(&self, _url: &str, is_live: bool) -> Result<Vec<(Event, Vec<Odd>)>, Box<dyn std::error::Error + Send + Sync>> {
        let scope = "501";
        let suffix = if is_live { "events/list" } else { "events/listBase" };
        // Use the API base URL directly
        let api_base = "https://line51.at58f5-resources.com";
        let url = format!("{}/{}?lang=ru&scopeMarket={}", api_base, suffix, scope);

        eprintln!("[BETTERY] Creating client for {}", url);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        eprintln!("[BETTERY] Sending request...");
        let resp = client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .send()
            .await?;
        
        eprintln!("[BETTERY] Response: {}", resp.status());

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        eprintln!("[BETTERY] Parsing JSON...");
        let json: serde_json::Value = resp.json().await?;
        eprintln!("[BETTERY] JSON parsed...");
        let result = crate::marathon::parse_api_response(&json, is_live, "bettery", &self.factors);
        eprintln!("[BETTERY] Fetch complete");
        result
    }

    fn base_url(&self) -> &str { "https://bettery.ru" }
    fn user_agent(&self) -> &str { "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" }
}
