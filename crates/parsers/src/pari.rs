use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Pari API parser — handles the actual API structure
/// API returns: { events: [...], customFactors: [...], ... }
/// events: [{ id, team1, team2, sportId, startTime, ... }]
/// customFactors: [{ e: event_id, factors: [{ f: factor_id, v: odds, p: line*100, pt: "line" }] }]
#[derive(Debug)]
pub struct PariParser {
    client: Arc<Client>,
    live_url: String,
    prematch_url: String,
}

impl PariParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            live_url: "https://line-lb01-w.pb06e2-resources.com/events/list?lang=ru&scopeMarket=2300".to_string(),
            prematch_url: "https://line-lb01-w.pb06e2-resources.com/events/listBase?lang=ru&scopeMarket=2300".to_string(),
        }
    }

    /// Factor ID mappings for Pari
    /// 921=P1, 922=X, 923=P2 (1X2)
    /// 930=Total Over, 931=Total Under
    /// 910=Handicap, 912=Handicap
    /// 927, 928 = Other markets
    const FACTOR_1X2_HOME: u64 = 921;
    const FACTOR_1X2_DRAW: u64 = 922;
    const FACTOR_1X2_AWAY: u64 = 923;
    const FACTOR_TOTAL_OVER: u64 = 930;
    const FACTOR_TOTAL_UNDER: u64 = 931;
}

#[async_trait]
impl BookmakerParser for PariParser {
    fn name(&self) -> &str { "Pari" }
    fn slug(&self) -> &str { "pari" }
    fn is_enabled(&self) -> bool { true }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((events, _)) => all_events.extend(events),
                Err(e) => warn!(error = %e, "Pari fetch events failed"),
            }
        }
        info!(count = all_events.len(), "Pari events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(&self, _event_id: &str) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((_, odds)) => all_odds.extend(odds),
                Err(e) => warn!(error = %e, "Pari fetch odds failed"),
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
                Err(e) => warn!(error = %e, "Pari fetch failed"),
            }
        }

        let elapsed = start.elapsed().as_millis() as u64;
        debug!(events = all_events.len(), odds = all_odds.len(), time_ms = elapsed, "Pari fetch complete");
        Ok(ParserResult::new("pari", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str { "https://pari.ru" }
    fn user_agent(&self) -> &str { "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" }
}

impl PariParser {
    async fn fetch_api(&self, url: &str, is_live: bool) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        eprintln!("[PARI] Fetching {}", url);
        
        // Create a fresh client for each request
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        eprintln!("[PARI] Sending request...");
        let resp = client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .send()
            .await?;

        eprintln!("[PARI] Response: {}", resp.status());

        if !resp.status().is_success() {
            eprintln!("[PARI] HTTP failed");
            return Ok((Vec::new(), Vec::new()));
        }

        eprintln!("[PARI] Parsing JSON...");
        let json: serde_json::Value = resp.json().await?;
        eprintln!("[PARI] JSON parsed, extracting...");
        let result = Self::parse_api_response(&json, is_live);
        eprintln!("[PARI] Fetch complete");
        result
    }

    fn parse_api_response(json: &serde_json::Value, is_live: bool) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        // Parse events
        let events_data = match json.get("events").and_then(|e| e.as_array()) {
            Some(e) => e,
            None => return Ok((Vec::new(), Vec::new())),
        };

        // Build event lookup: id -> (sport_id, team1, team2, start_time)
        let mut event_map: HashMap<u64, (u64, String, String, Option<i64>)> = HashMap::new();
        for event_data in events_data {
            if let (Some(id), Some(team1), Some(team2)) = (
                event_data.get("id").and_then(|v| v.as_u64()),
                event_data.get("team1").and_then(|v| v.as_str()),
                event_data.get("team2").and_then(|v| v.as_str()),
            ) {
                let sport_id = event_data.get("sportId").and_then(|v| v.as_u64()).unwrap_or(0);
                let start_time = event_data.get("startTime").and_then(|v| v.as_i64());
                event_map.insert(id, (sport_id, team1.to_string(), team2.to_string(), start_time));
            }
        }

        // Parse customFactors into odds
        let mut all_odds = Vec::new();
        let mut seen_events = std::collections::HashSet::new();
        let now = Utc::now();

        if let Some(custom_factors) = json.get("customFactors").and_then(|f| f.as_array()) {
            for cf in custom_factors {
                if let (Some(event_id), Some(factors)) = (
                    cf.get("e").and_then(|v| v.as_u64()),
                    cf.get("factors").and_then(|f| f.as_array()),
                ) {
                    if let Some((sport_id, team1, team2, start_time)) = event_map.get(&event_id) {
                        // Create event if not seen
                        if seen_events.insert(event_id) {
                            let sport = Self::sport_id_to_sport(*sport_id);
                            let start_dt = start_time.map(|ts| chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default());
                            
                            all_odds.push((Event {
                                id: format!("pari-{}", event_id),
                                sport,
                                league: String::new(),
                                home_team: team1.clone(),
                                away_team: team2.clone(),
                                start_time: start_dt,
                                is_live,
                                bookmaker_slug: "pari".to_string(),
                                raw_url: None,
                                extra: HashMap::new(),
                            }, Vec::new()));
                        }

                        // Parse factors into odds
                        let event_idx = all_odds.iter().position(|(e, _)| e.id == format!("pari-{}", event_id));
                        if let Some(idx) = event_idx {
                            for factor in factors {
                                if let (Some(fid), Some(val)) = (
                                    factor.get("f").and_then(|v| v.as_u64()),
                                    factor.get("v").and_then(|v| v.as_f64()),
                                ) {
                                    if val <= 1.0 { continue; }
                                    
                                    let line = factor.get("p").and_then(|v| v.as_f64()).map(|p| p / 100.0);
                                    let (market, selection, odds_type) = Self::factor_to_market(fid);
                                    
                                    let (_, odds_vec) = &mut all_odds[idx];
                                    odds_vec.push(Odd {
                                        id: format!("pari-{}-{}", event_id, fid),
                                        event_id: format!("pari-{}", event_id),
                                        bookmaker_slug: "pari".to_string(),
                                        market,
                                        selection,
                                        odds: val,
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

        let events = all_odds.iter().map(|(e, _)| e.clone()).collect();
        let odds = all_odds.into_iter().flat_map(|(_, o)| o).collect();
        Ok((events, odds))
    }

    fn factor_to_market(fid: u64) -> (String, String, OddsType) {
        match fid {
            921 => ("1X2".into(), "1".into(), OddsType::Home),
            922 => ("1X2".into(), "X".into(), OddsType::Draw),
            923 => ("1X2".into(), "2".into(), OddsType::Away),
            930 => ("Total".into(), "Over".into(), OddsType::Over),
            931 => ("Total".into(), "Under".into(), OddsType::Under),
            910 | 912 => ("Handicap".into(), "1".into(), OddsType::Handicap),
            _ => (format!("factor_{}", fid), format!("{}", fid), OddsType::Custom),
        }
    }

    fn sport_id_to_sport(id: u64) -> Sport {
        // Common sport IDs for Pari
        match id {
            15870 | 15869 | 15871 => Sport::Football, // Football
            15872 | 15873 => Sport::Basketball,
            15874 | 15875 => Sport::Hockey,
            15876 | 15877 => Sport::Tennis,
            15878 | 15879 => Sport::Volleyball,
            _ => Sport::Football,
        }
    }
}
