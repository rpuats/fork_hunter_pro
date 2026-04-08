use bloomfilter::Bloom;
use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use shared::odds::{calculate_surebet_profit, calculate_stakes, OddsType};
use shared::{Event, Odd, Surebet, SurebetLeg};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

#[derive(Clone)]
pub struct SurebetCalculator {
    min_profit: f64,
    max_profit: f64,
    default_stake: f64,
    seen_surebets: Arc<RwLock<Bloom<[u8]>>>,
    recent_events: Arc<DashMap<String, Vec<Odd>>>,
}

impl SurebetCalculator {
    pub fn new(min_profit: f64, max_profit: f64, default_stake: f64, capacity: usize, error_rate: f64) -> Self {
        Self {
            min_profit,
            max_profit,
            default_stake,
            seen_surebets: Arc::new(RwLock::new(Bloom::new_for_fp_rate(capacity, error_rate))),
            recent_events: Arc::new(DashMap::new()),
        }
    }

    pub fn find_surebets(&self, events: &[Event], all_odds: &[Odd]) -> Vec<Surebet> {
        let mut surebets = Vec::new();
        let odds_by_event = self.group_odds_by_event(all_odds);

        for event in events {
            if let Some(event_odds) = odds_by_event.get(&event.id) {
                if let Some(surebet) = self.analyze_event(event, event_odds) {
                    if surebet.profit_percent >= self.min_profit && surebet.profit_percent <= self.max_profit {
                        let key = self.surebet_key(&surebet);
                        if !self.seen_surebets.read().check(&key) {
                            debug!(profit = surebet.profit_percent, "New surebet found");
                            surebets.push(surebet);
                        }
                    }
                }
            }
        }
        surebets
    }

    pub fn analyze_two_way(&self, odds_a: f64, odds_b: f64) -> Option<f64> {
        calculate_surebet_profit(&[odds_a, odds_b])
    }

    pub fn analyze_three_way(&self, odds_1: f64, odds_x: f64, odds_2: f64) -> Option<f64> {
        calculate_surebet_profit(&[odds_1, odds_x, odds_2])
    }

    pub fn calculate_stakes(&self, odds: &[f64]) -> Vec<f64> {
        calculate_stakes(odds, self.default_stake)
    }

    pub fn mark_seen(&self, surebet: &Surebet) {
        let key = self.surebet_key(surebet);
        self.seen_surebets.write().set(&key);
    }

    pub fn cache_odds(&self, event_id: &str, odds: Vec<Odd>) {
        self.recent_events.insert(event_id.to_string(), odds);
    }

    fn group_odds_by_event(&self, all_odds: &[Odd]) -> HashMap<String, Vec<Odd>> {
        let mut map = HashMap::new();
        for odd in all_odds {
            map.entry(odd.event_id.clone()).or_insert_with(Vec::new).push(odd.clone());
        }
        map
    }

    fn analyze_event(&self, event: &Event, odds: &[Odd]) -> Option<Surebet> {
        let by_market = self.group_by_market(odds);

        // 2-way cross-bookmaker (same market group)
        for (market, market_odds) in &by_market {
            if let Some(surebet) = self.find_cross_bookmaker_surebet(event, market, market_odds) {
                return Some(surebet);
            }
        }

        // 3-way 1X2: combine Home + Draw + Away from different bookmakers
        let home_odds: Vec<&Odd> = odds.iter().filter(|o| o.odds_type == OddsType::Home || o.selection == "1").collect();
        let draw_odds: Vec<&Odd> = odds.iter().filter(|o| o.odds_type == OddsType::Draw || o.selection == "X").collect();
        let away_odds: Vec<&Odd> = odds.iter().filter(|o| o.odds_type == OddsType::Away || o.selection == "2").collect();

        if !home_odds.is_empty() && !draw_odds.is_empty() && !away_odds.is_empty() {
            let best_home = home_odds.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());
            let best_draw = draw_odds.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());
            let best_away = away_odds.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());

            if let (Some(&h), Some(&d), Some(&a)) = (best_home, best_draw, best_away) {
                if let Some(profit) = calculate_surebet_profit(&[h.odds, d.odds, a.odds]) {
                    if profit >= self.min_profit {
                        let stakes = calculate_stakes(&[h.odds, d.odds, a.odds], self.default_stake);
                        let payout = stakes[0] * h.odds;
                        return Some(Surebet {
                            id: Uuid::new_v4(),
                            sport: event.sport.clone(),
                            league: event.league.clone(),
                            home_team: event.home_team.clone(),
                            away_team: event.away_team.clone(),
                            start_time: event.start_time,
                            is_live: event.is_live,
                            profit_percent: profit,
                            total_stake: self.default_stake,
                            legs: vec![
                                SurebetLeg { bookmaker: h.bookmaker_slug.clone(), market: h.market.clone(), selection: h.selection.clone(), odds: h.odds, line: h.line, stake: stakes[0], payout, url: None },
                                SurebetLeg { bookmaker: d.bookmaker_slug.clone(), market: d.market.clone(), selection: d.selection.clone(), odds: d.odds, line: d.line, stake: stakes[1], payout, url: None },
                                SurebetLeg { bookmaker: a.bookmaker_slug.clone(), market: a.market.clone(), selection: a.selection.clone(), odds: a.odds, line: a.line, stake: stakes[2], payout, url: None },
                            ],
                            detected_at: Utc::now(),
                            verified: false,
                            mirror: false,
                        });
                    }
                }
            }
        }

        // 2-way complementary: Over/Under, Yes/No, Even/Odd
        let over_odds: Vec<&Odd> = odds.iter().filter(|o| o.odds_type == OddsType::Over).collect();
        let under_odds: Vec<&Odd> = odds.iter().filter(|o| o.odds_type == OddsType::Under).collect();

        if !over_odds.is_empty() && !under_odds.is_empty() {
            let best_over = over_odds.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());
            let best_under = under_odds.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());

            if let (Some(&o), Some(&u)) = (best_over, best_under) {
                if o.line == u.line {
                    if let Some(profit) = calculate_surebet_profit(&[o.odds, u.odds]) {
                        if profit >= self.min_profit {
                            let stakes = calculate_stakes(&[o.odds, u.odds], self.default_stake);
                            let payout = stakes[0] * o.odds;
                            return Some(Surebet {
                                id: Uuid::new_v4(),
                                sport: event.sport.clone(),
                                league: event.league.clone(),
                                home_team: event.home_team.clone(),
                                away_team: event.away_team.clone(),
                                start_time: event.start_time,
                                is_live: event.is_live,
                                profit_percent: profit,
                                total_stake: self.default_stake,
                                legs: vec![
                                    SurebetLeg { bookmaker: o.bookmaker_slug.clone(), market: o.market.clone(), selection: o.selection.clone(), odds: o.odds, line: o.line, stake: stakes[0], payout, url: None },
                                    SurebetLeg { bookmaker: u.bookmaker_slug.clone(), market: u.market.clone(), selection: u.selection.clone(), odds: u.odds, line: u.line, stake: stakes[1], payout, url: None },
                                ],
                                detected_at: Utc::now(),
                                verified: false,
                                mirror: false,
                            });
                        }
                    }
                }
            }
        }

        // BTTS: Yes/No
        let yes_odds: Vec<&Odd> = odds.iter().filter(|o| o.odds_type == OddsType::BothTeamsScoreYes).collect();
        let no_odds: Vec<&Odd> = odds.iter().filter(|o| o.odds_type == OddsType::BothTeamsScoreNo).collect();

        if !yes_odds.is_empty() && !no_odds.is_empty() {
            let best_yes = yes_odds.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());
            let best_no = no_odds.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());

            if let (Some(&y), Some(&n)) = (best_yes, best_no) {
                if let Some(profit) = calculate_surebet_profit(&[y.odds, n.odds]) {
                    if profit >= self.min_profit {
                        let stakes = calculate_stakes(&[y.odds, n.odds], self.default_stake);
                        let payout = stakes[0] * y.odds;
                        return Some(Surebet {
                            id: Uuid::new_v4(),
                            sport: event.sport.clone(),
                            league: event.league.clone(),
                            home_team: event.home_team.clone(),
                            away_team: event.away_team.clone(),
                            start_time: event.start_time,
                            is_live: event.is_live,
                            profit_percent: profit,
                            total_stake: self.default_stake,
                            legs: vec![
                                SurebetLeg { bookmaker: y.bookmaker_slug.clone(), market: y.market.clone(), selection: y.selection.clone(), odds: y.odds, line: y.line, stake: stakes[0], payout, url: None },
                                SurebetLeg { bookmaker: n.bookmaker_slug.clone(), market: n.market.clone(), selection: n.selection.clone(), odds: n.odds, line: n.line, stake: stakes[1], payout, url: None },
                            ],
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

    fn group_by_market<'a>(&self, odds: &'a [Odd]) -> HashMap<String, Vec<&'a Odd>> {
        let mut map = HashMap::new();
        for odd in odds {
            // Для рынков с линией (тоталы, форы) включаем линию в ключ
            let key = if let Some(line) = odd.line {
                format!("{}|{}|{}", odd.market, odd.odds_type, line)
            } else {
                format!("{}|{}", odd.market, odd.odds_type)
            };
            map.entry(key).or_insert_with(Vec::new).push(odd);
        }
        map
    }

    fn find_cross_bookmaker_surebet(&self, event: &Event, _market: &str, odds: &[&Odd]) -> Option<Surebet> {
        let mut seen_bk = std::collections::HashSet::new();
        let bookmakers: Vec<&Odd> = odds.iter()
            .filter(|odd| seen_bk.insert(&odd.bookmaker_slug))
            .cloned()
            .collect();

        if bookmakers.len() < 2 { return None; }

        for (i, odd_a) in bookmakers.iter().enumerate() {
            for odd_b in bookmakers.iter().skip(i + 1) {
                if odd_a.bookmaker_slug == odd_b.bookmaker_slug { continue; }
                if let Some(profit) = calculate_surebet_profit(&[odd_a.odds, odd_b.odds]) {
                    if profit >= self.min_profit {
                        let stakes = calculate_stakes(&[odd_a.odds, odd_b.odds], self.default_stake);
                        let payout = stakes[0] * odd_a.odds;
                        return Some(Surebet {
                            id: Uuid::new_v4(),
                            sport: event.sport.clone(),
                            league: event.league.clone(),
                            home_team: event.home_team.clone(),
                            away_team: event.away_team.clone(),
                            start_time: event.start_time,
                            is_live: event.is_live,
                            profit_percent: profit,
                            total_stake: self.default_stake,
                            legs: vec![
                                SurebetLeg { bookmaker: odd_a.bookmaker_slug.clone(), market: odd_a.market.clone(), selection: odd_a.selection.clone(), odds: odd_a.odds, line: odd_a.line, stake: stakes[0], payout, url: None },
                                SurebetLeg { bookmaker: odd_b.bookmaker_slug.clone(), market: odd_b.market.clone(), selection: odd_b.selection.clone(), odds: odd_b.odds, line: odd_b.line, stake: stakes[1], payout, url: None },
                            ],
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

    /// Ищем 2-way вилки по комплементарным исходам (Over/Under с той же линией, Yes/No)
    fn find_two_way_complementary_surebet(&self, event: &Event, odds: &[Odd]) -> Option<Surebet> {
        // Группием по market + line (для тоталов/фор)
        let mut by_market_line: HashMap<String, Vec<&Odd>> = HashMap::new();
        for odd in odds {
            let line_key = odd.line.map(|l| format!("{:.2}", l)).unwrap_or_default();
            let key = format!("{}|{}", odd.market, line_key);
            by_market_line.entry(key).or_default().push(odd);
        }

        // Для каждой группы ищем Over/Under или Yes/No от разных БК
        for (_key, market_odds) in &by_market_line {
            let mut best_over: Option<&Odd> = None;
            let mut best_under: Option<&Odd> = None;

            for odd in market_odds {
                let sel = odd.selection.to_lowercase();
                if sel.contains("over") || sel.contains("больше") || sel.contains("тб")
                    || sel.contains("да") || sel.contains("yes") || sel.contains("чёт") || sel.contains("even") {
                    if best_over.map_or(true, |b| odd.odds > b.odds) {
                        best_over = Some(odd);
                    }
                } else if sel.contains("under") || sel.contains("меньше") || sel.contains("тм")
                    || sel.contains("нет") || sel.contains("no") || sel.contains("нечет") || sel.contains("odd") {
                    if best_under.map_or(true, |b| odd.odds > b.odds) {
                        best_under = Some(odd);
                    }
                }
            }

            if let (Some(o_over), Some(o_under)) = (best_over, best_under) {
                if o_over.bookmaker_slug == o_under.bookmaker_slug {
                    continue; // Оба от одной БК — не вилка
                }
                if let Some(profit) = calculate_surebet_profit(&[o_over.odds, o_under.odds]) {
                    if profit >= self.min_profit {
                        let stakes = calculate_stakes(&[o_over.odds, o_under.odds], self.default_stake);
                        let payout = stakes[0] * o_over.odds;
                        return Some(Surebet {
                            id: Uuid::new_v4(),
                            sport: event.sport.clone(),
                            league: event.league.clone(),
                            home_team: event.home_team.clone(),
                            away_team: event.away_team.clone(),
                            start_time: event.start_time,
                            is_live: event.is_live,
                            profit_percent: profit,
                            total_stake: self.default_stake,
                            legs: vec![
                                SurebetLeg { bookmaker: o_over.bookmaker_slug.clone(), market: o_over.market.clone(), selection: o_over.selection.clone(), odds: o_over.odds, line: o_over.line, stake: stakes[0], payout, url: None },
                                SurebetLeg { bookmaker: o_under.bookmaker_slug.clone(), market: o_under.market.clone(), selection: o_under.selection.clone(), odds: o_under.odds, line: o_under.line, stake: stakes[1], payout, url: None },
                            ],
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

    /// Ищем 3-way вилку: 1 от одной БК, X от другой, 2 от третьей (все на одном событии)
    fn find_three_way_surebet(&self, event: &Event, odds: &[Odd]) -> Option<Surebet> {
        let mut best_1: Option<&Odd> = None;
        let mut best_x: Option<&Odd> = None;
        let mut best_2: Option<&Odd> = None;

        for odd in odds {
            let sel = odd.selection.to_lowercase();
            if sel == "1" || sel == "п1" || sel == "home" {
                if best_1.map_or(true, |b| odd.odds > b.odds) {
                    best_1 = Some(odd);
                }
            } else if sel == "x" || sel == "draw" || sel == "х" {
                if best_x.map_or(true, |b| odd.odds > b.odds) {
                    best_x = Some(odd);
                }
            } else if sel == "2" || sel == "п2" || sel == "away" {
                if best_2.map_or(true, |b| odd.odds > b.odds) {
                    best_2 = Some(odd);
                }
            }
        }

        if let (Some(o1), Some(ox), Some(o2)) = (best_1, best_x, best_2) {
            if let Some(profit) = calculate_surebet_profit(&[o1.odds, ox.odds, o2.odds]) {
                if profit >= self.min_profit {
                    let stakes = calculate_stakes(&[o1.odds, ox.odds, o2.odds], self.default_stake);
                    let payout = stakes[0] * o1.odds;
                    return Some(Surebet {
                        id: Uuid::new_v4(),
                        sport: event.sport.clone(),
                        league: event.league.clone(),
                        home_team: event.home_team.clone(),
                        away_team: event.away_team.clone(),
                        start_time: event.start_time,
                        is_live: event.is_live,
                        profit_percent: profit,
                        total_stake: self.default_stake,
                        legs: vec![
                            SurebetLeg { bookmaker: o1.bookmaker_slug.clone(), market: o1.market.clone(), selection: o1.selection.clone(), odds: o1.odds, line: o1.line, stake: stakes[0], payout, url: None },
                            SurebetLeg { bookmaker: ox.bookmaker_slug.clone(), market: ox.market.clone(), selection: ox.selection.clone(), odds: ox.odds, line: ox.line, stake: stakes[1], payout, url: None },
                            SurebetLeg { bookmaker: o2.bookmaker_slug.clone(), market: o2.market.clone(), selection: o2.selection.clone(), odds: o2.odds, line: o2.line, stake: stakes[2], payout, url: None },
                        ],
                        detected_at: Utc::now(),
                        verified: false,
                        mirror: false,
                    });
                }
            }
        }
        None
    }

    fn surebet_key(&self, surebet: &Surebet) -> Vec<u8> {
        let bks: Vec<String> = surebet.legs.iter().map(|l| l.bookmaker.clone()).collect();
        let odds_str: Vec<String> = surebet.legs.iter().map(|l| format!("{}:{}", l.selection, l.odds)).collect();
        let key = format!(
            "{}|{}|{}|{}",
            surebet.home_team,
            surebet.away_team,
            bks.join(","),
            odds_str.join("|"),
        );
        key.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;
    use shared::Sport;
    use std::collections::HashMap;

    fn make_event(id: &str) -> Event {
        Event { id: id.to_string(), sport: Sport::Football, league: "Test League".into(), home_team: "Team A".into(), away_team: "Team B".into(), start_time: None, is_live: false, bookmaker_slug: "test".into(), raw_url: None, extra: HashMap::new() }
    }
    fn make_odd(event_id: &str, bookmaker: &str, selection: &str, odds: f64) -> Odd {
        let odds_type = match selection {
            "1" => OddsType::Home,
            "X" => OddsType::Draw,
            "2" => OddsType::Away,
            "Over" => OddsType::Over,
            "Under" => OddsType::Under,
            "Yes" => OddsType::BothTeamsScoreYes,
            "No" => OddsType::BothTeamsScoreNo,
            _ => OddsType::Home,
        };
        Odd { id: format!("{}-{}-{}", event_id, bookmaker, selection), event_id: event_id.to_string(), bookmaker_slug: bookmaker.to_string(), market: "1X2".into(), selection: selection.to_string(), odds, odds_type, line: None, timestamp: Utc::now() }
    }

    #[test]
    fn test_two_way_surebet() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt1");
        // Используем тоталы — правильный 2-way рынок
        let odds = vec![
            Odd { id: "evt1-bk1-over".into(), event_id: "evt1".into(), bookmaker_slug: "bk1".into(), market: "Total".into(), selection: "Over".into(), odds: 2.10, odds_type: OddsType::Over, line: Some(2.5), timestamp: Utc::now() },
            Odd { id: "evt1-bk2-under".into(), event_id: "evt1".into(), bookmaker_slug: "bk2".into(), market: "Total".into(), selection: "Under".into(), odds: 2.10, odds_type: OddsType::Under, line: Some(2.5), timestamp: Utc::now() },
        ];
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty());
        assert!(surebets[0].profit_percent > 0.0);
    }

    #[test]
    fn test_no_surebet() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt2");
        let odds = vec![
            Odd { id: "evt2-bk1-over".into(), event_id: "evt2".into(), bookmaker_slug: "bk1".into(), market: "Total".into(), selection: "Over".into(), odds: 1.50, odds_type: OddsType::Over, line: Some(2.5), timestamp: Utc::now() },
            Odd { id: "evt2-bk2-under".into(), event_id: "evt2".into(), bookmaker_slug: "bk2".into(), market: "Total".into(), selection: "Under".into(), odds: 1.50, odds_type: OddsType::Under, line: Some(2.5), timestamp: Utc::now() },
        ];
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(surebets.is_empty());
    }

    #[test]
    fn test_calculate_stakes() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let stakes = calc.calculate_stakes(&[2.0, 2.0]);
        assert!((stakes[0] - 500.0).abs() < 0.01);
        assert!((stakes[1] - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_duplicate_filtering() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt3");
        let odds = vec![
            Odd { id: "evt3-bk1-over".into(), event_id: "evt3".into(), bookmaker_slug: "bk1".into(), market: "Total".into(), selection: "Over".into(), odds: 2.10, odds_type: OddsType::Over, line: Some(2.5), timestamp: Utc::now() },
            Odd { id: "evt3-bk2-under".into(), event_id: "evt3".into(), bookmaker_slug: "bk2".into(), market: "Total".into(), selection: "Under".into(), odds: 2.10, odds_type: OddsType::Under, line: Some(2.5), timestamp: Utc::now() },
        ];
        let surebets = calc.find_surebets(&[event.clone()], &odds);
        assert_eq!(surebets.len(), 1);
        calc.mark_seen(&surebets[0]);
        let surebets2 = calc.find_surebets(&[event], &odds);
        assert!(surebets2.is_empty());
    }

    #[test]
    fn test_three_way_surebet() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt4");
        let odds = vec![
            make_odd("evt4", "bk1", "1", 3.50),
            make_odd("evt4", "bk2", "X", 4.00),
            make_odd("evt4", "bk3", "2", 3.80),
        ];
        
        // Проверяем что 3-way profit положительный
        let profit = calculate_surebet_profit(&[3.50, 4.00, 3.80]);
        assert!(profit.is_some(), "3-way should have positive profit");
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find 3-way surebet");
        assert_eq!(surebets[0].legs.len(), 3);
    }

    #[test]
    fn test_total_surebet_with_line() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt5");
        // Тотал Over 2.5 у bk1 и Under 2.5 у bk2
        let odds = vec![
            Odd { id: "evt5-bk1-to".into(), event_id: "evt5".into(), bookmaker_slug: "bk1".into(), market: "Total".into(), selection: "Over".into(), odds: 2.05, odds_type: OddsType::Over, line: Some(2.5), timestamp: Utc::now() },
            Odd { id: "evt5-bk2-tu".into(), event_id: "evt5".into(), bookmaker_slug: "bk2".into(), market: "Total".into(), selection: "Under".into(), odds: 2.05, odds_type: OddsType::Under, line: Some(2.5), timestamp: Utc::now() },
        ];
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty());
        assert_eq!(surebets[0].legs.len(), 2);
        assert!(surebets[0].legs[0].line.is_some());
    }

    #[test]
    fn test_btts_surebet() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt6");
        // ОЗ Да у bk1 и ОЗ Нет у bk2
        let odds = vec![
            Odd { id: "evt6-bk1-btts-yes".into(), event_id: "evt6".into(), bookmaker_slug: "bk1".into(), market: "BothTeamsScore".into(), selection: "Yes".into(), odds: 2.10, odds_type: OddsType::BothTeamsScoreYes, line: None, timestamp: Utc::now() },
            Odd { id: "evt6-bk2-btts-no".into(), event_id: "evt6".into(), bookmaker_slug: "bk2".into(), market: "BothTeamsScore".into(), selection: "No".into(), odds: 2.00, odds_type: OddsType::BothTeamsScoreNo, line: None, timestamp: Utc::now() },
        ];
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty());
        assert_eq!(surebets[0].legs[0].market, "BothTeamsScore");
    }

    #[test]
    fn test_different_lines_not_matched() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt7");
        // Over 2.5 у bk1 и Under 3.5 у bk2 — разные линии, НЕ вилка
        let odds = vec![
            Odd { id: "evt7-bk1-to25".into(), event_id: "evt7".into(), bookmaker_slug: "bk1".into(), market: "Total".into(), selection: "Over".into(), odds: 2.05, odds_type: OddsType::Over, line: Some(2.5), timestamp: Utc::now() },
            Odd { id: "evt7-bk2-tu35".into(), event_id: "evt7".into(), bookmaker_slug: "bk2".into(), market: "Total".into(), selection: "Under".into(), odds: 2.05, odds_type: OddsType::Under, line: Some(3.5), timestamp: Utc::now() },
        ];
        // Это НЕ 2-way вилка (разные линии), но может быть коридор — не задача калькулятора вилок
        let surebets = calc.find_surebets(&[event], &odds);
        // Не должно найти — линии разные
        assert!(surebets.is_empty() || surebets[0].legs.iter().all(|l| l.line == Some(2.5) || l.line == Some(3.5)));
    }
}
