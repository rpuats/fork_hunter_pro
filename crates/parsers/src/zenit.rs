use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Zenit API parser — чистые HTTP запросы без Python/Playwright
///
/// API требует заголовки `imprinthash` и `frontversion`, захваченные из браузера.
/// Эндпоинты:
///   - Prematch: `https://zenit.win/ajax/line/printer/react`
///   - Live:     `https://zenit.win/ajax/live/printer/react`
///
/// Ответ: `{ games: { id: { c1_id, c2_id, tid, f_l: [{o, h}, ...] } }, dict: { cmd: { id: name }, tournament: { id: { name } } } }`
#[derive(Debug)]
pub struct ZenitParser {
    client: Arc<Client>,
}

impl ZenitParser {
    /// Заголовки, захваченные из реального браузера
    const IMPRINT_HASH: &str = "d01d68e5a9775b90a0c7239e7f078895";
    const FRONT_VERSION: &str = "1.72.1";

    // Sport IDs
    const SPORT_FOOTBALL: u64 = 1;
    const SPORT_HOCKEY: u64 = 2;
    const SPORT_BASKETBALL: u64 = 3;
    const SPORT_TENNIS: u64 = 5;

    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Fetch events from a single endpoint (line or live) for a specific sport
    async fn fetch_sport(
        &self,
        base_url: &str,
        sport_id: u64,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .client
            .get(base_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Referer", "https://zenit.win/line/football")
            .header("X-Requested-With", "XMLHttpRequest")
            .header("imprinthash", Self::IMPRINT_HASH)
            .header("frontversion", Self::FRONT_VERSION)
            .query(&[
                ("all", "1"),
                ("onlyview", "1"),
                ("timeline", "0"),
                ("tournaments_mode", "1"),
                ("sport", &sport_id.to_string()),
                ("tournament", ""),
                ("tournament_region", ""),
                ("tournament_info", ""),
                ("league", ""),
                ("games", ""),
                ("ross", "0"),
                ("lang_id", "1"),
                ("timezone", "3"),
                ("offset", "0"),
                ("show_from_main", "0"),
                ("client_v", ""),
                ("length", "1000"),
                ("sort_mode", "2"),
                ("b_id", ""),
                ("popular", "0"),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            debug!(status = %resp.status(), sport = sport_id, is_live, "Zenit API error");
            return Ok((Vec::new(), Vec::new()));
        }

        let json: serde_json::Value = resp.json().await?;

        // Check for application-level error
        if json.get("errorCode").is_some() {
            let msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("unknown");
            warn!(error = msg, "Zenit API application error");
            return Ok((Vec::new(), Vec::new()));
        }

        Ok(Self::parse_response(&json, sport_id, is_live))
    }

    /// Parse the JSON response from Zenit API
    fn parse_response(
        json: &serde_json::Value,
        sport_id: u64,
        is_live: bool,
    ) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut odds = Vec::new();
        let now = Utc::now();

        let games = match json.get("games").and_then(|v| v.as_object()) {
            Some(g) => g,
            None => return (Vec::new(), Vec::new()),
        };

        let dict = match json.get("dict").and_then(|v| v.as_object()) {
            Some(d) => d,
            None => return (Vec::new(), Vec::new()),
        };

        // Team names: dict.cmd[id] -> name
        let team_names: HashMap<String, String> = dict
            .get("cmd")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Tournament names: dict.tournament[id] -> { name: ... }
        let tournament_names: HashMap<String, String> = dict
            .get("tournament")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| {
                        v.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| (k.clone(), s.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let sport = Self::sport_id_to_sport(sport_id);
        let sports_without_draw: std::collections::HashSet<u64> = [3, 5].into_iter().collect();

        for (game_id, game) in games {
            let game_obj = match game.as_object() {
                Some(g) => g,
                None => continue,
            };

            let c1_id = game_obj
                .get("c1_id")
                .and_then(|v| v.as_u64())
                .map(|v| v.to_string());
            let c2_id = game_obj
                .get("c2_id")
                .and_then(|v| v.as_u64())
                .map(|v| v.to_string());

            let (home, away) = match (c1_id, c2_id) {
                (Some(id1), Some(id2)) => {
                    let h = team_names.get(&id1).cloned().unwrap_or_default();
                    let a = team_names.get(&id2).cloned().unwrap_or_default();
                    if h.is_empty() || a.is_empty() {
                        continue;
                    }
                    (h, a)
                }
                _ => continue,
            };

            // League
            let league = game_obj
                .get("tid")
                .and_then(|v| v.as_u64())
                .map(|tid| tid.to_string())
                .and_then(|tid| tournament_names.get(&tid).cloned())
                .unwrap_or_else(|| "Unknown".to_string());

            let event_id = format!("zenit-{}", game_id);

            // Parse odds from f_l or bets
            let odds_data = game_obj
                .get("f_l")
                .or_else(|| game_obj.get("bets"))
                .and_then(|v| v.as_array());

            let mut home_odds: Option<f64> = None;
            let mut draw_odds: Option<f64> = None;
            let mut away_odds: Option<f64> = None;

            if let Some(bets) = odds_data {
                for bet in bets {
                    let bet_obj = match bet.as_object() {
                        Some(b) => b,
                        None => continue,
                    };

                    let bet_option = bet_obj.get("o").and_then(|v| v.as_str());
                    let bet_value = bet_obj
                        .get("h")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .filter(|&v| v > 1.0);

                    match bet_option {
                        Some("1") => home_odds = bet_value.or(home_odds),
                        Some("2") => {
                            if sports_without_draw.contains(&sport_id) {
                                away_odds = bet_value.or(away_odds);
                            } else {
                                draw_odds = bet_value.or(draw_odds);
                            }
                        }
                        Some("3") => {
                            if sports_without_draw.contains(&sport_id) {
                                away_odds = bet_value.or(away_odds);
                            } else {
                                draw_odds = bet_value.or(draw_odds);
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Must have at least home and away odds
            if home_odds.is_none() || away_odds.is_none() {
                continue;
            }

            let event = Event {
                id: event_id.clone(),
                sport,
                league,
                home_team: home.clone(),
                away_team: away.clone(),
                start_time: None,
                is_live,
                bookmaker_slug: "zenit".to_string(),
                raw_url: Some("https://zenit.win".to_string()),
                extra: HashMap::new(),
            };
            events.push(event);

            // Push 1X2 odds
            if let Some(o1) = home_odds {
                odds.push(Odd {
                    id: format!("{}-1", event_id),
                    event_id: event_id.clone(),
                    bookmaker_slug: "zenit".to_string(),
                    market: "1X2".into(),
                    selection: "1".into(),
                    odds: o1,
                    odds_type: OddsType::Home,
                    line: None,
                    timestamp: now,
                });
            }
            if let Some(ox) = draw_odds {
                odds.push(Odd {
                    id: format!("{}-X", event_id),
                    event_id: event_id.clone(),
                    bookmaker_slug: "zenit".to_string(),
                    market: "1X2".into(),
                    selection: "X".into(),
                    odds: ox,
                    odds_type: OddsType::Draw,
                    line: None,
                    timestamp: now,
                });
            }
            if let Some(o2) = away_odds {
                odds.push(Odd {
                    id: format!("{}-2", event_id),
                    event_id: event_id.clone(),
                    bookmaker_slug: "zenit".to_string(),
                    market: "1X2".into(),
                    selection: "2".into(),
                    odds: o2,
                    odds_type: OddsType::Away,
                    line: None,
                    timestamp: now,
                });
            }
        }

        (events, odds)
    }

    fn sport_id_to_sport(id: u64) -> Sport {
        match id {
            1 => Sport::Football,
            2 => Sport::Hockey,
            3 => Sport::Basketball,
            5 => Sport::Tennis,
            _ => Sport::Football,
        }
    }
}

#[async_trait]
impl BookmakerParser for ZenitParser {
    fn name(&self) -> &str {
        "Zenit"
    }
    fn slug(&self) -> &str {
        "zenit"
    }
    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        let sport_ids = [
            Self::SPORT_FOOTBALL,
            Self::SPORT_HOCKEY,
            Self::SPORT_BASKETBALL,
            Self::SPORT_TENNIS,
        ];

        for sport_id in sport_ids {
            // Prematch
            match self
                .fetch_sport("https://zenit.win/ajax/line/printer/react", sport_id, false)
                .await
            {
                Ok((events, _)) => {
                    debug!(count = events.len(), sport = sport_id, "Zenit prematch");
                    all_events.extend(events);
                }
                Err(e) => warn!(error = %e, sport = sport_id, "Zenit prematch failed"),
            }

            // Live
            match self
                .fetch_sport("https://zenit.win/ajax/live/printer/react", sport_id, true)
                .await
            {
                Ok((events, _)) => {
                    debug!(count = events.len(), sport = sport_id, "Zenit live");
                    all_events.extend(events);
                }
                Err(e) => warn!(error = %e, sport = sport_id, "Zenit live failed"),
            }
        }

        info!(count = all_events.len(), "Zenit events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();
        let sport_ids = [
            Self::SPORT_FOOTBALL,
            Self::SPORT_HOCKEY,
            Self::SPORT_BASKETBALL,
            Self::SPORT_TENNIS,
        ];

        for sport_id in sport_ids {
            for url in [
                "https://zenit.win/ajax/line/printer/react",
                "https://zenit.win/ajax/live/printer/react",
            ] {
                if let Ok((_, odds)) = self.fetch_sport(url, sport_id, url.contains("live")).await {
                    all_odds.extend(odds);
                }
            }
        }

        Ok(all_odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        let sport_ids = [
            Self::SPORT_FOOTBALL,
            Self::SPORT_HOCKEY,
            Self::SPORT_BASKETBALL,
            Self::SPORT_TENNIS,
        ];

        for sport_id in sport_ids {
            // Prematch
            if let Ok((events, odds)) = self
                .fetch_sport("https://zenit.win/ajax/line/printer/react", sport_id, false)
                .await
            {
                all_events.extend(events);
                all_odds.extend(odds);
            }

            // Live
            if let Ok((events, odds)) = self
                .fetch_sport("https://zenit.win/ajax/live/printer/react", sport_id, true)
                .await
            {
                all_events.extend(events);
                all_odds.extend(odds);
            }
        }

        let elapsed = start.elapsed().as_millis() as u64;
        debug!(
            events = all_events.len(),
            odds = all_odds.len(),
            time_ms = elapsed,
            "Zenit fetch complete"
        );
        Ok(ParserResult::new("zenit", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://zenit.win"
    }
    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    }
}
