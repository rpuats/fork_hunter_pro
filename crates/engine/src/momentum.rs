use chrono::{DateTime, Utc};
use dashmap::DashMap;
use shared::odds::calculate_surebet_profit;
use shared::{Event, Odd, Surebet, SurebetLeg};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct MomentumScanner {
    min_profit: f64,
    default_stake: f64,
    live_events: Arc<DashMap<String, LiveEventState>>,
}

#[derive(Debug, Clone)]
struct LiveEventState {
    event: Event,
    last_update: DateTime<Utc>,
    odds_history: Vec<Vec<Odd>>,
    momentum_score: f64,
}

impl MomentumScanner {
    pub fn new(min_profit: f64, default_stake: f64) -> Self {
        Self {
            min_profit,
            default_stake,
            live_events: Arc::new(DashMap::new()),
        }
    }

    pub fn track_event(&self, event: Event, odds: Vec<Odd>) {
        let key = event.id.clone();
        let mut entry = self.live_events.entry(key).or_insert(LiveEventState {
            event,
            last_update: Utc::now(),
            odds_history: Vec::new(),
            momentum_score: 0.0,
        });

        entry.odds_history.push(odds);
        let len = entry.odds_history.len();
        if len > 50 {
            let drain_to = len - 50;
            entry.odds_history.drain(..drain_to);
        }
        entry.last_update = Utc::now();
        entry.momentum_score = Self::calc_momentum(&entry.odds_history);
    }

    pub fn detect_momentum_surebets(&self) -> Vec<Surebet> {
        let mut surebets = Vec::new();

        for entry in self.live_events.iter() {
            let state = entry.value();
            if state.momentum_score < 0.5 {
                continue;
            }

            if let Some(latest_odds) = state.odds_history.last() {
                if let Some(surebet) = Self::find_quick_surebet(
                    &state.event,
                    latest_odds,
                    self.min_profit,
                    self.default_stake,
                ) {
                    surebets.push(surebet);
                }
            }
        }

        surebets
    }

    pub fn get_high_momentum_events(&self, threshold: f64) -> Vec<Event> {
        self.live_events
            .iter()
            .filter(|e| e.value().momentum_score >= threshold)
            .map(|e| e.value().event.clone())
            .collect()
    }

    pub fn cleanup_stale(&self, max_age_secs: i64) {
        let cutoff = Utc::now() - chrono::Duration::seconds(max_age_secs);
        self.live_events
            .retain(|_, state| state.last_update >= cutoff);
    }

    pub fn stats(&self) -> MomentumStats {
        let total = self.live_events.len();
        let high_momentum = self
            .live_events
            .iter()
            .filter(|e| e.value().momentum_score >= 0.7)
            .count();
        MomentumStats {
            total_tracked: total,
            high_momentum,
        }
    }

    fn calc_momentum(history: &[Vec<Odd>]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }
        let mut total_change = 0.0_f64;
        let mut change_count = 0_u64;

        for window in history.windows(2) {
            let prev = &window[0];
            let curr = &window[1];
            for p in prev {
                if let Some(c) = curr
                    .iter()
                    .find(|o| o.selection == p.selection && o.market == p.market)
                {
                    let change = (c.odds - p.odds).abs() / p.odds;
                    total_change += change;
                    change_count += 1;
                }
            }
        }

        if change_count == 0 {
            return 0.0;
        }
        (total_change / change_count as f64 * 10.0).min(1.0)
    }

    fn find_quick_surebet(
        event: &Event,
        odds: &[Odd],
        min_profit: f64,
        default_stake: f64,
    ) -> Option<Surebet> {
        let by_market: HashMap<String, Vec<&Odd>> = {
            let mut m: HashMap<String, Vec<&Odd>> = HashMap::new();
            for odd in odds {
                let key = format!("{}|{}", odd.market, odd.selection);
                m.entry(key).or_default().push(odd);
            }
            m
        };

        for (_market, market_odds) in &by_market {
            let mut seen = std::collections::HashSet::new();
            let best: Vec<&Odd> = market_odds
                .iter()
                .filter(|o| seen.insert(&o.bookmaker_slug))
                .cloned()
                .collect();

            if best.len() >= 2 {
                let odds_values: Vec<f64> = best.iter().map(|o| o.odds).collect();
                if let Some(profit) = calculate_surebet_profit(&odds_values) {
                    if profit >= min_profit {
                        let stakes = shared::odds::calculate_stakes(&odds_values, default_stake);
                        let payout = stakes[0] * best[0].odds;
                        return Some(Surebet {
                            id: Uuid::new_v4(),
                            sport: event.sport.clone(),
                            league: event.league.clone(),
                            home_team: event.home_team.clone(),
                            away_team: event.away_team.clone(),
                            start_time: event.start_time,
                            is_live: event.is_live,
                            profit_percent: profit,
                            total_stake: default_stake,
                            legs: best
                                .iter()
                                .zip(stakes.iter())
                                .map(|(odd, &stake)| SurebetLeg {
                                    bookmaker: odd.bookmaker_slug.clone(),
                                    market: odd.market.clone(),
                                    selection: odd.selection.clone(),
                                    odds: odd.odds,
                                    line: odd.line,
                                    stake,
                                    payout,
                                    url: None,
                                })
                                .collect(),
                            detected_at: Utc::now(),
                            verified: false,
                            mirror: false,
                        });
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct MomentumStats {
    pub total_tracked: usize,
    pub high_momentum: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;
    use shared::Sport;
    use std::collections::HashMap;

    fn make_event(id: &str, live: bool) -> Event {
        Event {
            id: id.into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: live,
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
    fn test_track_event() {
        let scanner = MomentumScanner::new(1.0, 1000.0);
        let event = make_event("evt1", true);
        scanner.track_event(event, vec![make_odd("evt1", "bk1", "1", 2.0)]);
        assert_eq!(scanner.stats().total_tracked, 1);
    }

    #[test]
    fn test_cleanup_stale() {
        let scanner = MomentumScanner::new(1.0, 1000.0);
        let event = make_event("evt1", true);
        scanner.track_event(event, vec![]);
        scanner.cleanup_stale(0);
        assert_eq!(scanner.stats().total_tracked, 0);
    }
}
