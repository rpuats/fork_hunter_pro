/// Индекс щедрости БК — рассчитывает насколько «щедра» БК по отношению к игрокам.
/// Учитывает текущий snapshot рантайма: реальную маржу по рынкам, долю лучших коэффициентов
/// и покрытие событий по каждому виду спорта.
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use shared::{Event, GenerosityIndex, Odd, Sport};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BookmakerSportKey {
    bookmaker: String,
    sport: Sport,
}

#[derive(Clone)]
pub struct GenerosityIndexCalc {
    bookmaker_odds: Arc<DashMap<BookmakerSportKey, BookmakerStats>>,
}

#[derive(Debug, Clone, Default)]
struct BookmakerStats {
    pub total_odds: usize,
    pub total_events: usize,
    pub best_odds_count: usize,
    pub sum_margins: f64,
    pub market_count: usize,
    pub sum_odds: f64,
    pub updated_at: DateTime<Utc>,
}

impl GenerosityIndexCalc {
    pub fn new() -> Self {
        Self {
            bookmaker_odds: Arc::new(DashMap::new()),
        }
    }

    /// Обновить данные по событиям и коэффициентам
    pub fn update(&self, events: &[Event], all_odds: &[Odd]) {
        let updated_at = Utc::now();
        let event_lookup: HashMap<&str, &Event> = events
            .iter()
            .map(|event| (event.id.as_str(), event))
            .collect();
        let mut snapshot: HashMap<BookmakerSportKey, BookmakerStats> = HashMap::new();
        let mut seen_events: HashSet<(String, Sport, String)> = HashSet::new();
        let mut best_by_selection: HashMap<(Sport, String, String, String), (String, f64)> =
            HashMap::new();
        let mut markets: HashMap<(BookmakerSportKey, String, String, String), Vec<f64>> =
            HashMap::new();

        for odd in all_odds {
            let Some(event) = event_lookup.get(odd.event_id.as_str()) else {
                continue;
            };
            let event_key = comparable_event_key(event);

            let key = BookmakerSportKey {
                bookmaker: odd.bookmaker_slug.clone(),
                sport: event.sport,
            };
            let stats = snapshot
                .entry(key.clone())
                .or_insert_with(|| BookmakerStats {
                    updated_at,
                    ..BookmakerStats::default()
                });
            if seen_events.insert((odd.bookmaker_slug.clone(), event.sport, event_key.clone())) {
                stats.total_events += 1;
            }
            stats.total_odds += 1;
            stats.sum_odds += odd.odds;

            let best_key = (
                event.sport,
                event_key.clone(),
                odd.market.to_lowercase(),
                selection_key(odd),
            );
            match best_by_selection.get_mut(&best_key) {
                Some((bookmaker, best_odds)) if odd.odds > *best_odds => {
                    *bookmaker = odd.bookmaker_slug.clone();
                    *best_odds = odd.odds;
                }
                None => {
                    best_by_selection.insert(best_key, (odd.bookmaker_slug.clone(), odd.odds));
                }
                _ => {}
            }

            let market_key = (key, event_key, market_key(&odd.market), line_key(odd.line));
            markets.entry(market_key).or_default().push(odd.odds);
        }

        for ((sport, _, _, _), (bookmaker, _)) in best_by_selection {
            let key = BookmakerSportKey { bookmaker, sport };
            let stats = snapshot.entry(key).or_insert_with(|| BookmakerStats {
                updated_at,
                ..BookmakerStats::default()
            });
            stats.best_odds_count += 1;
        }

        for ((key, _, _, _), odds) in markets {
            if let Some(margin) = calculate_market_margin(&odds) {
                let stats = snapshot.entry(key).or_insert_with(|| BookmakerStats {
                    updated_at,
                    ..BookmakerStats::default()
                });
                stats.sum_margins += margin;
                stats.market_count += 1;
            }
        }

        self.bookmaker_odds.clear();
        for (key, stats) in snapshot {
            self.bookmaker_odds.insert(key, stats);
        }
    }

    /// Получить индекс щедрости для конкретной БК
    pub fn get_index(&self, bookmaker: &str, sport: Sport) -> GenerosityIndex {
        let stats = self
            .bookmaker_odds
            .get(&BookmakerSportKey {
                bookmaker: bookmaker.to_string(),
                sport,
            })
            .map(|e| e.value().clone())
            .unwrap_or_default();

        let avg_margin = if stats.market_count > 0 {
            stats.sum_margins / stats.market_count as f64
        } else {
            0.0
        };

        let avg_odds = if stats.total_odds > 0 {
            stats.sum_odds / stats.total_odds as f64
        } else {
            0.0
        };

        // Score: 0-10, где 10 = очень щедрый.
        // Компоненты: лучшие коэффициенты, низкая маржа, ширина покрытия.
        let avg_odds_score = if stats.total_odds > 0 {
            ((avg_odds - 1.0) / 4.0 * 2.0).clamp(0.0, 2.0)
        } else {
            0.0
        };

        let best_ratio_score = if stats.total_odds > 0 {
            (stats.best_odds_count as f64 / stats.total_odds as f64) * 5.0
        } else {
            0.0
        };

        let margin_score = if stats.market_count > 0 {
            (3.0 - (avg_margin / 5.0)).clamp(0.0, 3.0)
        } else {
            0.0
        };
        let coverage_score = if stats.total_events > 0 {
            (stats.total_events as f64 / 50.0).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let score = best_ratio_score + margin_score + coverage_score + avg_odds_score;

        GenerosityIndex {
            bookmaker: bookmaker.to_string(),
            sport,
            avg_margin,
            avg_odds,
            best_odds_count: stats.best_odds_count,
            total_events: stats.total_events,
            score,
            updated_at: stats.updated_at,
        }
    }

    /// Получить индексы для всех БК по виду спорта
    pub fn get_indices_by_sport(&self, sport: Sport) -> Vec<GenerosityIndex> {
        let mut indices = Vec::new();
        for entry in self.bookmaker_odds.iter() {
            let key = entry.key();
            if key.sport == sport {
                indices.push(self.get_index(&key.bookmaker, sport));
            }
        }
        sort_indices(&mut indices);
        indices
    }

    /// Получить индексы для всех БК по всем видам спорта
    pub fn get_all_indices(&self) -> Vec<GenerosityIndex> {
        let mut indices = Vec::new();
        for entry in self.bookmaker_odds.iter() {
            let key = entry.key();
            indices.push(self.get_index(&key.bookmaker, key.sport));
        }
        sort_indices(&mut indices);
        indices
    }

    /// Очистить данные
    pub fn clear(&self) {
        self.bookmaker_odds.clear();
    }
}

fn sort_indices(indices: &mut [GenerosityIndex]) {
    indices.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.bookmaker.cmp(&b.bookmaker))
            .then_with(|| format!("{:?}", a.sport).cmp(&format!("{:?}", b.sport)))
    });
}

fn line_key(line: Option<f64>) -> String {
    line.map(|value| format!("{value:.3}")).unwrap_or_default()
}

fn market_key(market: &str) -> String {
    market.trim().to_lowercase()
}

fn selection_key(odd: &Odd) -> String {
    let line = line_key(odd.line);
    if line.is_empty() {
        odd.selection.trim().to_lowercase()
    } else {
        format!("{}|{}", odd.selection.trim().to_lowercase(), line)
    }
}

fn comparable_event_key(event: &Event) -> String {
    let home = normalize_entity_name(&event.home_team);
    let away = normalize_entity_name(&event.away_team);
    let league = normalize_entity_name(&event.league);
    let (first, second) = if home <= away {
        (home, away)
    } else {
        (away, home)
    };

    format!(
        "{:?}|{}|{}|{}|{}",
        event.sport,
        if event.is_live { "live" } else { "prematch" },
        league,
        first,
        second
    )
}

fn normalize_entity_name(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn calculate_market_margin(odds: &[f64]) -> Option<f64> {
    if odds.len() < 2 {
        return None;
    }

    let implied_probability_sum: f64 = odds
        .iter()
        .filter(|odd| **odd > 1.0)
        .map(|odd| 1.0 / odd)
        .sum();
    if implied_probability_sum <= 0.0 {
        return None;
    }

    Some(((implied_probability_sum - 1.0) * 100.0).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(id: &str) -> Event {
        Event {
            id: id.to_string(),
            sport: Sport::Football,
            league: "Test League".into(),
            home_team: format!("Team A {id}"),
            away_team: format!("Team B {id}"),
            start_time: None,
            is_live: false,
            bookmaker_slug: String::new(),
            raw_url: None,
            extra: std::collections::HashMap::new(),
        }
    }

    fn make_odd(event_id: &str, bk: &str, selection: &str, odds: f64) -> Odd {
        use shared::odds::OddsType;
        Odd {
            id: format!("{}-{}-{}", event_id, bk, selection),
            event_id: event_id.into(),
            bookmaker_slug: bk.into(),
            market: "1X2".into(),
            selection: selection.into(),
            odds,
            odds_type: OddsType::Home,
            line: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_generosity_index_basic() {
        let calc = GenerosityIndexCalc::new();

        let events = vec![make_event("e1"), make_event("e2")];
        // bk1 щедрее — предлагает более высокие коэффициенты
        let odds = vec![
            make_odd("e1", "bk1", "1", 2.00), // margin = 50%
            make_odd("e1", "bk2", "1", 1.80), // margin = 44%
            make_odd("e2", "bk1", "1", 1.95), // margin = 48.7%
            make_odd("e2", "bk2", "1", 1.85), // margin = 45.9%
        ];

        calc.update(&events, &odds);

        let idx1 = calc.get_index("bk1", Sport::Football);
        let idx2 = calc.get_index("bk2", Sport::Football);

        // bk1 щедрее — у него 2 лучших коэффициента из 2
        assert!(
            idx1.score > idx2.score,
            "bk1 score {} should be > bk2 score {}",
            idx1.score,
            idx2.score
        );
        assert_eq!(idx1.best_odds_count, 2);
        assert_eq!(idx2.best_odds_count, 0);
        assert_eq!(idx1.total_events, 2);
        assert!(idx1.avg_margin >= 0.0);
    }

    #[test]
    fn test_generosity_index_multiple_events() {
        let calc = GenerosityIndexCalc::new();

        let events = vec![make_event("e1"), make_event("e2"), make_event("e3")];

        let odds = vec![
            // bk1 лучше в e1 и e3
            make_odd("e1", "bk1", "1", 2.10), // bk1 лучший
            make_odd("e1", "bk2", "1", 1.90),
            make_odd("e2", "bk1", "1", 1.95),
            make_odd("e2", "bk2", "1", 2.00), // bk2 лучший
            make_odd("e3", "bk1", "1", 2.05), // bk1 лучший
            make_odd("e3", "bk2", "1", 1.85),
        ];

        calc.update(&events, &odds);

        let all = calc.get_indices_by_sport(Sport::Football);
        assert_eq!(all.len(), 2);
        // bk1 должен быть первым (2 лучших vs 1 у bk2)
        assert_eq!(all[0].bookmaker, "bk1", "bk1 should be first");
    }

    #[test]
    fn test_generosity_index_tracks_sports_separately() {
        let calc = GenerosityIndexCalc::new();

        let football_event = make_event("e1");
        let mut tennis_event = make_event("e2");
        tennis_event.sport = Sport::Tennis;
        tennis_event.bookmaker_slug = "bk1".into();

        let mut football_bk2 = make_event("e3");
        football_bk2.bookmaker_slug = "bk2".into();

        let mut football_bk1 = football_event.clone();
        football_bk1.bookmaker_slug = "bk1".into();

        let events = vec![football_bk1, football_bk2, tennis_event];
        let odds = vec![
            make_odd("e1", "bk1", "1", 2.10),
            make_odd("e1", "bk2", "1", 1.90),
            make_odd("e2", "bk1", "player_a", 1.95),
            make_odd("e2", "bk1", "player_b", 1.95),
        ];

        calc.update(&events, &odds);

        let football = calc.get_indices_by_sport(Sport::Football);
        let tennis = calc.get_indices_by_sport(Sport::Tennis);
        let all = calc.get_all_indices();

        assert_eq!(football.len(), 2);
        assert_eq!(tennis.len(), 1);
        assert_eq!(all.len(), 3);
        assert_eq!(tennis[0].bookmaker, "bk1");
        assert_eq!(tennis[0].total_events, 1);
    }

    #[test]
    fn test_generosity_index_uses_market_overround_margin() {
        let calc = GenerosityIndexCalc::new();

        let mut event = make_event("e1");
        event.bookmaker_slug = "bk1".into();
        let events = vec![event];
        let odds = vec![
            make_odd("e1", "bk1", "1", 2.0),
            make_odd("e1", "bk1", "X", 3.5),
            make_odd("e1", "bk1", "2", 4.0),
        ];

        calc.update(&events, &odds);

        let idx = calc.get_index("bk1", Sport::Football);
        let expected_margin = ((1.0 / 2.0) + (1.0 / 3.5) + (1.0 / 4.0) - 1.0) * 100.0;

        assert!((idx.avg_margin - expected_margin).abs() < 0.0001);
        assert_eq!(idx.total_events, 1);
    }

    #[test]
    fn test_generosity_index_empty() {
        let calc = GenerosityIndexCalc::new();
        let idx = calc.get_index("unknown", Sport::Football);
        assert_eq!(idx.score, 0.0);
        assert_eq!(idx.avg_margin, 0.0);
    }
}
