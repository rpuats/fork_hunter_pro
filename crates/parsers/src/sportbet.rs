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

/// Sportbet API parser v1
/// API: sportbet.ru/sport/v1/fixtures-tree-live and rating-fixtures-tree
#[derive(Debug)]
pub struct SportbetParser {
    client: Arc<Client>,
    live_url: String,
    prematch_url: String,
}

impl SportbetParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            // CRITICAL: `all-fixtures-tree` returns ALL events (~350), `rating-fixtures-tree` only returns featured (~115)
            live_url: "https://sportbet.ru/sport/v1/fixtures-tree-live".to_string(),
            prematch_url: "https://sportbet.ru/sport/v1/all-fixtures-tree?period=ALL".to_string(),
        }
    }
}

#[async_trait]
impl BookmakerParser for SportbetParser {
    fn name(&self) -> &str { "Sportbet" }
    fn slug(&self) -> &str { "sportbet" }
    fn is_enabled(&self) -> bool { true }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((events, _)) => all_events.extend(events),
                Err(e) => warn!(error = %e, "Sportbet fetch events failed"),
            }
        }
        info!(count = all_events.len(), "Sportbet events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(&self, _event_id: &str) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((_, odds)) => all_odds.extend(odds),
                Err(e) => warn!(error = %e, "Sportbet fetch odds failed"),
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
                Err(e) => warn!(error = %e, "Sportbet fetch failed"),
            }
        }

        let elapsed = start.elapsed().as_millis() as u64;
        info!(events = all_events.len(), odds = all_odds.len(), time_ms = elapsed, "Sportbet fetch complete");
        Ok(ParserResult::new("sportbet", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str { "https://sportbet.ru" }
    fn user_agent(&self) -> &str { "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" }
}

impl SportbetParser {
    async fn fetch_api(&self, url: &str, is_live: bool) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        debug!(url = url, "Sportbet: fetching");

        let resp = self.client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .header("Referer", "https://sportbet.ru/")
            .timeout(Duration::from_secs(20))
            .send()
            .await?;

        if !resp.status().is_success() {
            debug!(status = %resp.status(), "Sportbet: API failed");
            return Ok((Vec::new(), Vec::new()));
        }

        let json: serde_json::Value = resp.json().await?;
        Self::parse_api_response(&json, is_live)
    }

    fn parse_api_response(json: &serde_json::Value, is_live: bool) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let mut events = Vec::new();
        let mut all_odds = Vec::new();
        let now = Utc::now();

        // API structure:
        // fixtures: { event_id: { c: [{n: "Home"}, {n: "Away"}], l: league_id, s: sport_id } }
        // m: { event_id: [ {n: "1x2", m: [ {sel: [{n: "Home", o: 1.85}, ...]} ] } ] }
        // l: [{i: league_id, l: "League Name"}]

        let fixtures = match json.get("fixtures").and_then(|f| f.as_object()) {
            Some(f) => f,
            None => {
                debug!("Sportbet: no fixtures found");
                return Ok((Vec::new(), Vec::new()));
            }
        };

        let matches = json.get("m").and_then(|m| m.as_object());
        let leagues: HashMap<String, String> = json.get("l")
            .and_then(|l| l.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let id = item.get("i").and_then(|v| v.as_u64())?;
                        let name = item.get("l").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        Some((id.to_string(), name))
                    })
                    .collect()
            })
            .unwrap_or_default();

        for (event_id, fixture_data) in fixtures {
            let (home, away, league_id, sport_id, league_name_from_fixture) = match Self::extract_event_info(fixture_data) {
                Some(info) => info,
                None => continue,
            };

            // Use league name from fixture if available, otherwise lookup in leagues array
            let league_name = if !league_name_from_fixture.is_empty() {
                league_name_from_fixture
            } else {
                leagues.get(&league_id.to_string())
                    .cloned()
                    .unwrap_or_else(|| if is_live { "Live" } else { "Prematch" }.to_string())
            };

            let sport = Self::sport_id_to_sport(sport_id);
            let event_key = format!("sportbet-{}", event_id);

            let event = Event {
                id: event_key.clone(),
                sport,
                league: league_name.clone(),
                home_team: home.clone(),
                away_team: away.clone(),
                start_time: None,
                is_live,
                bookmaker_slug: "sportbet".to_string(),
                raw_url: None,
                extra: HashMap::new(),
            };
            events.push(event);

            // Parse markets from "m" section
            if let Some(event_markets) = matches.and_then(|m| m.get(event_id)).and_then(|m| m.as_array()) {
                for market in event_markets {
                    let market_name = market.get("n").and_then(|v| v.as_str()).unwrap_or("unknown");

                    if let Some(market_items) = market.get("m").and_then(|m| m.as_array()) {
                        for item in market_items {
                            if let Some(selections) = item.get("sel").and_then(|s| s.as_array()) {
                                for sel in selections {
                                    if let (Some(selection_name), Some(odds_val)) = (
                                        sel.get("n").and_then(|v| v.as_str()),
                                        sel.get("o").and_then(|v| v.as_f64()),
                                    ) {
                                        if odds_val > 1.0 {
                                            let odds_type = Self::selection_to_odds_type(selection_name, market_name);
                                            all_odds.push(Odd {
                                                id: format!("{}-{}-{}", event_key, market_name, selection_name),
                                                event_id: event_key.clone(),
                                                bookmaker_slug: "sportbet".to_string(),
                                                market: market_name.to_string(),
                                                selection: selection_name.to_string(),
                                                odds: odds_val,
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
            }
        }

        debug!(events = events.len(), odds = all_odds.len(), "Sportbet: parsed");
        Ok((events, all_odds))
    }

    fn extract_event_info(data: &serde_json::Value) -> Option<(String, String, u64, u64, String)> {
        let competitors = data.get("c").and_then(|c| c.as_array())?;
        if competitors.len() < 2 {
            return None;
        }

        let home = competitors.get(0)
            .and_then(|c| c.get("n"))
            .and_then(|v| v.as_str())?
            .to_string();

        let away = competitors.get(1)
            .and_then(|c| c.get("n"))
            .and_then(|v| v.as_str())?
            .to_string();

        // CRITICAL: `s` is now match status ("NOT_STARTED"/"LIVE"), real sport ID is in `sid`
        let league_id = data.get("tid").or_else(|| data.get("l")).and_then(|v| v.as_u64()).unwrap_or(0);
        let sport_id = data.get("sid").and_then(|v| v.as_u64()).unwrap_or(1);
        // Tournament name is directly in the fixture
        let league_name = data.get("tn").or_else(|| data.get("l"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Some((home, away, league_id, sport_id, league_name))
    }

    fn selection_to_odds_type(selection: &str, market_name: &str) -> OddsType {
        let lower = selection.to_lowercase();
        let market_lower = market_name.to_lowercase();

        if market_lower.contains("total") || market_lower.contains("тотал") {
            if lower.contains("over") || lower.contains("больше") || lower.contains("tb") {
                return OddsType::Over;
            } else if lower.contains("under") || lower.contains("меньше") || lower.contains("tm") {
                return OddsType::Under;
            }
        }

        if market_lower.contains("handicap") || market_lower.contains("фора") {
            return OddsType::Handicap;
        }

        match lower.as_str() {
            "1" | "home" | "п1" => OddsType::Home,
            "x" | "draw" | "ничья" => OddsType::Draw,
            "2" | "away" | "п2" => OddsType::Away,
            "over" | "больше" | "тб" | "tb" => OddsType::Over,
            "under" | "меньше" | "тм" | "tm" => OddsType::Under,
            _ => OddsType::Custom,
        }
    }

    fn sport_id_to_sport(id: u64) -> Sport {
        match id {
            1 | 3 => Sport::Football,
            6 => Sport::Tennis,
            9 => Sport::Hockey,
            12 | 129 => Sport::Basketball,
            15 => Sport::Volleyball,
            24 => Sport::Badminton,
            30 => Sport::Baseball,
            46 => Sport::WaterPolo,     // Water Polo
            49 => Sport::Handball,
            63 => Sport::Darts,         // Darts
            72 => Sport::Cricket,       // Cricket
            89 => Sport::TableTennis,   // Table Tennis
            107 => Sport::Rugby,        // Rugby League
            134 => Sport::Futsal,       // Futsal
            31118 => Sport::Tennis,     // Padel -> Tennis
            _ => Sport::Other,
        }
    }
}
