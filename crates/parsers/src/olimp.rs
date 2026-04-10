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

/// Olimp API parser v4
/// API: olimp.bet/api/v4/{sport_id}/live|line/sports-with-competitions-with-events
#[derive(Debug)]
pub struct OlimpParser {
    client: Arc<Client>,
    base_api_url: String,
}

impl OlimpParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            base_api_url: "https://www.olimp.bet/api/v4".to_string(),
        }
    }
}

#[async_trait]
impl BookmakerParser for OlimpParser {
    fn name(&self) -> &str { "Olimp" }
    fn slug(&self) -> &str { "olimp" }
    fn is_enabled(&self) -> bool { true }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        for (section, is_live) in [("live", true), ("line/top", false)] {
            match self.fetch_section(section, is_live).await {
                Ok((events, _)) => all_events.extend(events),
                Err(e) => warn!(error = %e, "Olimp fetch events failed"),
            }
        }
        info!(count = all_events.len(), "Olimp events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(&self, _event_id: &str) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();
        for (section, is_live) in [("live", true), ("line/top", false)] {
            match self.fetch_section(section, is_live).await {
                Ok((_, odds)) => all_odds.extend(odds),
                Err(e) => warn!(error = %e, "Olimp fetch odds failed"),
            }
        }
        Ok(all_odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        // Fetch live and prematch in parallel
        let live_fut = self.fetch_section("live", true);
        let prematch_fut = self.fetch_section("line/top", false);
        let (live_res, prematch_res) = tokio::join!(live_fut, prematch_fut);

        if let Ok((events, odds)) = live_res {
            all_events.extend(events);
            all_odds.extend(odds);
        }
        if let Ok((events, odds)) = prematch_res {
            all_events.extend(events);
            all_odds.extend(odds);
        }

        let elapsed = start.elapsed().as_millis() as u64;
        info!(events = all_events.len(), odds = all_odds.len(), time_ms = elapsed, "Olimp fetch complete");
        Ok(ParserResult::new("olimp", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str { "https://www.olimp.bet" }
    fn user_agent(&self) -> &str { "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" }
}

impl OlimpParser {
    async fn fetch_section(&self, section: &str, is_live: bool) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        // CRITICAL: Olimp uses /v4/0/ which returns ALL sports at once
        let url = format!("{}/v4/0/{}/sports-with-competitions-with-events?vids%5B%5D=", self.base_api_url, section);

        debug!(url = url, "Olimp: fetching section");

        let resp = self.client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .header("Referer", "https://www.olimp.bet/")
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        if !resp.status().is_success() {
            debug!(status = %resp.status(), "Olimp: API failed");
            return Ok((Vec::new(), Vec::new()));
        }

        let text = resp.text().await?;
        debug!(response_length = text.len(), "Olimp: received data");

        let json: serde_json::Value = match serde_json::from_str(&text) {
            Ok(j) => j,
            Err(e) => {
                debug!(error = %e, "Olimp: JSON parse failed");
                return Ok((Vec::new(), Vec::new()));
            }
        };

        // Parse ALL sports from this single response
        Self::parse_api_response_all_sports(&json, is_live)
    }

    fn parse_api_response_all_sports(
        json: &serde_json::Value,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let mut events = Vec::new();
        let mut all_odds = Vec::new();
        let now = Utc::now();

        // API returns array of sport objects: [{ operationId, payload: { sport: {...}, competitionsWithEvents: [...] } }]
        let sports_array = match json.as_array() {
            Some(a) => a,
            None => return Ok((Vec::new(), Vec::new())),
        };

        for sport_obj in sports_array {
            let payload = match sport_obj.get("payload") {
                Some(p) => p,
                None => continue,
            };

            let sport_info = payload.get("sport").unwrap_or(&serde_json::Value::Null);
            let sport_id = sport_info.get("id").and_then(|v| v.as_str()).unwrap_or("0");
            let sport_name = sport_info.get("name").or_else(|| sport_info.get("names").and_then(|n| n.get("0")))
                .and_then(|v| v.as_str()).unwrap_or("");

            let sport = match sport_id {
                "1" | "3" => Sport::Football,
                "2" => Sport::Basketball,
                "4" => Sport::Hockey,
                "5" => Sport::Tennis,
                "6" => Sport::Volleyball,
                "7" => Sport::TableTennis,
                "8" => Sport::Baseball,
                "9" => Sport::Handball,
                "10" => Sport::Badminton,
                "11" => Sport::WaterPolo,
                "12" => Sport::Cricket,
                "13" => Sport::Darts,
                _ => Sport::Other,
            };

            let competitions = match payload.get("competitionsWithEvents").and_then(|c| c.as_array()) {
                Some(c) => c,
                None => continue,
            };

            for comp in competitions {
                let league_name = comp.get("name").and_then(|v| v.as_str()).unwrap_or("");

                if let Some(events_array) = comp.get("events").and_then(|e| e.as_array()) {
                    for event_data in events_array {
                        let (event_id, home, away) = match Self::extract_event_info(event_data) {
                            Some(info) => info,
                            None => continue,
                        };

                        let event_key = format!("olimp-{}", event_id);

                        let event = Event {
                            id: event_key.clone(),
                            sport,
                            league: if !league_name.is_empty() { league_name.to_string() } else { sport_name.to_string() },
                            home_team: home.clone(),
                            away_team: away.clone(),
                            start_time: None,
                            is_live,
                            bookmaker_slug: "olimp".to_string(),
                            raw_url: None,
                            extra: HashMap::new(),
                        };
                        events.push(event);

                        // Parse outcomes into odds
                        if let Some(outcomes) = event_data.get("outcomes").and_then(|o| o.as_array()) {
                            for outcome in outcomes {
                                if let (Some(selection), Some(prob_str)) = (
                                    outcome.get("shortName").and_then(|v| v.as_str()),
                                    outcome.get("probability").and_then(|v| v.as_str()),
                                ) {
                                    if let Ok(prob) = prob_str.parse::<f64>() {
                                        if prob > 1.0 {
                                            let market = outcome.get("groupName")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown");
                                            let odds_type = Self::selection_to_odds_type(selection);
                                            let line = outcome.get("param").and_then(|v| v.as_f64());

                                            all_odds.push(Odd {
                                                id: format!("{}-{}-{}", event_key, market, selection),
                                                event_id: event_key.clone(),
                                                bookmaker_slug: "olimp".to_string(),
                                                market: market.to_string(),
                                                selection: selection.to_string(),
                                                odds: prob,
                                                odds_type,
                                                line,
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

        debug!(sports = sports_array.len(), events = events.len(), odds = all_odds.len(), "Olimp: parsed");
        Ok((events, all_odds))
    }

    fn extract_event_info(data: &serde_json::Value) -> Option<(String, String, String)> {
        let event_id = data.get("id")?.to_string().trim_matches('"').to_string();

        let home = data.get("team1Name").or_else(|| data.get("competitor1"))
            .and_then(|v| v.as_str())?
            .to_string();

        let away = data.get("team2Name").or_else(|| data.get("competitor2"))
            .and_then(|v| v.as_str())?
            .to_string();

        Some((event_id, home, away))
    }

    fn selection_to_odds_type(selection: &str) -> OddsType {
        match selection {
            "П1" | "1" | "home" => OddsType::Home,
            "Х" | "X" | "draw" | "ничья" => OddsType::Draw,
            "П2" | "2" | "away" => OddsType::Away,
            "ТБ" | "TB" | "over" | "больше" => OddsType::Over,
            "ТМ" | "TM" | "under" | "меньше" => OddsType::Under,
            "Ф1" | "Ф2" | "handicap" => OddsType::Handicap,
            _ => OddsType::Custom,
        }
    }
}
