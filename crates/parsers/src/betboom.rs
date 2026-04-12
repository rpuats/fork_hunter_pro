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

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// BetBoom HTTP-first scaffold.
/// Disabled by default until official endpoints and payload contracts are verified in production.
#[derive(Debug, Clone)]
pub struct BetboomParser {
    client: Arc<Client>,
    endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, Copy)]
struct Endpoint {
    url: &'static str,
    is_live: bool,
}

impl BetboomParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            endpoints: vec![
                Endpoint {
                    url: "https://betboom.ru/api/sport/events?mode=prematch",
                    is_live: false,
                },
                Endpoint {
                    url: "https://betboom.ru/api/sport/events?mode=live",
                    is_live: true,
                },
            ],
        }
    }

    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let mut events = Vec::new();
        let mut odds = Vec::new();

        for endpoint in &self.endpoints {
            match self.fetch_endpoint(*endpoint).await {
                Ok((endpoint_events, endpoint_odds)) => {
                    events.extend(endpoint_events);
                    odds.extend(endpoint_odds);
                }
                Err(error) => warn!(url = endpoint.url, %error, "BetBoom endpoint fetch failed"),
            }
        }

        Ok((events, odds))
    }

    async fn fetch_endpoint(
        &self,
        endpoint: Endpoint,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            url = endpoint.url,
            is_live = endpoint.is_live,
            "BetBoom: probing endpoint"
        );

        let response = self
            .client
            .get(endpoint.url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .header("Referer", "https://betboom.ru/")
            .timeout(Duration::from_secs(20))
            .send()
            .await?;

        if !response.status().is_success() {
            debug!(status = %response.status(), url = endpoint.url, "BetBoom: non-success status");
            return Ok((Vec::new(), Vec::new()));
        }

        let json: serde_json::Value = response.json().await?;
        Ok(Self::parse_response(&json, endpoint.is_live))
    }

    fn parse_response(json: &serde_json::Value, is_live: bool) -> (Vec<Event>, Vec<Odd>) {
        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        let Some(items) = json
            .get("events")
            .and_then(|value| value.as_array())
            .or_else(|| {
                json.get("data")
                    .and_then(|value| value.get("events"))
                    .and_then(|value| value.as_array())
            })
            .or_else(|| json.get("data").and_then(|value| value.as_array()))
            .or_else(|| json.as_array())
        else {
            debug!("BetBoom: response schema not recognized yet");
            return (events, odds);
        };

        for item in items {
            let Some((event_id, home, away)) = Self::extract_teams(item) else {
                continue;
            };

            let event_key = format!("betboom-{event_id}");
            let sport = Self::extract_sport(item);
            let league = Self::extract_league(item);

            events.push(Event {
                id: event_key.clone(),
                sport,
                league,
                home_team: home,
                away_team: away,
                start_time: None,
                is_live,
                bookmaker_slug: "betboom".to_string(),
                raw_url: None,
                extra: HashMap::new(),
            });

            if let Some(markets) = item.get("markets").or_else(|| item.get("bets")) {
                Self::append_markets(&mut odds, &event_key, markets, now);
            }
        }

        (events, odds)
    }

    fn extract_teams(item: &serde_json::Value) -> Option<(String, String, String)> {
        let event_id = item
            .get("id")
            .or_else(|| item.get("eventId"))
            .or_else(|| item.get("matchId"))?
            .to_string()
            .trim_matches('"')
            .to_string();

        if let Some(competitors) = item.get("competitors").and_then(|value| value.as_array()) {
            if competitors.len() >= 2 {
                let home = competitors
                    .first()?
                    .get("name")
                    .and_then(|value| value.as_str())?
                    .to_string();
                let away = competitors
                    .get(1)?
                    .get("name")
                    .and_then(|value| value.as_str())?
                    .to_string();
                return Some((event_id, home, away));
            }
        }

        let home = item
            .get("home")
            .or_else(|| item.get("homeTeam"))
            .and_then(|value| value.as_str())?
            .to_string();
        let away = item
            .get("away")
            .or_else(|| item.get("awayTeam"))
            .and_then(|value| value.as_str())?
            .to_string();

        Some((event_id, home, away))
    }

    fn extract_sport(item: &serde_json::Value) -> Sport {
        let raw = item
            .get("sport")
            .and_then(|value| value.as_str())
            .or_else(|| item.get("sportName").and_then(|value| value.as_str()))
            .unwrap_or("football");
        Sport::from_str(raw)
    }

    fn extract_league(item: &serde_json::Value) -> String {
        item.get("league")
            .and_then(|value| value.as_str())
            .or_else(|| item.get("tournament").and_then(|value| value.as_str()))
            .or_else(|| item.get("competition").and_then(|value| value.as_str()))
            .unwrap_or("Unknown")
            .to_string()
    }

    fn append_markets(
        odds: &mut Vec<Odd>,
        event_id: &str,
        markets: &serde_json::Value,
        now: chrono::DateTime<Utc>,
    ) {
        let Some(markets) = markets.as_array() else {
            return;
        };

        for market in markets {
            let market_name = market
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");

            let Some(outcomes) = market
                .get("outcomes")
                .and_then(|value| value.as_array())
                .or_else(|| market.get("selections").and_then(|value| value.as_array()))
                .or_else(|| market.get("values").and_then(|value| value.as_array()))
            else {
                continue;
            };

            for outcome in outcomes {
                let Some(selection) = outcome
                    .get("name")
                    .and_then(|value| value.as_str())
                    .or_else(|| outcome.get("selection").and_then(|value| value.as_str()))
                else {
                    continue;
                };

                let Some(price) = outcome
                    .get("price")
                    .and_then(|value| value.as_f64())
                    .or_else(|| outcome.get("odds").and_then(|value| value.as_f64()))
                    .or_else(|| outcome.get("value").and_then(|value| value.as_f64()))
                else {
                    continue;
                };

                if price <= 1.0 {
                    continue;
                }

                odds.push(Odd {
                    id: format!("{event_id}-{market_name}-{selection}"),
                    event_id: event_id.to_string(),
                    bookmaker_slug: "betboom".to_string(),
                    market: market_name.to_string(),
                    selection: selection.to_string(),
                    odds: price,
                    odds_type: Self::selection_to_odds_type(selection, market_name),
                    line: outcome.get("line").and_then(|value| value.as_f64()),
                    timestamp: now,
                });
            }
        }
    }

    fn selection_to_odds_type(selection: &str, market_name: &str) -> OddsType {
        let selection = selection.to_lowercase();
        let market_name = market_name.to_lowercase();

        if market_name.contains("тотал") || market_name.contains("total") {
            if selection.contains("over")
                || selection.contains("больше")
                || selection.contains("тб")
            {
                return OddsType::Over;
            }
            if selection.contains("under")
                || selection.contains("меньше")
                || selection.contains("тм")
            {
                return OddsType::Under;
            }
        }

        if market_name.contains("фора") || market_name.contains("handicap") {
            return OddsType::Handicap;
        }

        match selection.as_str() {
            "1" | "п1" | "home" => OddsType::Home,
            "x" | "ничья" | "draw" => OddsType::Draw,
            "2" | "п2" | "away" => OddsType::Away,
            _ => OddsType::Custom,
        }
    }
}

#[async_trait]
impl BookmakerParser for BetboomParser {
    fn name(&self) -> &str {
        "BetBoom"
    }
    fn slug(&self) -> &str {
        "betboom"
    }
    fn is_enabled(&self) -> bool {
        false
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let (events, _) = self.fetch_runtime_data().await?;
        info!(count = events.len(), "BetBoom events fetched");
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let (_, odds) = self.fetch_runtime_data().await?;
        info!(count = odds.len(), "BetBoom odds fetched");
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let started = std::time::Instant::now();
        let (events, odds) = self.fetch_runtime_data().await?;
        let elapsed = started.elapsed().as_millis() as u64;
        info!(
            events = events.len(),
            odds = odds.len(),
            time_ms = elapsed,
            "BetBoom fetch complete"
        );
        Ok(ParserResult::new("betboom", events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://betboom.ru"
    }
    fn user_agent(&self) -> &str {
        USER_AGENT
    }
}
