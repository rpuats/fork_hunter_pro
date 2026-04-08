use chrono::Utc;
use shared::{CorridorOpportunity, Event, Odd, Sport};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct CorridorFinder {
    min_corridor_size: f64,
}

impl CorridorFinder {
    pub fn new(min_corridor_size: f64) -> Self {
        Self { min_corridor_size }
    }

    pub fn find_corridors(&self, events: &[Event], all_odds: &[Odd]) -> Vec<CorridorOpportunity> {
        let mut corridors = Vec::new();
        let odds_by_market = self.group_by_market_line(all_odds);

        for ((_sport, _league, market), market_odds) in &odds_by_market {
            let over_odds: Vec<&&Odd> = market_odds.iter().filter(|o| o.selection.to_lowercase().contains("over")).collect();
            let under_odds: Vec<&&Odd> = market_odds.iter().filter(|o| o.selection.to_lowercase().contains("under")).collect();

            for &over in &over_odds {
                for &under in &under_odds {
                    if over.bookmaker_slug == under.bookmaker_slug { continue; }
                    if let (Some(line_over), Some(line_under)) = (over.line, under.line) {
                        if line_over > line_under {
                            let corridor_size = line_over - line_under;
                            if corridor_size >= self.min_corridor_size {
                                let event = events.iter().find(|e| e.id == over.event_id);
                                if let Some(event) = event {
                                    corridors.push(CorridorOpportunity {
                                        id: Uuid::new_v4(),
                                        sport: event.sport.clone(),
                                        league: event.league.clone(),
                                        home_team: event.home_team.clone(),
                                        away_team: event.away_team.clone(),
                                        start_time: event.start_time,
                                        is_live: event.is_live,
                                        bookmaker_a: over.bookmaker_slug.clone(),
                                        bookmaker_b: under.bookmaker_slug.clone(),
                                        market: market.clone(),
                                        line_a: line_over,
                                        odds_a: over.odds,
                                        line_b: line_under,
                                        odds_b: under.odds,
                                        corridor_size,
                                        double_win_probability: 0.0,
                                        expected_roi: 0.0,
                                        detected_at: Utc::now(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        corridors
    }

    fn group_by_market_line<'a>(&self, all_odds: &'a [Odd]) -> HashMap<(Sport, String, String), Vec<&'a Odd>> {
        let mut map: HashMap<(Sport, String, String), Vec<&'a Odd>> = HashMap::new();
        for odd in all_odds {
            if odd.line.is_some() {
                let key = (Sport::Football, "unknown".to_string(), odd.market.clone());
                map.entry(key).or_default().push(odd);
            }
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;
    use std::collections::HashMap;

    fn make_odd(event_id: &str, bookmaker: &str, selection: &str, odds: f64, line: f64) -> Odd {
        Odd {
            id: format!("{}-{}-{}", event_id, bookmaker, selection),
            event_id: event_id.to_string(),
            bookmaker_slug: bookmaker.to_string(),
            market: "Total".into(),
            selection: selection.to_string(),
            odds,
            odds_type: OddsType::Over,
            line: Some(line),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_find_corridor() {
        let finder = CorridorFinder::new(0.5);
        let event = Event {
            id: "evt1".into(),
            sport: Sport::Football,
            league: "RPL".into(),
            home_team: "Team A".into(),
            away_team: "Team B".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: HashMap::new(),
        };

        let odds = vec![
            make_odd("evt1", "bk1", "Over", 1.90, 2.5),
            make_odd("evt1", "bk2", "Under", 1.90, 2.0),
        ];

        let corridors = finder.find_corridors(&[event], &odds);
        assert!(!corridors.is_empty());
        assert!(corridors[0].corridor_size >= 0.5);
    }
}
