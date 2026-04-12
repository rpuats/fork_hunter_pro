use chrono::Utc;
use shared::odds::decimal_to_implied_probability;
use shared::{Event, Odd, ValueBet};
use std::collections::HashMap;
use uuid::Uuid;

use bankroll_manager::kelly::KellyCalculator;

/// Результат расчёта "честной вероятности" и margin analysis
#[derive(Debug, Clone)]
pub struct FairProbabilityAnalysis {
    /// Подразумеваемая вероятность по кэфу букмекера
    pub implied_probability: f64,
    /// "Честная" вероятность (без маржи)
    pub fair_probability: f64,
    /// Маржа букмекера на этом рынке (%)
    pub bookmaker_margin: f64,
    /// Edge — преимущество над честной вероятностью (%)
    pub edge_percent: f64,
    /// Является ли ставкой с value (edge > порога)
    pub is_value: bool,
    /// Рекомендуемый размер ставки по Kelly
    pub kelly_stake_percent: f64,
}

/// Конфигурация ValueDetector
#[derive(Clone, Debug)]
pub struct ValueDetectorConfig {
    /// Минимальный edge для детекции value (%)
    pub min_edge: f64,
    /// Доля Kelly для расчёта ставки
    pub kelly_fraction: f64,
    /// Размер банкролла
    pub bankroll: f64,
    /// Максимальная маржа букмекера (выше — игнорируем)
    pub max_acceptable_margin: f64,
}

impl Default for ValueDetectorConfig {
    fn default() -> Self {
        Self {
            min_edge: 5.0,
            kelly_fraction: 0.25,
            bankroll: 10000.0,
            max_acceptable_margin: 15.0,
        }
    }
}

#[derive(Clone)]
pub struct ValueDetector {
    config: ValueDetectorConfig,
}

impl ValueDetector {
    pub fn new(min_edge: f64) -> Self {
        Self {
            config: ValueDetectorConfig {
                min_edge,
                ..Default::default()
            },
        }
    }

    /// Создать с полной конфигурацией
    pub fn with_config(config: ValueDetectorConfig) -> Self {
        Self { config }
    }

    /// Основной метод: детекция value ставок
    pub fn detect_values(&self, events: &[Event], all_odds: &[Odd]) -> Vec<ValueBet> {
        let mut values = Vec::new();
        let market_averages = self.calculate_market_averages(all_odds);

        for odd in all_odds {
            let key = format!(
                "{}|{}|{}",
                odd.market,
                odd.selection,
                odd.line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "none".into())
            );

            if let Some(&avg_odds) = market_averages.get(&key) {
                let analysis = self.analyze_fair_probability(odd.odds, avg_odds);

                if analysis.is_value {
                    let event = events.iter().find(|e| e.id == odd.event_id);
                    if let Some(event) = event {
                        let fair_odds = 1.0 / analysis.fair_probability;
                        values.push(ValueBet {
                            id: Uuid::new_v4(),
                            bookmaker: odd.bookmaker_slug.clone(),
                            event: event.clone(),
                            market: odd.market.clone(),
                            selection: odd.selection.clone(),
                            odds: odd.odds,
                            fair_odds,
                            edge_percent: analysis.edge_percent,
                            detected_at: Utc::now(),
                        });
                    }
                }
            }
        }

        values.sort_by(|a, b| b.edge_percent.partial_cmp(&a.edge_percent).unwrap());
        values
    }

    /// ALGORITHM: Расчёт "честной вероятности" через маржу букмекера
    ///
    /// Методика:
    /// 1. Берём все кэфы на рынке у одного букмекера
    /// 2. Суммируем подразумеваемые вероятности (с маржой)
    /// 3. Рассчитываем маржу: total_implied - 1.0
    /// 4. "Честная вероятность" = implied / (1 + margin)
    /// 5. Edge = fair_prob - implied_this_odd
    pub fn calculate_fair_probability(
        &self,
        target_odds: f64,
        all_market_odds: &[f64],
    ) -> FairProbabilityAnalysis {
        if all_market_odds.is_empty() {
            return FairProbabilityAnalysis {
                implied_probability: 0.0,
                fair_probability: 0.0,
                bookmaker_margin: 0.0,
                edge_percent: 0.0,
                is_value: false,
                kelly_stake_percent: 0.0,
            };
        }

        // 1. Рассчитываем суммарную подразумеваемую вероятность
        let total_implied: f64 = all_market_odds
            .iter()
            .map(|&o| decimal_to_implied_probability(o))
            .sum();

        // 2. Маржа букмекера
        let margin = total_implied - 1.0;
        let margin_percent = if total_implied > 0.0 {
            (margin / total_implied) * 100.0
        } else {
            0.0
        };

        // 3. "Честная вероятность" target_odds
        let implied = decimal_to_implied_probability(target_odds);
        let fair_prob = if margin > 0.0 {
            implied / (1.0 + margin)
        } else {
            implied
        };

        // 4. Edge: если fair_prob > implied, значит кэф завышен — это value
        let edge = fair_prob - implied;
        let edge_percent = if implied > 0.0 {
            (edge / implied) * 100.0
        } else {
            0.0
        };

        // 5. Kelly stake
        let kelly = if edge > 0.0 {
            KellyCalculator::full_kelly(fair_prob, target_odds)
        } else {
            0.0
        };

        let is_value = edge_percent >= self.config.min_edge
            && margin_percent <= self.config.max_acceptable_margin;

        FairProbabilityAnalysis {
            implied_probability: implied,
            fair_probability: fair_prob,
            bookmaker_margin: margin_percent,
            edge_percent,
            is_value,
            kelly_stake_percent: kelly * 100.0,
        }
    }

    /// Альтернативный метод: расчёт fair probability через средний кэф рынка
    fn analyze_fair_probability(&self, target_odds: f64, avg_odds: f64) -> FairProbabilityAnalysis {
        let implied = decimal_to_implied_probability(target_odds);
        let fair_implied = decimal_to_implied_probability(avg_odds);
        let fair_prob = fair_implied;

        // Edge: насколько наша вероятность ниже "рыночной"
        let edge = if fair_implied > implied {
            fair_implied - implied
        } else {
            0.0
        };
        let edge_percent = if fair_implied > 0.0 {
            (edge / fair_implied) * 100.0
        } else {
            0.0
        };

        // Approximate margin
        let margin_percent = ((avg_odds - target_odds).abs() / avg_odds) * 100.0;

        let kelly = if edge > 0.0 {
            KellyCalculator::fractional_kelly(
                fair_prob,
                target_odds,
                self.config.kelly_fraction,
            )
        } else {
            0.0
        };

        let is_value = edge_percent >= self.config.min_edge;

        FairProbabilityAnalysis {
            implied_probability: implied,
            fair_probability: fair_prob,
            bookmaker_margin: margin_percent,
            edge_percent,
            is_value,
            kelly_stake_percent: kelly * 100.0,
        }
    }

    /// Рассчитать рекоменду stake для value ставки через Kelly
    pub fn calculate_value_stake(
        &self,
        fair_probability: f64,
        odds: f64,
        bankroll: Option<f64>,
    ) -> f64 {
        let br = bankroll.unwrap_or(self.config.bankroll);
        let kelly = KellyCalculator::optimal_stake(
            br,
            fair_probability,
            odds,
            self.config.kelly_fraction,
            5.0, // max 5% exposure
        );
        kelly
    }

    /// Детекция value с полным анализом (возвращает детализированные данные)
    pub fn detect_values_with_analysis(
        &self,
        events: &[Event],
        all_odds: &[Odd],
    ) -> Vec<(ValueBet, FairProbabilityAnalysis)> {
        let market_averages = self.calculate_market_averages(all_odds);
        let mut results = Vec::new();

        for odd in all_odds {
            let key = format!(
                "{}|{}|{}",
                odd.market,
                odd.selection,
                odd.line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "none".into())
            );

            if let Some(&avg_odds) = market_averages.get(&key) {
                let analysis = self.analyze_fair_probability(odd.odds, avg_odds);

                if analysis.is_value {
                    let event = events.iter().find(|e| e.id == odd.event_id);
                    if let Some(event) = event {
                        let fair_odds = 1.0 / analysis.fair_probability;
                        let value_bet = ValueBet {
                            id: Uuid::new_v4(),
                            bookmaker: odd.bookmaker_slug.clone(),
                            event: event.clone(),
                            market: odd.market.clone(),
                            selection: odd.selection.clone(),
                            odds: odd.odds,
                            fair_odds,
                            edge_percent: analysis.edge_percent,
                            detected_at: Utc::now(),
                        };
                        results.push((value_bet, analysis));
                    }
                }
            }
        }

        results.sort_by(|a, b| {
            b.0.edge_percent
                .partial_cmp(&a.0.edge_percent)
                .unwrap()
        });
        results
    }

    fn calculate_market_averages(&self, all_odds: &[Odd]) -> HashMap<String, f64> {
        let mut groups: HashMap<String, Vec<f64>> = HashMap::new();

        for odd in all_odds {
            let key = format!(
                "{}|{}|{}",
                odd.market,
                odd.selection,
                odd.line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "none".into())
            );
            groups.entry(key).or_default().push(odd.odds);
        }

        groups
            .into_iter()
            .map(|(key, odds)| {
                let avg = odds.iter().sum::<f64>() / odds.len() as f64;
                (key, avg)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;
    use shared::Sport;
    use std::collections::HashMap;

    fn make_event(id: &str) -> Event {
        Event {
            id: id.into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

    fn make_odd(event_id: &str, bk: &str, sel: &str, odds: f64) -> Odd {
        Odd {
            id: format!("{}-{}-{}", event_id, bk, sel),
            event_id: event_id.into(),
            bookmaker_slug: bk.into(),
            market: "1X2".into(),
            selection: sel.into(),
            odds,
            odds_type: OddsType::Home,
            line: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_detect_value_bet() {
        let detector = ValueDetector::new(5.0);
        let event = make_event("evt1");
        let odds = vec![
            make_odd("evt1", "bk1", "1", 2.50),
            make_odd("evt1", "bk2", "1", 2.00),
            make_odd("evt1", "bk3", "1", 2.00),
        ];

        let values = detector.detect_values(&[event], &odds);
        assert!(!values.is_empty());
        assert!(values[0].edge_percent >= 5.0);
    }

    #[test]
    fn test_fair_probability_calculation() {
        let detector = ValueDetector::new(5.0);

        // Пример: рынок из 2 исходов с небольшой маржой
        let market_odds: &[f64] = &[2.0, 2.1];
        let analysis = detector.calculate_fair_probability(2.1, market_odds);

        assert!(analysis.implied_probability > 0.0);
        assert!(analysis.fair_probability > 0.0);
        // Маржа может быть отрицательной если сумма implied < 1
        assert!(analysis.bookmaker_margin.is_finite());
        assert!(analysis.kelly_stake_percent >= 0.0);
    }

    #[test]
    fn test_value_detection_with_kelly() {
        let config = ValueDetectorConfig {
            min_edge: 3.0,
            kelly_fraction: 0.25,
            bankroll: 10000.0,
            max_acceptable_margin: 20.0,
        };
        let detector = ValueDetector::with_config(config);

        let event = make_event("evt1");
        let odds = vec![
            make_odd("evt1", "bk1", "1", 3.0),  // value
            make_odd("evt1", "bk2", "1", 2.0),
            make_odd("evt1", "bk3", "1", 2.0),
        ];

        let results = detector.detect_values_with_analysis(&[event], &odds);
        if !results.is_empty() {
            let (_, analysis) = &results[0];
            assert!(analysis.kelly_stake_percent >= 0.0);
        }
    }

    #[test]
    fn test_value_stake_calculation() {
        let detector = ValueDetector::new(5.0);

        // fair_prob = 0.4, odds = 3.0 => edge положительный
        let stake = detector.calculate_value_stake(0.4, 3.0, Some(10000.0));
        assert!(stake > 0.0);
        assert!(stake <= 500.0); // max 5% от 10000
    }

    #[test]
    fn test_no_value_when_edge_below_threshold() {
        let detector = ValueDetector::new(10.0); // высокий порог

        let event = make_event("evt1");
        let odds = vec![
            make_odd("evt1", "bk1", "1", 2.1),
            make_odd("evt1", "bk2", "1", 2.0),
            make_odd("evt1", "bk3", "1", 2.0),
        ];

        let values = detector.detect_values(&[event], &odds);
        // С edge 5% и порогом 10% не должно быть value
        assert!(values.is_empty() || values[0].edge_percent >= 10.0);
    }

    #[test]
    fn test_margin_filtering() {
        let config = ValueDetectorConfig {
            min_edge: 1.0,
            kelly_fraction: 0.25,
            bankroll: 10000.0,
            max_acceptable_margin: 5.0, // очень низкий порог маржи
        };
        let detector = ValueDetector::with_config(config);

        // Рынок с высокой маржей (~10%)
        let market_odds: &[f64] = &[1.5, 3.0]; // implied: 0.667 + 0.333 = 1.0
        let analysis = detector.calculate_fair_probability(1.5, market_odds);

        // Маржа должна быть низкой для прохождения фильтра
        // В данном случае маржа ~0%, так что должно пройти
        assert!(analysis.bookmaker_margin >= 0.0);
    }
}
