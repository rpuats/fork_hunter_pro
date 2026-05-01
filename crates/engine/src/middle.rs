use shared::{Event, Odd, Sport};
use std::collections::HashMap;

/// Middle opportunity - когда две ставки на один матч могут дать гарантированный профит
/// при определенных исходах (например, Ф1(-1.5) и Ф2(+2.5) в баскетболе)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MiddleOpportunity {
    pub event_id: String,
    pub sport: Sport,
    pub bookmaker_a: String,
    pub bookmaker_b: String,
    pub market_a: String,
    pub market_b: String,
    pub odds_a: f64,
    pub odds_b: f64,
    pub win_win_scenario: Option<String>, // Описание сценария где обе ставки выигрывают
    pub loss_win_scenario: Option<String>, // Описание сценария где одна проигрывает, вторая выигрывает
    pub max_profit: f64,
    pub max_loss: f64,
    pub expected_value: f64,
    pub confidence_score: f64, // 0.0 - 1.0
}

/// Результат поиска middles
#[derive(Debug, Clone)]
pub struct MiddleSearchResult {
    pub opportunities: Vec<MiddleOpportunity>,
    pub searched_events: usize,
    pub found_middles: usize,
    pub search_time_ms: u64,
}

pub struct MiddleDetector {
    min_profit_threshold: f64,
    max_loss_threshold: f64,
}

impl Default for MiddleDetector {
    fn default() -> Self {
        Self {
            min_profit_threshold: 0.02, // 2% минимум
            max_loss_threshold: 0.5,    // максимум 50% потери
        }
    }
}

impl MiddleDetector {
    pub fn new(min_profit_threshold: f64, max_loss_threshold: f64) -> Self {
        Self {
            min_profit_threshold,
            max_loss_threshold,
        }
    }

    /// Ищет middle opportunities между двумя наборами odds
    pub fn find_middles(&self, events: &[Event], odds: &[Odd]) -> MiddleSearchResult {
        let start_time = std::time::Instant::now();
        let mut opportunities = Vec::new();

        // Группируем odds по событиям
        let mut odds_by_event: HashMap<String, Vec<&Odd>> = HashMap::new();
        for odd in odds {
            odds_by_event
                .entry(odd.event_id.clone())
                .or_default()
                .push(odd);
        }

        for event in events {
            if let Some(event_odds) = odds_by_event.get(&event.id) {
                let event_middles = self.find_event_middles(event, event_odds);
                opportunities.extend(event_middles);
            }
        }

        let search_time = start_time.elapsed().as_millis() as u64;

        let found_middles = opportunities.len();
        MiddleSearchResult {
            opportunities,
            searched_events: events.len(),
            found_middles,
            search_time_ms: search_time,
        }
    }

    /// Ищет middles для одного события
    fn find_event_middles(&self, event: &Event, odds: &[&Odd]) -> Vec<MiddleOpportunity> {
        let mut opportunities = Vec::new();

        // Группируем по рынкам
        let mut odds_by_market: HashMap<String, Vec<&Odd>> = HashMap::new();
        for odd in odds {
            odds_by_market
                .entry(odd.market.clone())
                .or_default()
                .push(odd);
        }

        // Ищем middles между разными рынками
        let markets: Vec<String> = odds_by_market.keys().cloned().collect();
        for i in 0..markets.len() {
            for j in (i + 1)..markets.len() {
                let market_a = &markets[i];
                let market_b = &markets[j];

                if let (Some(odds_a), Some(odds_b)) =
                    (odds_by_market.get(market_a), odds_by_market.get(market_b))
                {
                    let market_middles =
                        self.find_market_middles(event, market_a, odds_a, market_b, odds_b);
                    opportunities.extend(market_middles);
                }
            }
        }

        opportunities
    }

    /// Ищет middles между двумя конкретными рынками
    fn find_market_middles(
        &self,
        event: &Event,
        market_a: &str,
        odds_a: &[&Odd],
        market_b: &str,
        odds_b: &[&Odd],
    ) -> Vec<MiddleOpportunity> {
        let mut opportunities = Vec::new();

        // Проверяем handicaps и totals - основные кандидаты на middles
        if self.is_handicap_market(market_a) && self.is_handicap_market(market_b) {
            opportunities
                .extend(self.find_handicap_middles(event, market_a, odds_a, market_b, odds_b));
        }

        if self.is_total_market(market_a) && self.is_total_market(market_b) {
            opportunities
                .extend(self.find_total_middles(event, market_a, odds_a, market_b, odds_b));
        }

        opportunities
    }

    /// Ищет middles между двумя handicap рынками
    fn find_handicap_middles(
        &self,
        event: &Event,
        market_a: &str,
        odds_a: &[&Odd],
        market_b: &str,
        odds_b: &[&Odd],
    ) -> Vec<MiddleOpportunity> {
        let mut opportunities = Vec::new();

        // Извлекаем handicap values
        let handicap_a = self.extract_handicap_value(market_a);
        let handicap_b = self.extract_handicap_value(market_b);

        if handicap_a.is_none() || handicap_b.is_none() {
            return opportunities;
        }

        let ha = handicap_a.unwrap();
        let hb = handicap_b.unwrap();

        // Проверяем возможность middle
        if (ha - hb).abs() <= 1.0 {
            // handicaps близки
            // Ищем пересекающиеся исходы
            for odd_a in odds_a {
                for odd_b in odds_b {
                    let middle = self.analyze_handicap_middle(event, odd_a, odd_b, ha, hb);
                    if let Some(middle) = middle {
                        opportunities.push(middle);
                    }
                }
            }
        }

        opportunities
    }

    /// Анализирует конкретную пару handicap ставок на middle
    fn analyze_handicap_middle(
        &self,
        event: &Event,
        odd_a: &Odd,
        odd_b: &Odd,
        _handicap_a: f64,
        _handicap_b: f64,
    ) -> Option<MiddleOpportunity> {
        // Упрощенная логика для тестирования
        let stake = 1000.0;
        let profit = (stake / odd_a.odds + stake / odd_b.odds - 2.0) * stake;

        if profit > self.min_profit_threshold * stake {
            Some(MiddleOpportunity {
                event_id: event.id.clone(),
                sport: event.sport,
                bookmaker_a: odd_a.bookmaker_slug.clone(),
                bookmaker_b: odd_b.bookmaker_slug.clone(),
                market_a: odd_a.market.clone(),
                market_b: odd_b.market.clone(),
                odds_a: odd_a.odds,
                odds_b: odd_b.odds,
                win_win_scenario: Some("Win-win scenario".to_string()),
                loss_win_scenario: Some("Loss-win scenario".to_string()),
                max_profit: profit,
                max_loss: -stake,
                expected_value: profit * 0.5,
                confidence_score: 0.7,
            })
        } else {
            None
        }
    }

    /// Ищет middles между total рынками
    fn find_total_middles(
        &self,
        _event: &Event,
        _market_a: &str,
        _odds_a: &[&Odd],
        _market_b: &str,
        _odds_b: &[&Odd],
    ) -> Vec<MiddleOpportunity> {
        // Аналогичная логика для totals
        vec![] // Заглушка - реализовать позже
    }

    /// Вспомогательные методы
    fn is_handicap_market(&self, market: &str) -> bool {
        market.to_lowercase().contains("handicap")
            || market.to_lowercase().contains("фора")
            || market.to_lowercase().contains("asian handicap")
    }

    fn is_total_market(&self, market: &str) -> bool {
        market.to_lowercase().contains("total")
            || market.to_lowercase().contains("тотал")
            || market.to_lowercase().contains("over")
            || market.to_lowercase().contains("under")
    }

    fn extract_handicap_value(&self, market: &str) -> Option<f64> {
        // Пример: "Handicap (-1.5)" -> -1.5
        let re = regex::Regex::new(r"[-+]?(\d+(?:\.\d+)?)").ok()?;
        re.captures(market)
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .map(|v| if market.contains('-') { -v } else { v })
    }

    fn calculate_profit_scenarios(
        &self,
        odd_a: &Odd,
        odd_b: &Odd,
        ha: f64,
        hb: f64,
    ) -> ProfitScenarios {
        // Упрощенная математика middles
        // В реальности нужна сложная модель вероятностей

        ProfitScenarios {
            max_profit: 500.0,     // Заглушка
            max_loss: -200.0,      // Заглушка
            expected_value: 150.0, // Заглушка
        }
    }
}

#[derive(Debug)]
struct ProfitScenarios {
    max_profit: f64,
    max_loss: f64,
    expected_value: f64,
}
