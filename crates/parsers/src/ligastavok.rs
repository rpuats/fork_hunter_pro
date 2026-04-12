use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use rand::Rng;
use reqwest::Client;
use serde::Serialize;
use shared::odds::OddsType;
use shared::{DiagnosticSeverity, ParserDiagnosticCheck, ParserReadiness, ParserReadinessStage};
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const EVENTS_LIST_URL: &str = "https://lds-api-sites.ligastavok.ru/rest/events/v8/eventsList";
const BASE_URL: &str = "https://www.ligastavok.ru";
const BOOKMAKER_SLUG: &str = "ligastavok";

/// Liga Stavok HTTP-first scaffold.
/// Uses the discovered POST `eventsList` flow, but remains disabled by default
/// until QRATOR/session bootstrap is stable enough for production traffic.
#[derive(Debug, Clone)]
pub struct LigaStavokParser {
    client: Arc<Client>,
    endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, Copy)]
struct Endpoint {
    referer: &'static str,
    route_hint: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventsListPayload {
    game_id: Vec<u32>,
    limit: u32,
    skip: u32,
    top_events: bool,
    ts: i64,
    view: &'static str,
    widget_video: bool,
    proposed_types: Vec<&'static str>,
}

impl LigaStavokParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            endpoints: vec![
                Endpoint {
                    referer: "https://www.ligastavok.ru/line/football",
                    route_hint: "line",
                },
                Endpoint {
                    referer: "https://www.ligastavok.ru/live/football",
                    route_hint: "live",
                },
            ],
        }
    }

    fn build_payload(&self) -> EventsListPayload {
        EventsListPayload {
            game_id: Vec::new(),
            limit: 200,
            skip: 0,
            top_events: false,
            ts: Utc::now().timestamp_millis(),
            view: "priority",
            widget_video: false,
            proposed_types: vec!["MAINOFFER"],
        }
    }

    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let mut events_by_id = HashMap::new();
        let mut odds_by_id = HashMap::new();

        for endpoint in &self.endpoints {
            match self.fetch_endpoint(*endpoint).await {
                Ok((endpoint_events, endpoint_odds)) => {
                    for event in endpoint_events {
                        events_by_id.entry(event.id.clone()).or_insert(event);
                    }

                    for odd in endpoint_odds {
                        odds_by_id.entry(odd.id.clone()).or_insert(odd);
                    }
                }
                Err(error) => {
                    warn!(referer = endpoint.referer, route_hint = endpoint.route_hint, %error, "Liga Stavok endpoint fetch failed");
                }
            }
        }

        Ok((
            events_by_id.into_values().collect(),
            odds_by_id.into_values().collect(),
        ))
    }

    async fn fetch_endpoint(
        &self,
        endpoint: Endpoint,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let req_id = Self::request_id();
        let payload = self.build_payload();

        debug!(
            url = EVENTS_LIST_URL,
            referer = endpoint.referer,
            route_hint = endpoint.route_hint,
            request_id = req_id,
            "Liga Stavok: probing eventsList endpoint"
        );

        let response = self
            .client
            .post(EVENTS_LIST_URL)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .header("Content-Type", "application/json")
            .header("Referer", endpoint.referer)
            .header("x-application-name", "mobile")
            .header("x-req-id", req_id)
            .timeout(Duration::from_secs(20))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            debug!(status = %response.status(), referer = endpoint.referer, route_hint = endpoint.route_hint, "Liga Stavok: non-success status from eventsList");
            return Ok((Vec::new(), Vec::new()));
        }

        let json: serde_json::Value = response.json().await?;
        Ok(Self::parse_response(&json, endpoint.route_hint))
    }

    fn request_id() -> String {
        format!("ls-{:016x}", rand::thread_rng().gen::<u64>())
    }

    fn readiness_snapshot() -> ParserReadiness {
        ParserReadiness {
            stage: ParserReadinessStage::RolloutReady,
            production_enabled: false,
            self_check_available: true,
            checks: vec![
                ParserDiagnosticCheck {
                    code: "events_list_endpoint_configured".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!("POST probe configured for {EVENTS_LIST_URL}."),
                },
                ParserDiagnosticCheck {
                    code: "route_probes_seeded".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Prematch and live referer probes are configured for readiness diagnostics.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "schema_parser_present".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "eventsList payload parser extracts events, markets, and totals from the known scaffold shape.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "session_bootstrap_pending".to_string(),
                    severity: DiagnosticSeverity::Warn,
                    message: "QRATOR/session bootstrap is not stable enough for production traffic yet, so the parser remains disabled by default.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "production_guardrail".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Factory registration is kept for diagnostics and explicit testing only; scanner enablement stays off.".to_string(),
                },
            ],
        }
    }

    fn parse_response(json: &serde_json::Value, route_hint: &str) -> (Vec<Event>, Vec<Odd>) {
        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        let Some(items) = json
            .get("result")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.as_array())
            .or_else(|| json.get("data").and_then(|value| value.as_array()))
        else {
            debug!(
                route_hint,
                "Liga Stavok: response schema not recognized yet"
            );
            return (events, odds);
        };

        for item in items {
            let Some((event_id, home, away)) = Self::extract_teams(item) else {
                continue;
            };

            let event_key = format!("{BOOKMAKER_SLUG}-{event_id}");
            let is_live = Self::extract_is_live(item, route_hint);

            let mut extra = HashMap::new();
            if let Some(ns) = item.get("ns").and_then(|value| value.as_str()) {
                extra.insert(
                    "namespace".to_string(),
                    serde_json::Value::String(ns.to_string()),
                );
            }
            if let Some(ext_id) = item
                .get("ids")
                .and_then(|value| value.get("extId"))
                .cloned()
            {
                extra.insert("ext_id".to_string(), ext_id);
            }

            events.push(Event {
                id: event_key.clone(),
                sport: Self::extract_sport(item),
                league: Self::extract_league(item),
                home_team: home,
                away_team: away,
                start_time: Self::extract_start_time(item),
                is_live,
                bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                raw_url: Self::extract_raw_url(item, is_live),
                extra,
            });

            Self::append_markets(&mut odds, &event_key, item, now);
        }

        (events, odds)
    }

    fn extract_teams(item: &serde_json::Value) -> Option<(String, String, String)> {
        let event_id = item.get("id")?.to_string().trim_matches('"').to_string();
        let event = item.get("event")?;

        if let Some(competitors) = event.get("competitors").and_then(|value| value.as_array()) {
            if competitors.len() >= 2 {
                let home = competitors
                    .first()?
                    .get("name")
                    .and_then(|value| value.as_str())?
                    .trim()
                    .to_string();
                let away = competitors
                    .get(1)?
                    .get("name")
                    .and_then(|value| value.as_str())?
                    .trim()
                    .to_string();
                if !home.is_empty() && !away.is_empty() {
                    return Some((event_id, home, away));
                }
            }
        }

        let home = event
            .get("team1")
            .and_then(|value| value.as_str())?
            .trim()
            .to_string();
        let away = event
            .get("team2")
            .and_then(|value| value.as_str())?
            .trim()
            .to_string();

        if home.is_empty() || away.is_empty() {
            return None;
        }

        Some((event_id, home, away))
    }

    fn extract_league(item: &serde_json::Value) -> String {
        let event = item.get("event");
        let tournament = event
            .and_then(|value| value.get("tournamentTitle"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        let category = event
            .and_then(|value| value.get("categoryTitle"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        let topic = event
            .and_then(|value| value.get("topicTitle"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();

        if !category.is_empty() && !tournament.is_empty() && category != tournament {
            return format!("{category}. {tournament}");
        }
        if !topic.is_empty() {
            return topic.to_string();
        }
        if !tournament.is_empty() {
            return tournament.to_string();
        }
        if !category.is_empty() {
            return category.to_string();
        }

        "Unknown".to_string()
    }

    fn extract_sport(item: &serde_json::Value) -> Sport {
        let raw = item
            .get("gameTitle")
            .and_then(|value| value.as_str())
            .or_else(|| item.get("title").and_then(|value| value.as_str()))
            .or_else(|| item.get("gameName").and_then(|value| value.as_str()))
            .unwrap_or("football");
        Sport::from_str(raw)
    }

    fn extract_is_live(item: &serde_json::Value, route_hint: &str) -> bool {
        match item.get("ns").and_then(|value| value.as_str()) {
            Some("live") => true,
            Some("line") | Some("prematch") => false,
            Some(other) => other.contains("live"),
            None => route_hint == "live",
        }
    }

    fn extract_start_time(item: &serde_json::Value) -> Option<DateTime<Utc>> {
        item.get("gameTs")
            .and_then(|value| value.as_i64())
            .and_then(|timestamp| Utc.timestamp_millis_opt(timestamp).single())
    }

    fn extract_raw_url(item: &serde_json::Value, is_live: bool) -> Option<String> {
        let game = item
            .get("gameSeoName")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())?;
        let section = if is_live { "live" } else { "line" };
        Some(format!("{BASE_URL}/{section}/{game}"))
    }

    fn append_markets(
        odds: &mut Vec<Odd>,
        event_id: &str,
        item: &serde_json::Value,
        now: chrono::DateTime<Utc>,
    ) {
        let Some(markets) = item.get("markets").and_then(|value| value.as_object()) else {
            return;
        };
        let Some(outcomes) = item.get("outcomes").and_then(|value| value.as_object()) else {
            return;
        };

        let parts = item.get("parts").and_then(|value| value.as_object());
        let mut outcomes_by_market: HashMap<String, Vec<&serde_json::Value>> = HashMap::new();

        for outcome in outcomes.values() {
            let Some(market_id) = outcome.get("marketId").and_then(|value| value.as_str()) else {
                continue;
            };
            outcomes_by_market
                .entry(market_id.to_string())
                .or_default()
                .push(outcome);
        }

        for (market_key, market_value) in markets {
            let Some(market) = market_value.as_object() else {
                continue;
            };
            if Self::is_locked_or_corrupted(market_value) {
                continue;
            }

            let Some(market_name) = market.get("title").and_then(|value| value.as_str()) else {
                continue;
            };
            let market_type = market
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let market_id = market
                .get("id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default();
            let market_label = Self::format_market_name(
                market_name,
                market.get("partId").and_then(|value| value.as_str()),
                parts,
            );

            let Some(linked_outcomes) = outcomes_by_market.get(market_key) else {
                continue;
            };

            for outcome in linked_outcomes {
                if Self::is_locked_or_corrupted(outcome) {
                    continue;
                }

                let Some(price) = outcome.get("value").and_then(Self::parse_f64) else {
                    continue;
                };
                if price <= 1.0 {
                    continue;
                }

                let selection = Self::format_selection(outcome);
                let line = outcome
                    .get("adValue")
                    .and_then(Self::parse_f64)
                    .or_else(|| outcome.get("line").and_then(Self::parse_f64));
                let outcome_id = outcome
                    .get("id")
                    .and_then(|value| value.as_i64())
                    .unwrap_or_default();

                odds.push(Odd {
                    id: format!("{event_id}-{market_id}-{outcome_id}"),
                    event_id: event_id.to_string(),
                    bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                    market: market_label.clone(),
                    selection: selection.clone(),
                    odds: price,
                    odds_type: Self::selection_to_odds_type(&selection, market_name, market_type),
                    line,
                    timestamp: now,
                });
            }
        }
    }

    fn format_market_name(
        market_name: &str,
        part_id: Option<&str>,
        parts: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> String {
        let part_title = part_id
            .and_then(|id| parts.and_then(|value| value.get(id)))
            .and_then(|value| value.get("title"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();

        if part_title.is_empty() || part_title == "Основное время" {
            return market_name.to_string();
        }

        format!("{part_title} / {market_name}")
    }

    fn format_selection(outcome: &serde_json::Value) -> String {
        let ad_title = outcome
            .get("adTitle")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        if !ad_title.is_empty() {
            return ad_title.to_string();
        }

        outcome
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .trim()
            .to_string()
    }

    fn parse_f64(value: &serde_json::Value) -> Option<f64> {
        match value {
            serde_json::Value::Number(number) => number.as_f64(),
            serde_json::Value::String(text) => {
                let normalized = text.trim().replace(',', ".");
                normalized.parse::<f64>().ok()
            }
            _ => None,
        }
    }

    fn is_locked_or_corrupted(value: &serde_json::Value) -> bool {
        value
            .get("locked")
            .and_then(|field| field.as_bool())
            .unwrap_or(false)
            || value
                .get("corrupted")
                .and_then(|field| field.as_bool())
                .unwrap_or(false)
    }

    fn selection_to_odds_type(selection: &str, market_name: &str, market_type: &str) -> OddsType {
        let selection = selection.to_lowercase();
        let market_name = market_name.to_lowercase();
        let market_type = market_type.to_lowercase();

        if market_type == "ttl" || market_name.contains("тотал") || market_name.contains("total")
        {
            if selection.contains("бол")
                || selection.contains("больше")
                || selection.contains("over")
                || selection.contains("tb")
            {
                return OddsType::Over;
            }
            if selection.contains("мен")
                || selection.contains("меньше")
                || selection.contains("under")
                || selection.contains("tm")
                || selection.contains("less")
            {
                return OddsType::Under;
            }
        }

        if market_type == "han" || market_name.contains("фора") || market_name.contains("handicap")
        {
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
impl BookmakerParser for LigaStavokParser {
    fn name(&self) -> &str {
        "Liga Stavok"
    }

    fn slug(&self) -> &str {
        BOOKMAKER_SLUG
    }

    fn is_enabled(&self) -> bool {
        false
    }

    fn readiness(&self) -> Option<ParserReadiness> {
        Some(Self::readiness_snapshot())
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let (events, _) = self.fetch_runtime_data().await?;
        info!(count = events.len(), "Liga Stavok events fetched");
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let (_, odds) = self.fetch_runtime_data().await?;
        info!(count = odds.len(), "Liga Stavok odds fetched");
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
            "Liga Stavok fetch complete"
        );
        Ok(ParserResult::new(BOOKMAKER_SLUG, events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        BASE_URL
    }

    fn user_agent(&self) -> &str {
        USER_AGENT
    }
}

#[cfg(test)]
mod tests {
    use super::LigaStavokParser;

    #[test]
    fn parses_events_list_shape() {
        let payload = serde_json::json!({
            "result": {
                "data": [
                    {
                        "id": 22957768,
                        "gameTitle": "Футбол",
                        "gameSeoName": "football",
                        "gameTs": 1775365200000_i64,
                        "ns": "live",
                        "ids": { "extId": 685893 },
                        "event": {
                            "team1": "СКА Хабаровск",
                            "team2": "Волга Ульяновск",
                            "categoryTitle": "Россия",
                            "tournamentTitle": "Первая лига",
                            "competitors": [
                                { "name": "ФК СКА-Хабаровск" },
                                { "name": "Волга Ульяновск" }
                            ]
                        },
                        "parts": {
                            "main": { "title": "Основное время" }
                        },
                        "markets": {
                            "_759248971": {
                                "id": 759248971,
                                "title": "Победитель",
                                "type": "WIN",
                                "partId": "main",
                                "locked": false,
                                "corrupted": false
                            },
                            "_759248982": {
                                "id": 759248982,
                                "title": "Тотал",
                                "type": "TTL",
                                "partId": "main",
                                "locked": false,
                                "corrupted": false
                            }
                        },
                        "outcomes": {
                            "_1": {
                                "id": 1,
                                "marketId": "_759248971",
                                "title": "1",
                                "value": 3.6,
                                "locked": false,
                                "corrupted": false
                            },
                            "_x": {
                                "id": 2,
                                "marketId": "_759248971",
                                "title": "X",
                                "value": 1.57,
                                "locked": false,
                                "corrupted": false
                            },
                            "_2": {
                                "id": 3,
                                "marketId": "_759248971",
                                "title": "2",
                                "value": 5.5,
                                "locked": false,
                                "corrupted": false
                            },
                            "_over": {
                                "id": 4,
                                "marketId": "_759248982",
                                "title": "Бол",
                                "adValue": "2.50",
                                "value": 2.04,
                                "locked": false,
                                "corrupted": false
                            },
                            "_under": {
                                "id": 5,
                                "marketId": "_759248982",
                                "title": "Мен",
                                "adValue": "2.50",
                                "value": 1.66,
                                "locked": false,
                                "corrupted": false
                            }
                        }
                    }
                ]
            }
        });

        let (events, odds) = LigaStavokParser::parse_response(&payload, "live");

        assert_eq!(events.len(), 1);
        assert_eq!(odds.len(), 5);
        assert_eq!(events[0].league, "Россия. Первая лига");
        assert!(events[0].is_live);
        assert!(odds
            .iter()
            .any(|odd| odd.market == "Победитель" && odd.selection == "1"));
        assert!(odds
            .iter()
            .any(|odd| odd.market == "Тотал" && odd.line == Some(2.5)));
    }

    #[test]
    fn exposes_rollout_readiness_diagnostics() {
        let readiness = LigaStavokParser::readiness_snapshot();

        assert_eq!(readiness.stage, shared::ParserReadinessStage::RolloutReady);
        assert!(!readiness.production_enabled);
        assert!(readiness.self_check_available);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "session_bootstrap_pending"));
    }
}
