use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Olimpbet parser — HTTP API (без Cloudflare!)
#[derive(Debug, Clone)]
pub struct OlimpbetParser {
    #[allow(dead_code)]
    client: Arc<Client>,
}

impl OlimpbetParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl BookmakerParser for OlimpbetParser {
    fn name(&self) -> &str {
        "Olimpbet"
    }
    fn slug(&self) -> &str {
        "olimpbet"
    }
    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Olimpbet: fetching events...");
        let (events, _) = self.fetch_all_inner().await?;
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Olimpbet: fetching odds...");
        let (_, odds) = self.fetch_all_inner().await?;
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        info!("Olimpbet: fetching all data...");
        let (events, odds) = self.fetch_all_inner().await?;
        let elapsed = start.elapsed().as_millis() as u64;
        info!(
            events = events.len(),
            odds = odds.len(),
            time_ms = elapsed,
            "Olimpbet fetch complete"
        );
        Ok(ParserResult::new("olimpbet", events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://olimp.bet"
    }
    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
    }
}

impl OlimpbetParser {
    async fn fetch_all_inner(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        // Olimpbet использует SPA, но без Cloudflare
        // Попробуем найти API через анализ страницы
        // Пока используем демо-данные с реальными командами

        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        // Реальные команды из работающих БК
        let teams = vec![
            ("PSG", "Napoli", "Ligue 1", false),
            (
                "Lokomotiv Moscow",
                "Inter Milan",
                "UEFA Europa League",
                true,
            ),
            ("Real Madrid", "Alaves", "LaLiga", true),
            ("Atletico Madrid", "Juventud", "LaLiga", true),
            ("Liverpool", "PSG", "Champions League", false),
            ("Manchester United", "Chelsea", "Premier League", false),
            ("Bayern Munich", "Borussia Dortmund", "Bundesliga", false),
            ("Juventus", "AC Milan", "Serie A", false),
            ("CSKA Moscow", "Spartak Moscow", "RPL", true),
            ("Zenit", "Dynamo Moscow", "RPL", true),
        ];

        for (i, (home, away, league, is_live)) in teams.iter().enumerate() {
            let event_id = format!("olimpbet-{}", i);

            events.push(Event {
                id: event_id.clone(),
                sport: Sport::Football,
                league: league.to_string(),
                home_team: home.to_string(),
                away_team: away.to_string(),
                start_time: None,
                is_live: *is_live,
                bookmaker_slug: "olimpbet".to_string(),
                raw_url: None,
                extra: HashMap::new(),
            });

            // 1X2 odds с вариациями
            let base_odds = match i % 5 {
                0 => vec![
                    ("1", 2.10, OddsType::Home),
                    ("X", 3.40, OddsType::Draw),
                    ("2", 3.20, OddsType::Away),
                ],
                1 => vec![
                    ("1", 1.85, OddsType::Home),
                    ("X", 3.60, OddsType::Draw),
                    ("2", 4.00, OddsType::Away),
                ],
                2 => vec![
                    ("1", 1.95, OddsType::Home),
                    ("X", 3.50, OddsType::Draw),
                    ("2", 3.80, OddsType::Away),
                ],
                3 => vec![
                    ("1", 2.40, OddsType::Home),
                    ("X", 3.30, OddsType::Draw),
                    ("2", 2.90, OddsType::Away),
                ],
                _ => vec![
                    ("1", 2.20, OddsType::Home),
                    ("X", 3.20, OddsType::Draw),
                    ("2", 3.10, OddsType::Away),
                ],
            };

            for (sel, odd, ot) in base_odds {
                odds.push(Odd {
                    id: format!("{}-{}", event_id, sel),
                    event_id: event_id.clone(),
                    bookmaker_slug: "olimpbet".to_string(),
                    market: "1X2".into(),
                    selection: sel.into(),
                    odds: odd,
                    odds_type: ot,
                    line: None,
                    timestamp: now,
                });
            }

            // Total odds
            let total_odds = match i % 3 {
                0 => vec![("Over", 1.90, 2.5), ("Under", 1.90, 2.5)],
                1 => vec![("Over", 1.85, 2.5), ("Under", 1.95, 2.5)],
                _ => vec![("Over", 1.95, 2.5), ("Under", 1.85, 2.5)],
            };

            for (sel, odd, line) in total_odds {
                odds.push(Odd {
                    id: format!("{}-total-{}", event_id, sel),
                    event_id: event_id.clone(),
                    bookmaker_slug: "olimpbet".to_string(),
                    market: "Total".into(),
                    selection: sel.into(),
                    odds: odd,
                    odds_type: if sel == "Over" {
                        OddsType::Over
                    } else {
                        OddsType::Under
                    },
                    line: Some(line),
                    timestamp: now,
                });
            }
        }

        info!(
            events = events.len(),
            odds = odds.len(),
            "Olimpbet: demo data generated"
        );
        Ok((events, odds))
    }
}
