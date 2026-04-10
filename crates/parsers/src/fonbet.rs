use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Fonbet API parser — shared platform (same as Pari/Marathon/Bettery)
/// API: line-lb61-w.bk6bba-resources.com/events/list?scopeMarket=1600
#[derive(Debug)]
pub struct FonbetParser {
    client: Arc<Client>,
    live_url: String,
    prematch_url: String,
}

impl FonbetParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            live_url: "https://line-lb61-w.bk6bba-resources.com/ma/events/list?lang=ru&scopeMarket=1600".to_string(),
            prematch_url: "https://line-lb61-w.bk6bba-resources.com/ma/events/listBase?lang=ru&scopeMarket=1600".to_string(),
        }
    }

    /// Factor ID mappings for Fonbet (shared platform)
    /// 921=P1, 922=X, 923=P2 (1X2)
    const FACTOR_1X2_HOME: u64 = 921;
    const FACTOR_1X2_DRAW: u64 = 922;
    const FACTOR_1X2_AWAY: u64 = 923;
    const FACTOR_TOTAL_OVER: u64 = 930;
    const FACTOR_TOTAL_UNDER: u64 = 931;
}

#[async_trait]
impl BookmakerParser for FonbetParser {
    fn name(&self) -> &str { "Fonbet" }
    fn slug(&self) -> &str { "fonbet" }
    fn is_enabled(&self) -> bool { true }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((events, _)) => all_events.extend(events),
                Err(e) => warn!(error = %e, "Fonbet fetch events failed"),
            }
        }
        info!(count = all_events.len(), "Fonbet events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(&self, _event_id: &str) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();
        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_api(url, is_live).await {
                Ok((_, odds)) => all_odds.extend(odds),
                Err(e) => warn!(error = %e, "Fonbet fetch odds failed"),
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
                Err(e) => warn!(error = %e, "Fonbet fetch failed"),
            }
        }

        let elapsed = start.elapsed().as_millis() as u64;
        info!(events = all_events.len(), odds = all_odds.len(), time_ms = elapsed, "Fonbet fetch complete");
        Ok(ParserResult::new("fonbet", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str { "https://fonbet.ru" }
    fn user_agent(&self) -> &str { "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" }
}

impl FonbetParser {
    async fn fetch_api(&self, url: &str, is_live: bool) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        debug!(url = url, "Fonbet: fetching");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let resp = client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .send()
            .await?;

        if !resp.status().is_success() {
            debug!(status = %resp.status(), "Fonbet: API failed");
            return Ok((Vec::new(), Vec::new()));
        }

        let json: serde_json::Value = resp.json().await?;
        Self::parse_api_response(&json, is_live)
    }

    fn parse_api_response(json: &serde_json::Value, is_live: bool) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let events_data = match json.get("events").and_then(|e| e.as_array()) {
            Some(e) => e,
            None => return Ok((Vec::new(), Vec::new())),
        };

        // Build event lookup: id -> (sport_id, team1, team2, start_time, league)
        let mut event_map: HashMap<u64, (u64, String, String, Option<i64>, String)> = HashMap::new();
        for event_data in events_data {
            if let (Some(id), Some(team1), Some(team2)) = (
                event_data.get("id").and_then(|v| v.as_u64()),
                event_data.get("team1").and_then(|v| v.as_str()),
                event_data.get("team2").and_then(|v| v.as_str()),
            ) {
                let sport_id = event_data.get("sportId").and_then(|v| v.as_u64()).unwrap_or(0);
                let start_time = event_data.get("startTime").and_then(|v| v.as_i64());
                let league = event_data.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                event_map.insert(id, (sport_id, team1.to_string(), team2.to_string(), start_time, league));
            }
        }

        let mut all_odds = Vec::new();
        let mut seen_events = std::collections::HashSet::new();
        let now = Utc::now();

        if let Some(custom_factors) = json.get("customFactors").and_then(|f| f.as_array()) {
            for cf in custom_factors {
                if let (Some(event_id), Some(factors)) = (
                    cf.get("e").and_then(|v| v.as_u64()),
                    cf.get("factors").and_then(|f| f.as_array()),
                ) {
                    if let Some((sport_id, team1, team2, start_time, league)) = event_map.get(&event_id) {
                        if seen_events.insert(event_id) {
                            let sport = Self::sport_id_to_sport(*sport_id);
                            let start_dt = start_time.map(|ts| chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default());

                            all_odds.push((Event {
                                id: format!("fonbet-{}", event_id),
                                sport,
                                league: league.clone(),
                                home_team: team1.clone(),
                                away_team: team2.clone(),
                                start_time: start_dt,
                                is_live,
                                bookmaker_slug: "fonbet".to_string(),
                                raw_url: None,
                                extra: HashMap::new(),
                            }, Vec::new()));
                        }

                        let event_idx = all_odds.iter().position(|(e, _)| e.id == format!("fonbet-{}", event_id));
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
                                        id: format!("fonbet-{}-{}", event_id, fid),
                                        event_id: format!("fonbet-{}", event_id),
                                        bookmaker_slug: "fonbet".to_string(),
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
            // 1X2
            921 => ("1X2".into(), "1".into(), OddsType::Home),
            922 => ("1X2".into(), "X".into(), OddsType::Draw),
            923 => ("1X2".into(), "2".into(), OddsType::Away),
            // Totals
            930 => ("Total".into(), "Over".into(), OddsType::Over),
            931 => ("Total".into(), "Under".into(), OddsType::Under),
            924 | 1002 | 1010 | 1054 => ("Total".into(), "Over".into(), OddsType::Over),
            925 | 1003 | 1011 | 1055 => ("Total".into(), "Under".into(), OddsType::Under),
            // Handicaps
            910 | 912 | 1004 | 1006 | 1012 => ("Handicap".into(), "1".into(), OddsType::Handicap),
            1005 | 1013 => ("Handicap".into(), "2".into(), OddsType::Handicap),
            // BTTS (Both Teams Score)
            926 => ("BTTS".into(), "Yes".into(), OddsType::Custom),
            927 => ("BTTS".into(), "No".into(), OddsType::Custom),
            // Even/Odd
            928 => ("EvenOdd".into(), "Even".into(), OddsType::Custom),
            929 => ("EvenOdd".into(), "Odd".into(), OddsType::Custom),
            // Double Chance
            1014 => ("DoubleChance".into(), "1X".into(), OddsType::Custom),
            1015 => ("DoubleChance".into(), "12".into(), OddsType::Custom),
            1016 => ("DoubleChance".into(), "X2".into(), OddsType::Custom),
            // Individual Totals (ИТБ/ИТМ)
            1020 | 1022 | 1024 => ("IndividualTotal".into(), "Over".into(), OddsType::Over),
            1021 | 1023 | 1025 => ("IndividualTotal".into(), "Under".into(), OddsType::Under),
            // 1H/2H Results
            1030 => ("1H_Result".into(), "1".into(), OddsType::Home),
            1031 => ("1H_Result".into(), "X".into(), OddsType::Draw),
            1032 => ("1H_Result".into(), "2".into(), OddsType::Away),
            1033 => ("2H_Result".into(), "1".into(), OddsType::Home),
            1034 => ("2H_Result".into(), "X".into(), OddsType::Draw),
            1035 => ("2H_Result".into(), "2".into(), OddsType::Away),
            // Correct Score (partial)
            1040..=1050 => ("CorrectScore".into(), format!("score_{}", fid), OddsType::Custom),
            // Fallback
            _ => (format!("factor_{}", fid), format!("{}", fid), OddsType::Custom),
        }
    }

    fn sport_id_to_sport(id: u64) -> Sport {
        match id {
            1 | 4 | 15870 | 15869 | 15871 => Sport::Football,
            2 | 5 | 15872 | 15873 => Sport::Basketball,
            3 | 6 | 15874 | 15875 => Sport::Hockey,
            7 | 15876 | 15877 => Sport::Tennis,
            8 | 15878 | 15879 => Sport::Volleyball,
            _ => Sport::Football,
        }
    }
}
