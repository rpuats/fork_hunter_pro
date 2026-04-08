use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Winline parser - uses Python Playwright parser via subprocess
/// This ensures we get real data from the working Python parser
#[derive(Debug, Clone)]
pub struct WinlineParser {
    #[allow(dead_code)]
    client: Arc<reqwest::Client>,
}

#[derive(Debug, Deserialize)]
struct PyEvent {
    home_team: Option<String>,
    away_team: Option<String>,
    league: Option<String>,
    is_live: Option<bool>,
    home_odds: Option<f64>,
    draw_odds: Option<f64>,
    away_odds: Option<f64>,
}

impl WinlineParser {
    pub fn new(client: Arc<reqwest::Client>) -> Self {
        Self { client }
    }

    /// Call Python Playwright parser via subprocess
    fn call_python_parser(&self) -> Vec<PyEvent> {
        let script_path = "scanner/parsers/parse_winline_json.py";
        
        debug!(script = script_path, "Calling Python Winline parser");
        
        let output = std::process::Command::new("python")
            .arg(script_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        
        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(events) = serde_json::from_str::<Vec<PyEvent>>(&stdout) {
                    debug!(count = events.len(), "Python Winline parser returned events");
                    events
                } else {
                    warn!("Failed to parse Python output: {}", &stdout[..stdout.len().min(200)]);
                    Vec::new()
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Python parser failed: {}", &stderr[..stderr.len().min(200)]);
                Vec::new()
            }
            Err(e) => {
                warn!("Failed to run Python parser: {}", e);
                Vec::new()
            }
        }
    }

    fn convert_events(&self, py_events: &[PyEvent]) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut all_odds = Vec::new();
        let now = Utc::now();

        for py_event in py_events {
            if let (Some(home), Some(away)) = (&py_event.home_team, &py_event.away_team) {
                if home.is_empty() || away.is_empty() {
                    continue;
                }

                let event_id = format!("winline-{}-{}", home.replace(' ', "_"), away.replace(' ', "_"));
                let is_live = py_event.is_live.unwrap_or(false);

                let event = Event {
                    id: event_id.clone(),
                    sport: Sport::Football,
                    league: py_event.league.clone().unwrap_or_else(|| if is_live { "Live".into() } else { "Prematch".into() }),
                    home_team: home.clone(),
                    away_team: away.clone(),
                    start_time: None,
                    is_live,
                    bookmaker_slug: "winline".to_string(),
                    raw_url: None,
                    extra: HashMap::new(),
                };
                events.push(event);

                // Parse odds
                if let Some(odds) = py_event.home_odds {
                    if odds > 1.0 {
                        all_odds.push(Odd {
                            id: format!("{}-1", event_id),
                            event_id: event_id.clone(),
                            bookmaker_slug: "winline".to_string(),
                            market: "1X2".into(),
                            selection: "1".into(),
                            odds,
                            odds_type: OddsType::Home,
                            line: None,
                            timestamp: now,
                        });
                    }
                }
                if let Some(odds) = py_event.draw_odds {
                    if odds > 1.0 {
                        all_odds.push(Odd {
                            id: format!("{}-x", event_id),
                            event_id: event_id.clone(),
                            bookmaker_slug: "winline".to_string(),
                            market: "1X2".into(),
                            selection: "X".into(),
                            odds,
                            odds_type: OddsType::Draw,
                            line: None,
                            timestamp: now,
                        });
                    }
                }
                if let Some(odds) = py_event.away_odds {
                    if odds > 1.0 {
                        all_odds.push(Odd {
                            id: format!("{}-2", event_id),
                            event_id: event_id.clone(),
                            bookmaker_slug: "winline".to_string(),
                            market: "1X2".into(),
                            selection: "2".into(),
                            odds,
                            odds_type: OddsType::Away,
                            line: None,
                            timestamp: now,
                        });
                    }
                }
            }
        }

        (events, all_odds)
    }
}

#[async_trait]
impl BookmakerParser for WinlineParser {
    fn name(&self) -> &str { "Winline" }
    fn slug(&self) -> &str { "winline" }
    fn is_enabled(&self) -> bool { true }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Winline: calling Python parser...");
        let parser = self.clone();
        let py_events = tokio::task::spawn_blocking(move || parser.call_python_parser()).await.unwrap_or_default();
        let (events, _) = self.convert_events(&py_events);
        info!(count = events.len(), "Winline events parsed via Python");
        Ok(events)
    }

    async fn fetch_odds(&self, _event_id: &str) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Winline: calling Python parser for odds...");
        let parser = self.clone();
        let py_events = tokio::task::spawn_blocking(move || parser.call_python_parser()).await.unwrap_or_default();
        let (_, odds) = self.convert_events(&py_events);
        info!(count = odds.len(), "Winline odds parsed via Python");
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        info!("Winline: calling Python parser...");
        let parser = self.clone();
        let py_events = tokio::task::spawn_blocking(move || parser.call_python_parser()).await.unwrap_or_default();
        let (events, odds) = self.convert_events(&py_events);
        let elapsed = start.elapsed().as_millis() as u64;
        info!(events = events.len(), odds = odds.len(), time_ms = elapsed, "Winline fetch complete via Python");
        Ok(ParserResult::new("winline", events, odds, elapsed))
    }

    fn base_url(&self) -> &str { "https://winline.ru" }
    fn user_agent(&self) -> &str { "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" }
}
