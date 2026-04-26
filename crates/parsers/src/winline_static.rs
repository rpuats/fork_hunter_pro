// crates/parsers/src/winline_static.rs - Static Winline parser for proof-of-concept
use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use shared::{Event, Odd, OddsType, Sport};
use std::collections::HashMap;

const BOOKMAKER_SLUG: &str = "winline";

/// Static Winline parser - demonstrates event structure working
/// This uses cached data to prove the parser pattern works
/// Production version will fetch from WebSocket at: wss://wss.winline.ru/data_ng?client=newsite&nb=true
pub async fn parse_events() -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    let mut event_id = 1000u64;

    // Football (Sport ID 5)
    let football_events = vec![
        ("Real Madrid", "Alaves", true, "La Liga"),
        ("Manchester City", "Arsenal", true, "Premier League"),
        ("Bayern Munich", "Borussia Dortmund", true, "Bundesliga"),
        ("PSG", "Marseille", true, "Ligue 1"),
        ("Juventus", "Inter Milan", true, "Serie A"),
        ("Barcelona", "Valencia", false, "La Liga"),
        ("Chelsea", "Liverpool", false, "Premier League"),
        ("Napoli", "Roma", false, "Serie A"),
        ("Atletico Madrid", "Villarreal", false, "La Liga"),
        ("Tottenham", "Manchester United", false, "Premier League"),
        ("Dortmund", "Cologne", false, "Bundesliga"),
        ("Lyon", "Nice", false, "Ligue 1"),
    ];

    for (home, away, is_live, league) in football_events {
        event_id += 1;
        let event = Event {
            id: event_id.to_string(),
            sport: Sport::Football,
            league: league.to_string(),
            home_team: home.to_string(),
            away_team: away.to_string(),
            start_time: None,
            is_live,
            bookmaker_slug: BOOKMAKER_SLUG.to_string(),
            raw_url: if is_live {
                Some("https://winline.ru/live/football".to_string())
            } else {
                Some("https://winline.ru/football".to_string())
            },
            extra: HashMap::new(),
        };

        events.push(event);
    }

    // Basketball (Sport ID 3)
    let basketball_events = vec![
        ("Lakers", "Celtics", true),
        ("Warriors", "Suns", true),
        ("Heat", "Bucks", true),
        ("Nets", "Nuggets", false),
        ("Mavericks", "Grizzlies", false),
    ];

    for (home, away, is_live) in basketball_events {
        event_id += 1;
        let event = Event {
            id: event_id.to_string(),
            sport: Sport::Basketball,
            league: "NBA".to_string(),
            home_team: home.to_string(),
            away_team: away.to_string(),
            start_time: None,
            is_live,
            bookmaker_slug: BOOKMAKER_SLUG.to_string(),
            raw_url: if is_live {
                Some("https://winline.ru/live/basketball".to_string())
            } else {
                Some("https://winline.ru/basketball".to_string())
            },
            extra: HashMap::new(),
        };

        events.push(event);
    }

    // Hockey (Sport ID 31)
    let hockey_events = vec![
        ("CSKA Moscow", "SKA", true),
        ("Dynamo Moscow", "Spartak", true),
        ("Ak Bars", "Metallurg", false),
    ];

    for (home, away, is_live) in hockey_events {
        event_id += 1;
        let event = Event {
            id: event_id.to_string(),
            sport: Sport::Hockey,
            league: "KHL".to_string(),
            home_team: home.to_string(),
            away_team: away.to_string(),
            start_time: None,
            is_live,
            bookmaker_slug: BOOKMAKER_SLUG.to_string(),
            raw_url: if is_live {
                Some("https://winline.ru/live/hockey".to_string())
            } else {
                Some("https://winline.ru/hockey".to_string())
            },
            extra: HashMap::new(),
        };

        events.push(event);
    }

    Ok(events)
}

/// Production parser - will connect to WebSocket
/// For now, returns static data to demonstrate infrastructure works
pub async fn fetch_from_websocket() -> Result<Vec<Event>, String> {
    // TODO: Implement WebSocket connection to wss://wss.winline.ru/data_ng?client=newsite&nb=true
    // The WebSocket uses binary protocol (needs decoder)
    // For now, fallback to static data

    eprintln!("[WINLINE] WebSocket implementation pending - using static data");
    parse_events().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_events() {
        let events = parse_events().await.unwrap();
        assert!(events.len() > 10, "Should return multiple events");

        // Check structure
        let football_events: Vec<_> = events
            .iter()
            .filter(|e| {
                e.league.contains("Liga")
                    || e.league.contains("Premier")
                    || e.league.contains("Bundesliga")
            })
            .collect();
        assert!(!football_events.is_empty(), "Should have football events");

        // Check live events exist
        let live: Vec<_> = events.iter().filter(|e| e.is_live).collect();
        assert!(!live.is_empty(), "Should have live events");
    }
}
