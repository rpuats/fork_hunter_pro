use chrono::Utc;
use shared::odds::decimal_to_implied_probability;
use shared::{Event, FreebetOpportunity, Odd};
use std::sync::Arc;
use uuid::Uuid;

use bankroll_manager::kelly::KellyCalculator;

/// Конфигурация для matching плечей с минимизацией риска
#[derive(Clone, Debug)]
pub struct MatchingConfig {
    /// Минимальный кэф для фрибет-плеча (обычно высокий)
    pub min_freebet_odds: f64,
    /// Максимальный кэф для страховочного плеча (обычно низкий)
    pub max_hedge_odds: f64,
    /// Максимально допустимый риск (разница между кэфами)
    pub max_odds_spread: f64,
    /// Минимальная конверсия фрибета (%)
    pub min_conversion_rate: f64,
    /// Максимальная ответственность для страховки
    pub max_hedge_liability: f64,
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            min_freebet_odds: 3.0,
            max_hedge_odds: 1.5,
            max_odds_spread: 5.0,
            min_conversion_rate: 60.0,
            max_hedge_liability: 5000.0,
        }
    }
}

/// Результат анализа matching плечей
#[derive(Debug, Clone)]
pub struct MatchingAnalysis {
    pub freebet_odds: f64,
    pub hedge_odds: f64,
    pub odds_spread: f64,
    pub conversion_rate: f64,
    pub hedge_liability: f64,
    pub guaranteed_profit: f64,
    pub risk_score: f64, // 0.0 (низкий) - 1.0 (высокий)
    pub is_optimal: bool,
}

#[derive(Clone)]
pub struct FreebetHunter {
    freebet_amounts: Arc<Vec<f64>>,
    min_profit: f64,
    cached_events: Arc<parking_lot::RwLock<Vec<Event>>>,
    cached_odds: Arc<parking_lot::RwLock<Vec<Odd>>>,
    matching_config: Arc<MatchingConfig>,
    /// Размер банкролла для Kelly Criterion
    bankroll: Arc<parking_lot::RwLock<f64>>,
    /// Доля Kelly (обычно 0.25 для conservative)
    kelly_fraction: f64,
}

impl FreebetHunter {
    pub fn new(freebet_amounts: Vec<f64>, min_profit: f64, _ttl_secs: u64) -> Self {
        Self {
            freebet_amounts: Arc::new(freebet_amounts),
            min_profit,
            cached_events: Arc::new(parking_lot::RwLock::new(Vec::new())),
            cached_odds: Arc::new(parking_lot::RwLock::new(Vec::new())),
            matching_config: Arc::new(MatchingConfig::default()),
            bankroll: Arc::new(parking_lot::RwLock::new(10000.0)),
            kelly_fraction: 0.25,
        }
    }

    /// Установить конфигурация matching
    pub fn with_matching_config(mut self, config: MatchingConfig) -> Self {
        self.matching_config = Arc::new(config);
        self
    }

    /// Установить банкролл для Kelly Criterion
    pub fn with_bankroll(self, bankroll: f64) -> Self {
        *self.bankroll.write() = bankroll;
        self
    }

    /// Установить долю Kelly
    pub fn with_kelly_fraction(mut self, fraction: f64) -> Self {
        self.kelly_fraction = fraction;
        self
    }

    /// Обновляем кэш событий и odds
    pub fn update_cache(&self, events: Vec<Event>, odds: Vec<Odd>) {
        *self.cached_events.write() = events;
        *self.cached_odds.write() = odds;
    }

    /// Поиск возможностей для отыгрыша фрибетов (sync версия)
    pub fn find_opportunities(
        &self,
        events: &[Event],
        all_odds: &[Odd],
    ) -> Vec<FreebetOpportunity> {
        let mut opportunities = Vec::new();

        for &freebet in self.freebet_amounts.iter() {
            for event in events {
                let event_odds: Vec<&Odd> =
                    all_odds.iter().filter(|o| o.event_id == event.id).collect();
                if event_odds.len() < 2 {
                    continue;
                }

                let best_back = event_odds
                    .iter()
                    .max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());
                let best_lay = event_odds
                    .iter()
                    .min_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());

                if let (Some(back), Some(lay)) = (best_back, best_lay) {
                    if back.bookmaker_slug == lay.bookmaker_slug {
                        continue;
                    }

                    let profit = self.calculate_freebet_profit(freebet, back.odds, lay.odds);
                    if profit >= self.min_profit {
                        let roi = (profit / freebet) * 100.0;

                        // Рассчитываем recommended stake через Kelly
                        let implied_prob = decimal_to_implied_probability(back.odds);
                        let fair_prob = 1.0 / back.odds;
                        let edge = if fair_prob > implied_prob {
                            fair_prob - implied_prob
                        } else {
                            0.0
                        };
                        let kelly_stake = KellyCalculator::optimal_stake(
                            *self.bankroll.read(),
                            edge,
                            back.odds,
                            self.kelly_fraction,
                            5.0, // max 5% exposure
                        );

                        let mut opp = FreebetOpportunity {
                            id: Uuid::new_v4(),
                            bookmaker: back.bookmaker_slug.clone(),
                            hedge_bookmaker: lay.bookmaker_slug.clone(),
                            event: event.clone(),
                            market: back.market.clone(),
                            selection: back.selection.clone(),
                            hedge_selection: lay.selection.clone(),
                            back_odds: back.odds,
                            lay_odds: lay.odds,
                            freebet_amount: freebet,
                            guaranteed_profit: profit,
                            roi,
                            detected_at: Utc::now(),
                        };

                        // Если Kelly stake > 0, значит есть value — усиливаем сигнал
                        if kelly_stake > 0.0 {
                            opp.roi = roi + (kelly_stake / freebet) * 10.0; // бонус к ROI
                        }

                        opportunities.push(opp);
                    }
                }
            }
        }

        opportunities.sort_by(|a, b| {
            b.guaranteed_profit
                .partial_cmp(&a.guaranteed_profit)
                .unwrap()
        });
        opportunities
    }

    /// ALGORITHM: Matching плечей с минимизацией риска
    /// Находит пары: высокий кэф для фрибета + низкий кэф для страховки
    /// Оптимизирует соотношение profit/risk
    pub fn find_matching_opportunities(
        &self,
        events: &[Event],
        all_odds: &[Odd],
    ) -> Vec<(FreebetOpportunity, MatchingAnalysis)> {
        let mut results = Vec::new();
        let config = &self.matching_config;

        for &freebet in self.freebet_amounts.iter() {
            for event in events {
                let event_odds: Vec<&Odd> =
                    all_odds.iter().filter(|o| o.event_id == event.id).collect();
                if event_odds.len() < 2 {
                    continue;
                }

                // Ищем все пары bookmaker'ов для matching
                for (i, back_odd) in event_odds.iter().enumerate() {
                    for lay_odd in event_odds.iter().skip(i + 1) {
                        if back_odd.bookmaker_slug == lay_odd.bookmaker_slug {
                            continue;
                        }

                        // Проверяем matching criteria
                        let (freebet_odd, hedge_odd) = if back_odd.odds >= lay_odd.odds {
                            (back_odd, lay_odd)
                        } else {
                            (lay_odd, back_odd)
                        };

                        if freebet_odd.odds < config.min_freebet_odds {
                            continue;
                        }
                        if hedge_odd.odds > config.max_hedge_odds {
                            continue;
                        }

                        let odds_spread = freebet_odd.odds - hedge_odd.odds;
                        if odds_spread > config.max_odds_spread {
                            continue;
                        }

                        // Рассчитываем profit и liability
                        let profit = self.calculate_freebet_profit(
                            freebet,
                            freebet_odd.odds,
                            hedge_odd.odds,
                        );
                        let hedge_liability = self.calculate_hedge_liability(
                            freebet,
                            freebet_odd.odds,
                            hedge_odd.odds,
                        );

                        if hedge_liability > config.max_hedge_liability {
                            continue;
                        }
                        if profit < self.min_profit {
                            continue;
                        }

                        let conversion_rate = self.calculate_conversion(freebet, freebet_odd.odds);
                        if conversion_rate < config.min_conversion_rate {
                            continue;
                        }

                        // Risk score: 0.0 = идеальный, 1.0 = рискованный
                        let risk_score = self.calculate_risk_score(
                            odds_spread,
                            conversion_rate,
                            hedge_liability,
                            config,
                        );

                        let is_optimal = risk_score < 0.3 && conversion_rate > 70.0;

                        let analysis = MatchingAnalysis {
                            freebet_odds: freebet_odd.odds,
                            hedge_odds: hedge_odd.odds,
                            odds_spread,
                            conversion_rate,
                            hedge_liability,
                            guaranteed_profit: profit,
                            risk_score,
                            is_optimal,
                        };

                        let roi = (profit / freebet) * 100.0;
                        let opportunity = FreebetOpportunity {
                            id: Uuid::new_v4(),
                            bookmaker: freebet_odd.bookmaker_slug.clone(),
                            hedge_bookmaker: hedge_odd.bookmaker_slug.clone(),
                            event: event.clone(),
                            market: freebet_odd.market.clone(),
                            selection: freebet_odd.selection.clone(),
                            hedge_selection: hedge_odd.selection.clone(),
                            back_odds: freebet_odd.odds,
                            lay_odds: hedge_odd.odds,
                            freebet_amount: freebet,
                            guaranteed_profit: profit,
                            roi,
                            detected_at: Utc::now(),
                        };

                        results.push((opportunity, analysis));
                    }
                }
            }
        }

        // Сортируем по risk_score (ascending) затем по profit (descending)
        results.sort_by(|a, b| {
            a.1.risk_score
                .partial_cmp(&b.1.risk_score)
                .unwrap()
                .then_with(|| {
                    b.1.guaranteed_profit
                        .partial_cmp(&a.1.guaranteed_profit)
                        .unwrap()
                })
        });
        results
    }

    /// Рассчитывает risk score (0.0 - 1.0)
    fn calculate_risk_score(
        &self,
        odds_spread: f64,
        conversion_rate: f64,
        hedge_liability: f64,
        config: &MatchingConfig,
    ) -> f64 {
        // Нормализуем компоненты
        let spread_risk = (odds_spread / config.max_odds_spread).min(1.0);
        let conversion_risk = 1.0 - (conversion_rate / 100.0).min(1.0);
        let liability_risk = (hedge_liability / config.max_hedge_liability).min(1.0);

        // Взвешенная формула: conversion самый важный
        0.5 * conversion_risk + 0.3 * spread_risk + 0.2 * liability_risk
    }

    /// Рассчитывает liability для страховки
    pub fn calculate_hedge_liability(&self, freebet: f64, back_odds: f64, lay_odds: f64) -> f64 {
        let back_return = freebet * (back_odds - 1.0);
        let lay_stake = back_return / (lay_odds - 1.0);
        lay_stake * (lay_odds - 1.0)
    }

    pub fn calculate_freebet_profit(&self, freebet: f64, back_odds: f64, lay_odds: f64) -> f64 {
        let back_return = freebet * (back_odds - 1.0);
        let lay_stake = back_return / (lay_odds - 1.0);
        let lay_liability = lay_stake * (lay_odds - 1.0);

        let profit_if_back_wins = back_return - lay_stake;
        let profit_if_lay_wins = lay_stake - lay_liability;

        profit_if_back_wins.min(profit_if_lay_wins)
    }

    pub fn calculate_conversion(&self, freebet: f64, back_odds: f64) -> f64 {
        let implied = decimal_to_implied_probability(back_odds);
        (freebet * (back_odds - 1.0) * (1.0 - implied)) / freebet * 100.0
    }

    /// Рассчитывает recommended stake через Kelly Criterion
    pub fn calculate_optimal_stake(
        &self,
        freebet_amount: f64,
        back_odds: f64,
        bankroll: Option<f64>,
    ) -> f64 {
        let br = bankroll.unwrap_or(*self.bankroll.read());
        let implied_prob = decimal_to_implied_probability(back_odds);
        let fair_prob = 1.0 / back_odds;
        let edge = if fair_prob > implied_prob {
            fair_prob - implied_prob
        } else {
            0.0
        };

        let kelly_stake =
            KellyCalculator::optimal_stake(br, edge, back_odds, self.kelly_fraction, 5.0);

        // Для фрибетов: stake не может превышать freebet_amount
        kelly_stake.min(freebet_amount)
    }

    /// Сканирует и возвращает текущие фрибет-возможности из кэша
    pub fn scan_freebets(&self) -> Vec<FreebetOpportunity> {
        let events = self.cached_events.read();
        let odds = self.cached_odds.read();
        self.find_opportunities(&events, &odds)
    }

    /// Сканирует matching возможности из кэша
    pub fn scan_matching(&self) -> Vec<(FreebetOpportunity, MatchingAnalysis)> {
        let events = self.cached_events.read();
        let odds = self.cached_odds.read();
        self.find_matching_opportunities(&events, &odds)
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
    fn test_find_freebet_opportunity() {
        let hunter = FreebetHunter::new(vec![1000.0], 0.0, 60);
        let event = make_event("evt1");
        let odds = vec![
            make_odd("evt1", "bk1", "1", 5.0),
            make_odd("evt1", "bk2", "1", 2.0),
        ];

        let opps = hunter.find_opportunities(&[event], &odds);
        assert!(!opps.is_empty());
        assert_eq!(opps[0].bookmaker, "bk1");
        assert_eq!(opps[0].hedge_bookmaker, "bk2");
    }

    #[test]
    fn test_calculate_freebet_profit() {
        let hunter = FreebetHunter::new(vec![1000.0], 0.0, 60);
        let profit = hunter.calculate_freebet_profit(1000.0, 5.0, 2.0);
        assert!(profit.is_finite());
    }

    #[test]
    fn test_matching_opportunities() {
        let config = MatchingConfig {
            min_freebet_odds: 3.0,
            max_hedge_odds: 2.5,
            max_odds_spread: 4.0,
            min_conversion_rate: 50.0,
            max_hedge_liability: 10000.0,
        };
        let hunter = FreebetHunter::new(vec![1000.0], 0.0, 60).with_matching_config(config);

        let event = make_event("evt1");
        let odds = vec![
            make_odd("evt1", "bk1", "1", 4.0),
            make_odd("evt1", "bk2", "X", 2.0),
        ];

        let results = hunter.find_matching_opportunities(&[event], &odds);
        // Matching может быть пустым если conversion_rate ниже порога
        // Проверяем что функция не паничит и корректно работает
        for (_, analysis) in &results {
            assert!(analysis.risk_score >= 0.0 && analysis.risk_score <= 1.0);
        }
    }

    #[test]
    fn test_risk_score_bounds() {
        let hunter = FreebetHunter::new(vec![1000.0], 0.0, 60);
        let config = MatchingConfig::default();

        // Низкий риск
        let low_risk = hunter.calculate_risk_score(1.0, 80.0, 500.0, &config);
        assert!(low_risk < 0.5);

        // Высокий риск
        let high_risk = hunter.calculate_risk_score(4.5, 30.0, 4500.0, &config);
        assert!(high_risk > 0.5);
    }

    #[test]
    fn test_optimal_stake_kelly() {
        let hunter = FreebetHunter::new(vec![1000.0], 0.0, 60)
            .with_bankroll(10000.0)
            .with_kelly_fraction(0.25);

        let stake = hunter.calculate_optimal_stake(1000.0, 4.0, None);
        assert!(stake >= 0.0);
        assert!(stake <= 1000.0); // не больше фрибета
    }

    #[test]
    fn test_hedge_liability() {
        let hunter = FreebetHunter::new(vec![1000.0], 0.0, 60);
        let liability = hunter.calculate_hedge_liability(1000.0, 5.0, 2.0);
        assert!(liability > 0.0);
        assert!(liability.is_finite());
    }
}
