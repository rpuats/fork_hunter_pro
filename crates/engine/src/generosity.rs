/// Индекс щедрости БК — рассчитывает насколько «щедра» БК по отношению к игрокам.
/// Учитывает: среднюю маржу, количество лучших коэффициентов, глубину линии.
use chrono::Utc;
use dashmap::DashMap;
use shared::{Event, GenerosityIndex, Odd, Sport};
use std::sync::Arc;

#[derive(Clone)]
pub struct GenerosityIndexCalc {
    bookmaker_odds: Arc<DashMap<String, BookmakerStats>>,
}

#[derive(Debug, Clone, Default)]
struct BookmakerStats {
    pub total_odds: usize,
    pub best_odds_count: usize,
    pub sum_margins: f64,
    pub sum_odds: f64,
    #[allow(dead_code)]
    pub events_by_sport: DashMap<String, usize>,
}

impl GenerosityIndexCalc {
    pub fn new() -> Self {
        Self {
            bookmaker_odds: Arc::new(DashMap::new()),
        }
    }

    /// Обновить данные по событиям и коэффициентам
    pub fn update(&self, _events: &[Event], all_odds: &[Odd]) {
        // Собираем статистику по каждой БК из коэффициентов
        let bk_stats: DashMap<String, BookmakerStats> = DashMap::new();

        // Инициализируем БК из odds
        for odd in all_odds {
            bk_stats.entry(odd.bookmaker_slug.clone()).or_default();
        }

        // Считаем коэффициенты
        for odd in all_odds {
            let mut stats = bk_stats.entry(odd.bookmaker_slug.clone()).or_default();
            stats.total_odds += 1;
            stats.sum_odds += odd.odds;
        }

        // Находим лучшие коэффициенты для каждого исхода
        let best_by_selection: DashMap<String, (String, f64)> = DashMap::new();
        for odd in all_odds {
            let key = format!("{}|{}|{}", odd.event_id, odd.market, odd.selection);
            let mut entry = best_by_selection
                .entry(key)
                .or_insert_with(|| (odd.bookmaker_slug.clone(), odd.odds));
            if odd.odds > entry.1 {
                *entry = (odd.bookmaker_slug.clone(), odd.odds);
            }
        }

        // Считаем сколько раз каждая БК была лучшей
        for entry in best_by_selection.iter() {
            let bk_slug = &entry.value().0;
            if let Some(mut stats) = bk_stats.get_mut(bk_slug.as_str()) {
                stats.best_odds_count += 1;
            }
        }

        // Рассчитываем средние маржи
        for odd in all_odds {
            if let Some(mut stats) = bk_stats.get_mut(&odd.bookmaker_slug) {
                // Упрощённая оценка маржи: margin = 1 - 1/odds (для одного исхода)
                let implied_prob = 1.0 / odd.odds;
                let margin = (1.0 - implied_prob) * 100.0;
                stats.sum_margins += margin.max(0.0);
            }
        }

        // Сохраняем
        for entry in bk_stats.iter() {
            let bk_slug = entry.key();
            let stats = entry.value();
            let mut my_entry = self.bookmaker_odds.entry(bk_slug.clone()).or_default();
            *my_entry = stats.clone();
        }
    }

    /// Получить индекс щедрости для конкретной БК
    pub fn get_index(&self, bookmaker: &str, sport: Sport) -> GenerosityIndex {
        let stats = self
            .bookmaker_odds
            .get(bookmaker)
            .map(|e| e.value().clone())
            .unwrap_or_default();

        let avg_margin = if stats.total_odds > 0 {
            stats.sum_margins / stats.total_odds as f64
        } else {
            0.0
        };

        let avg_odds = if stats.total_odds > 0 {
            stats.sum_odds / stats.total_odds as f64
        } else {
            0.0
        };

        // Score: 0-10, где 10 = очень щедрый
        // Щедрость = БК даёт игрокам лучшие коэффициенты
        // Компоненты:
        //   - avg_odds_score: чем выше средние odds, тем щедрее (0-5)
        //   - best_ratio: доля лучших коэффициентов (0-5)
        let avg_odds_score = if stats.total_odds > 0 {
            // Нормализуем: odds 1.0-5.0 → score 0-5
            ((stats.sum_odds / stats.total_odds as f64 - 1.0) / 4.0 * 5.0).clamp(0.0, 5.0)
        } else {
            0.0
        };

        let best_ratio_score = if stats.total_odds > 0 {
            (stats.best_odds_count as f64 / stats.total_odds as f64) * 5.0
        } else {
            0.0
        };

        let score = avg_odds_score + best_ratio_score;

        GenerosityIndex {
            bookmaker: bookmaker.to_string(),
            sport,
            avg_margin,
            avg_odds,
            best_odds_count: stats.best_odds_count,
            total_events: stats.total_odds,
            score,
            updated_at: Utc::now(),
        }
    }

    /// Получить индексы для всех БК
    pub fn get_all_indices(&self, sport: Sport) -> Vec<GenerosityIndex> {
        let mut indices = Vec::new();
        for entry in self.bookmaker_odds.iter() {
            let bk_slug = entry.key();
            indices.push(self.get_index(bk_slug, sport));
        }
        indices.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        indices
    }

    /// Очистить данные
    pub fn clear(&self) {
        self.bookmaker_odds.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(id: &str) -> Event {
        Event {
            id: id.to_string(),
            sport: Sport::Football,
            league: "Test League".into(),
            home_team: "Team A".into(),
            away_team: "Team B".into(),
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

        let all = calc.get_all_indices(Sport::Football);
        assert_eq!(all.len(), 2);
        // bk1 должен быть первым (2 лучших vs 1 у bk2)
        assert_eq!(all[0].bookmaker, "bk1", "bk1 should be first");
    }

    #[test]
    fn test_generosity_index_empty() {
        let calc = GenerosityIndexCalc::new();
        let idx = calc.get_index("unknown", Sport::Football);
        assert_eq!(idx.score, 0.0);
        assert_eq!(idx.avg_margin, 0.0);
    }
}
