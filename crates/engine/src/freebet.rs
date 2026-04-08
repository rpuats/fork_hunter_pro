use chrono::Utc;
use moka::future::Cache;
use shared::odds::decimal_to_implied_probability;
use shared::{Event, FreebetOpportunity, Odd};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct FreebetHunter {
    freebet_amounts: Arc<Vec<f64>>,
    min_profit: f64,
    #[allow(dead_code)]
    cache: Cache<String, Vec<FreebetOpportunity>>,
}

impl FreebetHunter {
    pub fn new(freebet_amounts: Vec<f64>, min_profit: f64, ttl_secs: u64) -> Self {
        Self {
            freebet_amounts: Arc::new(freebet_amounts),
            min_profit,
            cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(ttl_secs))
                .max_capacity(1000)
                .build(),
        }
    }

    pub async fn find_opportunities(&self, events: &[Event], all_odds: &[Odd]) -> Vec<FreebetOpportunity> {
        let mut opportunities = Vec::new();

        for &freebet in self.freebet_amounts.iter() {
            for event in events {
                let event_odds: Vec<&Odd> = all_odds.iter().filter(|o| o.event_id == event.id).collect();
                if event_odds.len() < 2 {
                    continue;
                }

                let best_back = event_odds.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());
                let best_lay = event_odds.iter().min_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());

                if let (Some(back), Some(lay)) = (best_back, best_lay) {
                    if back.bookmaker_slug == lay.bookmaker_slug {
                        continue;
                    }

                    let profit = self.calculate_freebet_profit(freebet, back.odds, lay.odds);
                    if profit >= self.min_profit {
                        let roi = (profit / freebet) * 100.0;
                        opportunities.push(FreebetOpportunity {
                            id: Uuid::new_v4(),
                            bookmaker: back.bookmaker_slug.clone(),
                            event: event.clone(),
                            back_odds: back.odds,
                            lay_odds: lay.odds,
                            freebet_amount: freebet,
                            guaranteed_profit: profit,
                            roi,
                            detected_at: Utc::now(),
                        });
                    }
                }
            }
        }

        opportunities.sort_by(|a, b| b.guaranteed_profit.partial_cmp(&a.guaranteed_profit).unwrap());
        opportunities
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

    /// Сканирует и возвращает текущие фрибет-возможности (синхронный метод для API).
    /// В реальной реализации здесь должен быть доступ к данным сканера.
    pub fn scan_freebets(&self) -> Vec<FreebetOpportunity> {
        // Заглушка — реальная реализация требует доступа к событиям и коэффициентам
        // из основного сканера. Для API endpoint возвращаем кэш если есть.
        Vec::new()
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

    #[tokio::test]
    async fn test_find_freebet_opportunity() {
        let hunter = FreebetHunter::new(vec![1000.0], 0.0, 60);
        let event = make_event("evt1");
        let odds = vec![
            make_odd("evt1", "bk1", "1", 5.0),
            make_odd("evt1", "bk2", "1", 2.0),
        ];

        let opps = hunter.find_opportunities(&[event], &odds).await;
        assert!(!opps.is_empty());
    }

    #[test]
    fn test_calculate_freebet_profit() {
        let hunter = FreebetHunter::new(vec![1000.0], 0.0, 60);
        let profit = hunter.calculate_freebet_profit(1000.0, 5.0, 2.0);
        assert!(profit.is_finite());
    }
}
