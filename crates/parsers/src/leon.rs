use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Leon API parser
/// API: leon.ru/api-2/betline/events/inplay and prematch
#[derive(Debug)]
pub struct LeonParser {
    client: Arc<Client>,
    live_url: String,
    prematch_url: String,
}

impl LeonParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            live_url: "https://leon.ru/api-2/betline/events/inplay?ctag=ru-RU".to_string(),
            prematch_url: "https://leon.ru/api-2/betline/events/prematch?ctag=ru-RU".to_string(),
        }
    }
}

#[async_trait]
impl BookmakerParser for LeonParser {
    fn name(&self) -> &str {
        "Leon"
    }
    fn slug(&self) -> &str {
        "leon"
    }
    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((events, _)) => all_events.extend(events),
                Err(e) => warn!(error = %e, "Leon fetch events failed"),
            }
        }
        info!(count = all_events.len(), "Leon events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((_, odds)) => all_odds.extend(odds),
                Err(e) => warn!(error = %e, "Leon fetch odds failed"),
            }
        }
        Ok(all_odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((events, odds)) => {
                    all_events.extend(events);
                    all_odds.extend(odds);
                }
                Err(e) => warn!(error = %e, "Leon fetch failed"),
            }
        }

        let elapsed = start.elapsed().as_millis() as u64;
        info!(
            events = all_events.len(),
            odds = all_odds.len(),
            time_ms = elapsed,
            "Leon fetch complete"
        );
        Ok(ParserResult::new("leon", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://leon.ru"
    }
    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    }
}

impl LeonParser {
    async fn fetch_api(
        &self,
        url: &str,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        debug!(url = url, "Leon: fetching");

        let resp = self.client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .header("Referer", "https://leon.ru/")
            .header("Origin", "https://leon.ru")
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        if !resp.status().is_success() {
            debug!(status = %resp.status(), "Leon: API failed");
            return Ok((Vec::new(), Vec::new()));
        }

        let json: serde_json::Value = resp.json().await?;
        Self::parse_api_response(&json, is_live)
    }

    fn parse_api_response(
        json: &serde_json::Value,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let mut events = Vec::new();
        let mut all_odds = Vec::new();
        let now = Utc::now();

        // API structure: { events: [ { id, competitors: [{name}, {name}], league: {name}, markets: [...] } ] }
        let events_array = match json.get("events").and_then(|e| e.as_array()) {
            Some(e) => e,
            None => {
                debug!("Leon: no events found");
                return Ok((Vec::new(), Vec::new()));
            }
        };

        for event_data in events_array {
            let (event_id, home, away, league_name) = match Self::extract_event_info(event_data) {
                Some(info) => info,
                None => continue,
            };

            let event_key = format!("leon-{}", event_id);
            let league = league_name
                .unwrap_or_else(|| if is_live { "Live" } else { "Prematch" }.to_string());

            let sport = Sport::Football; // Leon API возвращает смешанные виды спорта, пока футбол

            let event = Event {
                id: event_key.clone(),
                sport,
                league: league.clone(),
                home_team: home.clone(),
                away_team: away.clone(),
                start_time: None,
                is_live,
                bookmaker_slug: "leon".to_string(),
                raw_url: None,
                extra: HashMap::new(),
            };
            events.push(event);

            // Parse markets into odds
            if let Some(markets) = event_data.get("markets").and_then(|m| m.as_array()) {
                for market in markets {
                    let market_name = market
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    if let Some(runners) = market.get("runners").and_then(|r| r.as_array()) {
                        for runner in runners {
                            if let (Some(selection), Some(price)) = (
                                runner.get("name").and_then(|v| v.as_str()),
                                runner.get("price").and_then(|v| v.as_f64()),
                            ) {
                                if price > 1.0 {
                                    let odds_type =
                                        Self::selection_to_odds_type(selection, market_name);
                                    all_odds.push(Odd {
                                        id: format!("{}-{}-{}", event_key, market_name, selection),
                                        event_id: event_key.clone(),
                                        bookmaker_slug: "leon".to_string(),
                                        market: market_name.to_string(),
                                        selection: selection.to_string(),
                                        odds: price,
                                        odds_type,
                                        line: None,
                                        timestamp: now,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        debug!(events = events.len(), odds = all_odds.len(), "Leon: parsed");
        Ok((events, all_odds))
    }

    fn extract_event_info(
        data: &serde_json::Value,
    ) -> Option<(String, String, String, Option<String>)> {
        let event_id = data.get("id")?.to_string().trim_matches('"').to_string();

        let competitors = data.get("competitors").and_then(|c| c.as_array())?;
        if competitors.len() < 2 {
            return None;
        }

        let home = competitors
            .get(0)
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())?
            .to_string();

        let away = competitors
            .get(1)
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())?
            .to_string();

        let league = data
            .get("league")
            .and_then(|l| l.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some((event_id, home, away, league))
    }

    fn selection_to_odds_type(selection: &str, market_name: &str) -> OddsType {
        let lower = selection.to_lowercase();
        let market_lower = market_name.to_lowercase();

        if market_lower.contains("тотал") || market_lower.contains("total") {
            if lower.contains("больше") || lower.contains("over") || lower == "тб" || lower == "tb"
            {
                return OddsType::Over;
            } else if lower.contains("меньше")
                || lower.contains("under")
                || lower == "тм"
                || lower == "tm"
            {
                return OddsType::Under;
            }
        }

        if market_lower.contains("фора") || market_lower.contains("handicap") {
            return OddsType::Handicap;
        }

        match lower.as_str() {
            "1" | "п1" | "home" => OddsType::Home,
            "x" | "ничья" | "draw" | "tie" => OddsType::Draw,
            "2" | "п2" | "away" => OddsType::Away,
            "тб" | "tb" | "over" | "больше" => OddsType::Over,
            "тм" | "tm" | "under" | "меньше" => OddsType::Under,
            _ => OddsType::Custom,
        }
    }
}
