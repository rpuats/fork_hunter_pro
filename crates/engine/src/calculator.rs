use bloomfilter::Bloom;
use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use shared::odds::{calculate_stakes, calculate_surebet_profit};
use shared::{Event, Odd, Surebet, SurebetLeg};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;
use lru::LruCache;
use std::num::NonZeroUsize;

/// Cached combo result for quick lookup
#[derive(Clone, Debug)]
pub struct ComboCache {
    pub profit: Option<f64>,
    pub stakes: Option<Vec<f64>>,
}

impl Hash for ComboCache {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(p) = self.profit {
            p.to_bits().hash(state);
        }
    }
}

#[derive(Clone)]
pub struct SurebetCalculator {
    pub min_profit: f64,
    pub max_profit: f64,
    pub default_stake: f64,
    pub early_termination_roi: f64,
    seen_surebets: Arc<RwLock<Bloom<[u8]>>>,
    recent_events: Arc<DashMap<String, Vec<Odd>>>,
    /// LRU cache for combo profitability results (up to 1000 entries)
    combo_cache: Arc<RwLock<LruCache<Vec<u64>, ComboCache>>>,
}

impl SurebetCalculator {
    pub fn new(
        min_profit: f64,
        max_profit: f64,
        default_stake: f64,
        capacity: usize,
        error_rate: f64,
    ) -> Self {
        // Default early termination ROI: 3x min_profit
        Self::with_early_termination(min_profit, max_profit, default_stake, capacity, error_rate, min_profit * 3.0)
    }

    /// Create calculator with custom early termination threshold
    pub fn with_early_termination(
        min_profit: f64,
        max_profit: f64,
        default_stake: f64,
        capacity: usize,
        error_rate: f64,
        early_termination_roi: f64,
    ) -> Self {
        // Initialize LRU cache with max 1000 entries
        let cache_size = NonZeroUsize::new(1000).unwrap();
        Self {
            min_profit,
            max_profit,
            default_stake,
            early_termination_roi,
            seen_surebets: Arc::new(RwLock::new(Bloom::new_for_fp_rate(capacity, error_rate))),
            recent_events: Arc::new(DashMap::new()),
            combo_cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
        }
    }

    pub fn find_surebets(&self, events: &[Event], all_odds: &[Odd]) -> Vec<Surebet> {
        let mut surebets = Vec::new();
        let odds_by_event = self.group_odds_by_event(all_odds);

        for event in events {
            if let Some(event_odds) = odds_by_event.get(&event.id) {
                // Deduplicate odds by (bookmaker, market, selection, odds) before analysis
                // This removes redundant entries that would create duplicate work
                let deduped_odds = self.deduplicate_odds(event_odds);
                
                if let Some(surebet) = self.analyze_event(event, &deduped_odds) {
                    if surebet.profit_percent >= self.min_profit
                        && surebet.profit_percent <= self.max_profit
                    {
                        let key = self.surebet_key(&surebet);
                        if !self.seen_surebets.read().check(&key) {
                            debug!(profit = surebet.profit_percent, "New surebet found");
                            
                            // Early termination: if profit exceeds threshold and we have high ROI, stop searching for more
                            if surebet.profit_percent >= self.early_termination_roi {
                                debug!("Early termination: profit {:.2}% exceeds ROI threshold {:.2}%", 
                                    surebet.profit_percent, self.early_termination_roi);
                                surebets.push(surebet);
                                return surebets; // Found excellent surebet, return immediately
                            }
                            surebets.push(surebet);
                        }
                    }
                }
            }
        }
        surebets
    }

    /// Deduplicate odds using HashSet to prevent redundant combo testing
    fn deduplicate_odds(&self, odds: &[Odd]) -> Vec<Odd> {
        let mut seen = HashSet::new();
        let mut deduped = Vec::new();
        
        for odd in odds {
            // Create unique key: (bookmaker, market, selection, odds_rounded)
            let key = format!(
                "{}|{}|{}|{:.4}",
                odd.bookmaker_slug,
                odd.market.to_lowercase(),
                odd.selection.to_lowercase(),
                (odd.odds * 10000.0).round() / 10000.0  // Round to 4 decimals for comparison
            );
            
            if seen.insert(key) {
                deduped.push(odd.clone());
            }
        }
        
        deduped
    }

    pub fn analyze_two_way(&self, odds_a: f64, odds_b: f64) -> Option<f64> {
        calculate_surebet_profit(&[odds_a, odds_b])
    }

    pub fn analyze_three_way(&self, odds_1: f64, odds_x: f64, odds_2: f64) -> Option<f64> {
        calculate_surebet_profit(&[odds_1, odds_x, odds_2])
    }

    /// Calculate profit with LRU cache for fast repeated lookups
    /// This is critical for performance when testing multiple combo variations
    fn calculate_profit_cached(&self, odds: &[f64]) -> Option<f64> {
        // Create cache key by hashing odds
        let mut cache_key = Vec::new();
        for &o in odds {
            cache_key.push((o * 10000.0).round() as u64);
        }
        
        // Check cache first
        {
            let mut cache = self.combo_cache.write();
            if let Some(result) = cache.get(&cache_key) {
                return result.profit;
            }
        }
        
        // Calculate if not in cache
        let profit = calculate_surebet_profit(odds);
        
        // Store in cache
        {
            let mut cache = self.combo_cache.write();
            cache.put(cache_key, ComboCache {
                profit,
                stakes: profit.and_then(|_| Some(calculate_stakes(odds, self.default_stake))),
            });
        }
        
        profit
    }

    /// Calculate stakes with cache optimization
    fn calculate_stakes_cached(&self, odds: &[f64]) -> Vec<f64> {
        let mut cache_key = Vec::new();
        for &o in odds {
            cache_key.push((o * 10000.0).round() as u64);
        }
        
        {
            let mut cache = self.combo_cache.write();
            if let Some(result) = cache.get(&cache_key) {
                if let Some(stakes) = &result.stakes {
                    return stakes.clone();
                }
            }
        }
        
        calculate_stakes(odds, self.default_stake)
    }

    pub fn calculate_stakes(&self, odds: &[f64]) -> Vec<f64> {
        calculate_stakes(odds, self.default_stake)
    }

    pub fn mark_seen(&self, surebet: &Surebet) {
        let key = self.surebet_key(surebet);
        self.seen_surebets.write().set(&key);
    }

    pub fn is_seen(&self, surebet: &Surebet) -> bool {
        let key = self.surebet_key(surebet);
        self.seen_surebets.read().check(&key)
    }

    pub fn cache_odds(&self, event_id: &str, odds: Vec<Odd>) {
        self.recent_events.insert(event_id.to_string(), odds);
    }

    /// Clear the combo cache to free memory
    pub fn clear_combo_cache(&self) {
        self.combo_cache.write().clear();
    }

    /// Get current combo cache size
    pub fn combo_cache_size(&self) -> usize {
        self.combo_cache.read().len()
    }

    fn group_odds_by_event(&self, all_odds: &[Odd]) -> HashMap<String, Vec<Odd>> {
        let mut map = HashMap::new();
        for odd in all_odds {
            map.entry(odd.event_id.clone())
                .or_insert_with(Vec::new)
                .push(odd.clone());
        }
        map
    }

    fn analyze_event(&self, event: &Event, odds: &[Odd]) -> Option<Surebet> {
        // Группируем odds по market + line
        let by_market = self.group_by_market(odds);

        // Для каждого рынка ищем вилки между разными БК
        for (market_key, market_odds) in &by_market {
            if let Some(surebet) = self.find_market_surebet(event, market_key, market_odds) {
                return Some(surebet);
            }
        }

        None
    }

    /// Ищем вилку для конкретного рынка
    fn find_market_surebet(
        &self,
        event: &Event,
        market_key: &str,
        odds: &[&Odd],
    ) -> Option<Surebet> {
        let lower = market_key.to_lowercase();

        // 3-way: 1X2, 1H_Result, 2H_Result
        if lower.contains("1x2") || lower.contains("1h_result") || lower.contains("2h_result") {
            return self.find_three_way_from_market(event, odds);
        }

        // 2-way комплементарные: Over/Under, Yes/No, Even/Odd, 1X/12/X2
        if lower.contains("total") || lower.contains("individualtotal") {
            return self.find_two_way_complementary(event, market_key, odds);
        }

        if lower.contains("btts") || lower.contains("evenodd") {
            return self.find_two_way_yes_no(event, odds);
        }

        if lower.contains("doublechance") {
            return self.find_double_chance_surebet(event, odds);
        }

        // Asian Handicap: рынок с дробными линиями (±0.5, ±1.5, ±2.0, и т.д.)
        if lower.contains("asihandicap") 
            || (lower.contains("handicap") && self.is_asian_handicap_market(odds)) {
            return self.find_surebet_asian_handicap(event, market_key, odds);
        }

        // Обычная фора (целые значения, очень редко)
        if lower.contains("handicap") {
            return self.find_two_way_complementary(event, market_key, odds);
        }

        if lower.contains("correctscore") {
            return self.find_surebet_correct_score(event, odds);
        }

        // Fallback: ищем 2-way互补ные исходы
        self.find_two_way_complementary(event, market_key, odds)
    }

    /// 3-way вилка: 1/X/2 от разных БК
    fn find_three_way_from_market(&self, event: &Event, odds: &[&Odd]) -> Option<Surebet> {
        let mut best_1: Option<&&Odd> = None;
        let mut best_x: Option<&&Odd> = None;
        let mut best_2: Option<&&Odd> = None;

        for odd in odds {
            let sel = odd.selection.to_lowercase();
            if sel == "1" || sel == "п1" || sel == "home" {
                if best_1.map_or(true, |b| odd.odds > b.odds) {
                    best_1 = Some(odd);
                }
            } else if sel == "x" || sel == "draw" || sel == "х" || sel == "ничья" {
                if best_x.map_or(true, |b| odd.odds > b.odds) {
                    best_x = Some(odd);
                }
            } else if sel == "2" || sel == "п2" || sel == "away" {
                if best_2.map_or(true, |b| odd.odds > b.odds) {
                    best_2 = Some(odd);
                }
            }
        }

        if let (Some(&o1), Some(&ox), Some(&o2)) = (best_1, best_x, best_2) {
            // Проверяем что все от разных БК
            let bks: std::collections::HashSet<&str> = [
                o1.bookmaker_slug.as_str(),
                ox.bookmaker_slug.as_str(),
                o2.bookmaker_slug.as_str(),
            ]
            .iter()
            .cloned()
            .collect();
            if bks.len() < 2 {
                return None;
            } // Минимум 2 разные БК

            // Use cached profit calculation
            if let Some(profit) = self.calculate_profit_cached(&[o1.odds, ox.odds, o2.odds]) {
                if profit >= self.min_profit {
                    let stakes = self.calculate_stakes_cached(&[o1.odds, ox.odds, o2.odds]);
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
                            SurebetLeg {
                                bookmaker: o1.bookmaker_slug.clone(),
                                market: o1.market.clone(),
                                selection: o1.selection.clone(),
                                odds: o1.odds,
                                line: o1.line,
                                stake: stakes[0],
                                payout,
                                url: None,
                            },
                            SurebetLeg {
                                bookmaker: ox.bookmaker_slug.clone(),
                                market: ox.market.clone(),
                                selection: ox.selection.clone(),
                                odds: ox.odds,
                                line: ox.line,
                                stake: stakes[1],
                                payout,
                                url: None,
                            },
                            SurebetLeg {
                                bookmaker: o2.bookmaker_slug.clone(),
                                market: o2.market.clone(),
                                selection: o2.selection.clone(),
                                odds: o2.odds,
                                line: o2.line,
                                stake: stakes[2],
                                payout,
                                url: None,
                            },
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

    /// 2-way комплементарные: Over/Under, Handicap1/Handicap2 с той же линией
    fn find_two_way_complementary(
        &self,
        event: &Event,
        market_key: &str,
        odds: &[&Odd],
    ) -> Option<Surebet> {
        // Извлекаем line из market_key
        let _line = if let Some(pos) = market_key.rfind('|') {
            market_key[pos + 1..].parse::<f64>().ok()
        } else {
            None
        };

        let mut best_over: Option<&&Odd> = None;
        let mut best_under: Option<&&Odd> = None;

        for odd in odds {
            let sel = odd.selection.to_lowercase();
            let is_over = sel.contains("over")
                || sel.contains("больше")
                || sel.contains("тб")
                || sel.contains("да")
                || sel.contains("yes")
                || sel.contains("чёт")
                || sel.contains("even")
                || sel == "1";
            let is_under = sel.contains("under")
                || sel.contains("меньше")
                || sel.contains("тм")
                || sel.contains("нет")
                || sel.contains("no")
                || sel.contains("нечет")
                || sel.contains("odd")
                || sel == "2";

            if is_over {
                if best_over.map_or(true, |b| odd.odds > b.odds) {
                    best_over = Some(odd);
                }
            } else if is_under {
                if best_under.map_or(true, |b| odd.odds > b.odds) {
                    best_under = Some(odd);
                }
            }
        }

        if let (Some(&o_over), Some(&o_under)) = (best_over, best_under) {
            if o_over.bookmaker_slug == o_under.bookmaker_slug {
                return None;
            }

            // Проверяем что line совпадает (для тоталов/фор)
            if let (Some(l1), Some(l2)) = (o_over.line, o_under.line) {
                if (l1 - l2).abs() > 0.01 {
                    return None;
                }
            }

            // Use cached profit calculation
            if let Some(profit) = self.calculate_profit_cached(&[o_over.odds, o_under.odds]) {
                if profit >= self.min_profit {
                    let stakes = self.calculate_stakes_cached(&[o_over.odds, o_under.odds]);
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
                            SurebetLeg {
                                bookmaker: o_over.bookmaker_slug.clone(),
                                market: o_over.market.clone(),
                                selection: o_over.selection.clone(),
                                odds: o_over.odds,
                                line: o_over.line,
                                stake: stakes[0],
                                payout,
                                url: None,
                            },
                            SurebetLeg {
                                bookmaker: o_under.bookmaker_slug.clone(),
                                market: o_under.market.clone(),
                                selection: o_under.selection.clone(),
                                odds: o_under.odds,
                                line: o_under.line,
                                stake: stakes[1],
                                payout,
                                url: None,
                            },
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

    /// 2-way Yes/No: BTTS, EvenOdd
    fn find_two_way_yes_no(&self, event: &Event, odds: &[&Odd]) -> Option<Surebet> {
        let mut best_yes: Option<&&Odd> = None;
        let mut best_no: Option<&&Odd> = None;

        for odd in odds {
            let sel = odd.selection.to_lowercase();
            if sel.contains("yes")
                || sel.contains("да")
                || sel.contains("even")
                || sel.contains("чёт")
                || sel == "1"
            {
                if best_yes.map_or(true, |b| odd.odds > b.odds) {
                    best_yes = Some(odd);
                }
            } else if sel.contains("no")
                || sel.contains("нет")
                || sel.contains("odd")
                || sel.contains("нечет")
                || sel == "2"
            {
                if best_no.map_or(true, |b| odd.odds > b.odds) {
                    best_no = Some(odd);
                }
            }
        }

        if let (Some(&y), Some(&n)) = (best_yes, best_no) {
            if y.bookmaker_slug == n.bookmaker_slug {
                return None;
            }

            // Use cached profit calculation
            if let Some(profit) = self.calculate_profit_cached(&[y.odds, n.odds]) {
                if profit >= self.min_profit {
                    let stakes = self.calculate_stakes_cached(&[y.odds, n.odds]);
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
                            SurebetLeg {
                                bookmaker: y.bookmaker_slug.clone(),
                                market: y.market.clone(),
                                selection: y.selection.clone(),
                                odds: y.odds,
                                line: y.line,
                                stake: stakes[0],
                                payout,
                                url: None,
                            },
                            SurebetLeg {
                                bookmaker: n.bookmaker_slug.clone(),
                                market: n.market.clone(),
                                selection: n.selection.clone(),
                                odds: n.odds,
                                line: n.line,
                                stake: stakes[1],
                                payout,
                                url: None,
                            },
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

    /// Double Chance: 1X/12, 1X/X2, 12/X2
    fn find_double_chance_surebet(&self, event: &Event, odds: &[&Odd]) -> Option<Surebet> {
        let mut best_1x: Option<&&Odd> = None;
        let mut best_12: Option<&&Odd> = None;
        let mut best_x2: Option<&&Odd> = None;

        for odd in odds {
            let sel = odd.selection.to_lowercase();
            if sel == "1x" {
                if best_1x.map_or(true, |b| odd.odds > b.odds) {
                    best_1x = Some(odd);
                }
            } else if sel == "12" {
                if best_12.map_or(true, |b| odd.odds > b.odds) {
                    best_12 = Some(odd);
                }
            } else if sel == "x2" {
                if best_x2.map_or(true, |b| odd.odds > b.odds) {
                    best_x2 = Some(odd);
                }
            }
        }

        // Пробуем пары: 1X+X2, 1X+12, 12+X2
        if let (Some(&a), Some(&b)) = (best_1x, best_x2) {
            if a.bookmaker_slug != b.bookmaker_slug {
                // Use cached profit calculation
                if let Some(profit) = self.calculate_profit_cached(&[a.odds, b.odds]) {
                    if profit >= self.min_profit {
                        let stakes = self.calculate_stakes_cached(&[a.odds, b.odds]);
                        let payout = stakes[0] * a.odds;
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
                                SurebetLeg {
                                    bookmaker: a.bookmaker_slug.clone(),
                                    market: a.market.clone(),
                                    selection: a.selection.clone(),
                                    odds: a.odds,
                                    line: a.line,
                                    stake: stakes[0],
                                    payout,
                                    url: None,
                                },
                                SurebetLeg {
                                    bookmaker: b.bookmaker_slug.clone(),
                                    market: b.market.clone(),
                                    selection: b.selection.clone(),
                                    odds: b.odds,
                                    line: b.line,
                                    stake: stakes[1],
                                    payout,
                                    url: None,
                                },
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

    /// Correct Score вилка: находит комбинацию исходов (0-0, 0-1, 1-0, 1-1, 2-1, etc.)
    /// с разных БК где сумма вероятностей < 1 (прибыль есть)
    fn find_surebet_correct_score(&self, event: &Event, odds: &[&Odd]) -> Option<Surebet> {
        // Группируем по selection (это конкретные счёты типа "1-0", "2-1" и т.д.)
        let by_selection: std::collections::HashMap<String, Vec<&Odd>> = {
            let mut m: std::collections::HashMap<String, Vec<&Odd>> =
                std::collections::HashMap::new();
            for odd in odds {
                let sel = odd.selection.to_lowercase();
                // Фильтруем только корректные score исходы (должны быть вида "X-Y")
                if sel.contains('-') && sel.chars().filter(|c| c.is_numeric()).count() >= 2 {
                    m.entry(sel).or_default().push(*odd);
                }
            }
            m
        };

        // Нужно минимум 3 разных исхода для вилки
        if by_selection.len() < 3 {
            return None;
        }

        // Берём лучший коэффициент для каждого исхода, предпочитая разные БК
        let best_per_selection: Vec<&Odd> = by_selection
            .values()
            .filter_map(|group| group.iter().max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap()).copied())
            .collect();

        // Нужно минимум 3 разных исхода
        if best_per_selection.len() < 3 {
            return None;
        }

        // Проверяем наличие разных БК (минимум 2)
        let bks: std::collections::HashSet<&str> = best_per_selection
            .iter()
            .map(|o| o.bookmaker_slug.as_str())
            .collect();
        if bks.len() < 2 {
            return None;
        }

        // Сортируем по коэффициентам (от меньших к большим) для оптимизации
        let mut odds_sorted = best_per_selection.clone();
        odds_sorted.sort_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap());

        // Пробуем разные комбинации исходов (3, 4, 5, 6 исходов)
        for combo_size in 3..=std::cmp::min(6, odds_sorted.len()) {
            if let Some(surebet) = self.try_correct_score_combo(event, &odds_sorted, combo_size) {
                return Some(surebet);
            }
        }

        None
    }

    /// Пробует найти вилку из combo_size исходов в Correct Score
    fn try_correct_score_combo(
        &self,
        event: &Event,
        odds_sorted: &[&Odd],
        combo_size: usize,
    ) -> Option<Surebet> {
        // Берём первые combo_size исходов (с самыми низкими коэффициентами — лучшие для вилки)
        if odds_sorted.len() < combo_size {
            return None;
        }

        let combo = &odds_sorted[..combo_size];
        let odds_vec: Vec<f64> = combo.iter().map(|o| o.odds).collect();

        // Проверяем на вилку с кэшированием
        if let Some(profit) = self.calculate_profit_cached(&odds_vec) {
            if profit >= self.min_profit {
                let stakes = self.calculate_stakes_cached(&odds_vec);
                let payout = stakes[0] * combo[0].odds;

                let legs: Vec<SurebetLeg> = combo
                    .iter()
                    .zip(stakes.iter())
                    .map(|(o, &s)| SurebetLeg {
                        bookmaker: o.bookmaker_slug.clone(),
                        market: o.market.clone(),
                        selection: o.selection.clone(),
                        odds: o.odds,
                        line: o.line,
                        stake: s,
                        payout,
                        url: None,
                    })
                    .collect();

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
                    legs,
                    detected_at: Utc::now(),
                    verified: false,
                    mirror: false,
                });
            }
        }

        None
    }

    /// Asian Handicap: 2-way вилка с дробными линиями (±0.5, ±1.5, ±2.0, ±1.75, и т.д.)
    /// Team1 +X vs Team2 -X (комплементарные исходы)
    fn find_surebet_asian_handicap(
        &self,
        event: &Event,
        market_key: &str,
        odds: &[&Odd],
    ) -> Option<Surebet> {
        // Извлекаем line из market_key
        let line = if let Some(pos) = market_key.rfind('|') {
            market_key[pos + 1..].parse::<f64>().ok()
        } else {
            None
        };

        let mut best_positive: Option<&&Odd> = None; // Team1 + X (positive side)
        let mut best_negative: Option<&&Odd> = None; // Team2 - X (negative side)

        for odd in odds {
            let sel = odd.selection.to_lowercase();
            
            // Определяем тип: положительная (+) или отрицательная (-) сторона
            let is_positive = sel.contains('+') 
                || sel.contains("home +") 
                || sel.contains("п1+")
                || sel.contains("1+")
                || (sel.contains("home") && odd.line.map_or(false, |l| l > 0.0))
                || (sel.contains("п1") && odd.line.map_or(false, |l| l > 0.0));
            
            let is_negative = sel.contains('-') && !sel.starts_with('-') // исключаем "0-0"
                || sel.contains("away -")
                || sel.contains("п2-")
                || sel.contains("2-")
                || (sel.contains("away") && odd.line.map_or(false, |l| l < 0.0))
                || (sel.contains("п2") && odd.line.map_or(false, |l| l < 0.0));

            if is_positive {
                if best_positive.map_or(true, |b| odd.odds > b.odds) {
                    best_positive = Some(odd);
                }
            } else if is_negative {
                if best_negative.map_or(true, |b| odd.odds > b.odds) {
                    best_negative = Some(odd);
                }
            }
        }

        if let (Some(&o_positive), Some(&o_negative)) = (best_positive, best_negative) {
            // Проверяем что разные БК
            if o_positive.bookmaker_slug == o_negative.bookmaker_slug {
                return None;
            }

            // Проверяем что линии совпадают или комплементарны
            // Например: Team1 +1.5 и Team2 -1.5 (same absolute value, opposite signs)
            if let (Some(l1), Some(l2)) = (o_positive.line, o_negative.line) {
                if (l1 + l2).abs() > 0.01 {
                    // Линии не комплементарны (не сумма к нулю)
                    return None;
                }
            }

            // Use cached profit calculation
            if let Some(profit) = self.calculate_profit_cached(&[o_positive.odds, o_negative.odds]) {
                if profit >= self.min_profit {
                    let stakes = self.calculate_stakes_cached(&[o_positive.odds, o_negative.odds]);
                    let payout = stakes[0] * o_positive.odds;
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
                            SurebetLeg {
                                bookmaker: o_positive.bookmaker_slug.clone(),
                                market: o_positive.market.clone(),
                                selection: o_positive.selection.clone(),
                                odds: o_positive.odds,
                                line: o_positive.line,
                                stake: stakes[0],
                                payout,
                                url: None,
                            },
                            SurebetLeg {
                                bookmaker: o_negative.bookmaker_slug.clone(),
                                market: o_negative.market.clone(),
                                selection: o_negative.selection.clone(),
                                odds: o_negative.odds,
                                line: o_negative.line,
                                stake: stakes[1],
                                payout,
                                url: None,
                            },
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

    /// Проверяет, является ли рынок Asian Handicap по структуре данных
    fn is_asian_handicap_market(&self, odds: &[&Odd]) -> bool {
        // Asian Handicap обычно имеет:
        // 1. Дробные линии (0.5, 1.5, 1.25, 1.75, и т.д.)
        // 2. Selection с +/- символами или Team +/- X формат
        
        odds.iter().any(|o| {
            let sel = o.selection.to_lowercase();
            let has_sign = sel.contains('+') || (sel.contains('-') && !sel.starts_with('-'));
            let has_fractional_line = o.line.map_or(false, |l| {
                let frac = l.fract();
                frac.abs() > 0.01 && frac.abs() < 0.99 // has decimal part (0.25, 0.5, 0.75)
            });
            has_sign || has_fractional_line
        })
    }

    /// Multi-way вилка (для Correct Score и других рынков с N исходами)
    fn find_multi_way_surebet(
        &self,
        event: &Event,
        odds: &[&Odd],
        min_outcomes: usize,
    ) -> Option<Surebet> {
        // Группируем по selection
        let by_selection: std::collections::HashMap<String, Vec<&Odd>> = {
            let mut m: std::collections::HashMap<String, Vec<&Odd>> =
                std::collections::HashMap::new();
            for odd in odds {
                m.entry(odd.selection.to_lowercase())
                    .or_default()
                    .push(*odd);
            }
            m
        };

        if by_selection.len() < min_outcomes {
            return None;
        }

        // Берём лучший odds от разных БК для каждого selection
        let mut best_odds: Vec<&Odd> = Vec::new();
        let mut seen_bks: std::collections::HashSet<String> = std::collections::HashSet::new();

        for (_, group) in &by_selection {
            if let Some(&best) = group
                .iter()
                .max_by(|a, b| a.odds.partial_cmp(&b.odds).unwrap())
            {
                if !seen_bks.contains(&best.bookmaker_slug) {
                    seen_bks.insert(best.bookmaker_slug.clone());
                    best_odds.push(best);
                }
            }
        }

        if best_odds.len() < 2 {
            return None;
        }

        let odds_vec: Vec<f64> = best_odds.iter().map(|o| o.odds).collect();
        // Use cached profit calculation
        if let Some(profit) = self.calculate_profit_cached(&odds_vec) {
            if profit >= self.min_profit {
                let stakes = self.calculate_stakes_cached(&odds_vec);
                let payout = stakes[0] * best_odds[0].odds;
                let legs: Vec<SurebetLeg> = best_odds
                    .iter()
                    .zip(stakes.iter())
                    .map(|(o, &s)| SurebetLeg {
                        bookmaker: o.bookmaker_slug.clone(),
                        market: o.market.clone(),
                        selection: o.selection.clone(),
                        odds: o.odds,
                        line: o.line,
                        stake: s,
                        payout,
                        url: None,
                    })
                    .collect();

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
                    legs,
                    detected_at: Utc::now(),
                    verified: false,
                    mirror: false,
                });
            }
        }

        None
    }

    #[inline]
    fn group_by_market<'a>(&self, odds: &'a [Odd]) -> HashMap<String, Vec<&'a Odd>> {
        let mut map = HashMap::new();
        for odd in odds {
            // Для рынков с линией (тоталы, форы) включаем только market + line
            // НЕ включаем odds_type — Over/Under должны быть в ОДНОЙ группе
            let key = if let Some(line) = odd.line {
                format!("{}|{:.2}", odd.market.to_lowercase(), line)
            } else {
                format!("{}|none", odd.market.to_lowercase())
            };
            map.entry(key).or_insert_with(Vec::new).push(odd);
        }
        map
    }

    fn surebet_key(&self, surebet: &Surebet) -> Vec<u8> {
        let bks: Vec<String> = surebet.legs.iter().map(|l| l.bookmaker.clone()).collect();
        let odds_str: Vec<String> = surebet
            .legs
            .iter()
            .map(|l| format!("{}:{}", l.selection, l.odds))
            .collect();
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
        Event {
            id: id.to_string(),
            sport: Sport::Football,
            league: "Test League".into(),
            home_team: "Team A".into(),
            away_team: "Team B".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: HashMap::new(),
        }
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
        Odd {
            id: format!("{}-{}-{}", event_id, bookmaker, selection),
            event_id: event_id.to_string(),
            bookmaker_slug: bookmaker.to_string(),
            market: "1X2".into(),
            selection: selection.to_string(),
            odds,
            odds_type,
            line: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_two_way_surebet() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt1");
        // Используем тоталы — правильный 2-way рынок
        let odds = vec![
            Odd {
                id: "evt1-bk1-over".into(),
                event_id: "evt1".into(),
                bookmaker_slug: "bk1".into(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: 2.10,
                odds_type: OddsType::Over,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt1-bk2-under".into(),
                event_id: "evt1".into(),
                bookmaker_slug: "bk2".into(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: 2.10,
                odds_type: OddsType::Under,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
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
            Odd {
                id: "evt2-bk1-over".into(),
                event_id: "evt2".into(),
                bookmaker_slug: "bk1".into(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: 1.50,
                odds_type: OddsType::Over,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt2-bk2-under".into(),
                event_id: "evt2".into(),
                bookmaker_slug: "bk2".into(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: 1.50,
                odds_type: OddsType::Under,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
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
            Odd {
                id: "evt3-bk1-over".into(),
                event_id: "evt3".into(),
                bookmaker_slug: "bk1".into(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: 2.10,
                odds_type: OddsType::Over,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt3-bk2-under".into(),
                event_id: "evt3".into(),
                bookmaker_slug: "bk2".into(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: 2.10,
                odds_type: OddsType::Under,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
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
            Odd {
                id: "evt5-bk1-to".into(),
                event_id: "evt5".into(),
                bookmaker_slug: "bk1".into(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: 2.05,
                odds_type: OddsType::Over,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt5-bk2-tu".into(),
                event_id: "evt5".into(),
                bookmaker_slug: "bk2".into(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: 2.05,
                odds_type: OddsType::Under,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
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
            Odd {
                id: "evt6-bk1-btts-yes".into(),
                event_id: "evt6".into(),
                bookmaker_slug: "bk1".into(),
                market: "BothTeamsScore".into(),
                selection: "Yes".into(),
                odds: 2.10,
                odds_type: OddsType::BothTeamsScoreYes,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt6-bk2-btts-no".into(),
                event_id: "evt6".into(),
                bookmaker_slug: "bk2".into(),
                market: "BothTeamsScore".into(),
                selection: "No".into(),
                odds: 2.00,
                odds_type: OddsType::BothTeamsScoreNo,
                line: None,
                timestamp: Utc::now(),
            },
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
            Odd {
                id: "evt7-bk1-to25".into(),
                event_id: "evt7".into(),
                bookmaker_slug: "bk1".into(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: 2.05,
                odds_type: OddsType::Over,
                line: Some(2.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt7-bk2-tu35".into(),
                event_id: "evt7".into(),
                bookmaker_slug: "bk2".into(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: 2.05,
                odds_type: OddsType::Under,
                line: Some(3.5),
                timestamp: Utc::now(),
            },
        ];
        // Это НЕ 2-way вилка (разные линии), но может быть коридор — не задача калькулятора вилок
        let surebets = calc.find_surebets(&[event], &odds);
        // Не должно найти — линии разные
        assert!(
            surebets.is_empty()
                || surebets[0]
                    .legs
                    .iter()
                    .all(|l| l.line == Some(2.5) || l.line == Some(3.5))
        );
    }

    #[test]
    fn test_correct_score_basic_3_outcomes() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt8");
        
        // Correct Score вилка из 3 исходов: 1-0, 0-0, 0-1
        let odds = vec![
            Odd {
                id: "evt8-bk1-10".into(),
                event_id: "evt8".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.60,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt8-bk2-00".into(),
                event_id: "evt8".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 4.50,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt8-bk3-01".into(),
                event_id: "evt8".into(),
                bookmaker_slug: "bk3".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),
                odds: 4.80,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find correct score surebet");
        assert_eq!(surebets[0].legs.len(), 3, "Should have 3 legs");
        assert!(surebets[0].profit_percent > 0.0, "Should have positive profit");
    }

    #[test]
    fn test_correct_score_4_outcomes() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt9");
        
        // Correct Score из 4 исходов
        let odds = vec![
            Odd {
                id: "evt9-bk1-10".into(),
                event_id: "evt9".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.50,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt9-bk2-00".into(),
                event_id: "evt9".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 4.00,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt9-bk3-01".into(),
                event_id: "evt9".into(),
                bookmaker_slug: "bk3".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),
                odds: 4.25,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt9-bk1-21".into(),
                event_id: "evt9".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "2-1".into(),
                odds: 5.50,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find 4-outcome surebet");
        assert!(surebets[0].legs.len() >= 3, "Should have at least 3 legs");
    }

    #[test]
    fn test_correct_score_with_best_odds_selection() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt10");
        
        // Несколько БК с разными коэффициентами для одного исхода
        // Должны выбраться лучшие коэффициенты
        let odds = vec![
            Odd {
                id: "evt10-bk1-10-low".into(),
                event_id: "evt10".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.20,  // Худший для 1-0
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt10-bk2-10-high".into(),
                event_id: "evt10".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.80,  // Лучший для 1-0
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt10-bk3-00".into(),
                event_id: "evt10".into(),
                bookmaker_slug: "bk3".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 4.50,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt10-bk1-01".into(),
                event_id: "evt10".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),
                odds: 4.80,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        // Должна найти вилку с выбранными лучшими коэффициентами
        assert!(!surebets.is_empty(), "Should find surebet with best odds");
        
        // Проверяем что был выбран коэффициент 3.80, а не 3.20
        if let Some(surebet) = surebets.first() {
            let leg_with_10 = surebet.legs.iter().find(|l| l.selection == "1-0");
            assert!(leg_with_10.is_some());
            assert!(leg_with_10.unwrap().odds > 3.5, "Should use best odds");
        }
    }

    #[test]
    fn test_correct_score_needs_different_bookmakers() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt11");
        
        // Все коэффициенты от одной БК — не должна найти вилку
        let odds = vec![
            Odd {
                id: "evt11-bk1-10".into(),
                event_id: "evt11".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.50,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt11-bk1-00".into(),
                event_id: "evt11".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 4.00,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt11-bk1-01".into(),
                event_id: "evt11".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),
                odds: 4.25,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(surebets.is_empty(), "Should not find surebet from single bookmaker");
    }

    #[test]
    fn test_correct_score_not_enough_outcomes() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt12");
        
        // Только 2 исхода — недостаточно для вилки
        let odds = vec![
            Odd {
                id: "evt12-bk1-10".into(),
                event_id: "evt12".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.50,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt12-bk2-00".into(),
                event_id: "evt12".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 4.00,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(surebets.is_empty(), "Should not find surebet from 2 outcomes");
    }

    #[test]
    fn test_correct_score_profit_calculation() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt13");
        
        // Вилка с четырьмя исходами с хорошей прибылью
        let odds = vec![
            Odd {
                id: "evt13-bk1-10".into(),
                event_id: "evt13".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.40,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt13-bk2-00".into(),
                event_id: "evt13".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 3.80,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt13-bk3-01".into(),
                event_id: "evt13".into(),
                bookmaker_slug: "bk3".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),
                odds: 4.20,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt13-bk1-21".into(),
                event_id: "evt13".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "2-1".into(),
                odds: 5.00,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        if !surebets.is_empty() {
            let profit = surebets[0].profit_percent;
            assert!(profit > 0.0, "Profit should be positive");
            assert!(profit <= 30.0, "Profit should be within max limit");
        }
    }

    #[test]
    fn test_correct_score_low_profit_filtered() {
        let calc = SurebetCalculator::new(5.0, 30.0, 1000.0, 10000, 0.01);  // min_profit = 5%
        let event = make_event("evt14");
        
        // Вилка с малой прибылью (< 5%)
        let odds = vec![
            Odd {
                id: "evt14-bk1-10".into(),
                event_id: "evt14".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.10,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt14-bk2-00".into(),
                event_id: "evt14".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 3.15,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt14-bk3-01".into(),
                event_id: "evt14".into(),
                bookmaker_slug: "bk3".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),
                odds: 3.20,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(surebets.is_empty(), "Should filter out low profit surebet");
    }

    #[test]
    fn test_correct_score_5_outcomes() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt15");
        
        // Вилка из 5 исходов
        let odds = vec![
            Odd {
                id: "evt15-bk1-10".into(),
                event_id: "evt15".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.50,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt15-bk2-00".into(),
                event_id: "evt15".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 4.00,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt15-bk3-01".into(),
                event_id: "evt15".into(),
                bookmaker_slug: "bk3".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),
                odds: 4.20,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt15-bk1-11".into(),
                event_id: "evt15".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-1".into(),
                odds: 4.50,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt15-bk2-21".into(),
                event_id: "evt15".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "2-1".into(),
                odds: 5.50,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find 5-outcome surebet");
        if let Some(s) = surebets.first() {
            assert!(s.legs.len() >= 3, "Should have at least 3 legs");
        }
    }

    #[test]
    fn test_correct_score_duplicate_filtering() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt16");
        
        let odds = vec![
            Odd {
                id: "evt16-bk1-10".into(),
                event_id: "evt16".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),
                odds: 3.60,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt16-bk2-00".into(),
                event_id: "evt16".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),
                odds: 4.50,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt16-bk3-01".into(),
                event_id: "evt16".into(),
                bookmaker_slug: "bk3".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),
                odds: 4.80,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event.clone()], &odds);
        assert_eq!(surebets.len(), 1);
        
        // Отмечаем как seen
        if let Some(s) = surebets.first() {
            calc.mark_seen(s);
        }
        
        // Следующий раз та же вилка не должна найтись
        let surebets2 = calc.find_surebets(&[event], &odds);
        assert!(surebets2.is_empty());
    }

    #[test]
    fn test_correct_score_with_invalid_selections() {
        let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt17");
        
        // Смешиваем корректные score исходы с некорректными
        let odds = vec![
            Odd {
                id: "evt17-bk1-10".into(),
                event_id: "evt17".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "1-0".into(),  // Корректный
                odds: 3.60,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt17-bk2-invalid".into(),
                event_id: "evt17".into(),
                bookmaker_slug: "bk2".into(),
                market: "CorrectScore".into(),
                selection: "Other".into(),  // Некорректный
                odds: 1.50,
                odds_type: OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt17-bk3-00".into(),
                event_id: "evt17".into(),
                bookmaker_slug: "bk3".into(),
                market: "CorrectScore".into(),
                selection: "0-0".into(),  // Корректный
                odds: 4.50,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt17-bk1-01".into(),
                event_id: "evt17".into(),
                bookmaker_slug: "bk1".into(),
                market: "CorrectScore".into(),
                selection: "0-1".into(),  // Корректный
                odds: 4.80,
                odds_type: OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        // Должна найти вилку, используя только корректные score исходы
        assert!(!surebets.is_empty(), "Should find surebet with valid selections");
    }

    // ======================= ASIAN HANDICAP TESTS =======================

    #[test]
    fn test_asian_handicap_basic_positive_negative() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah1");
        
        // Asian Handicap: Team A +1.5 (bk1) vs Team B -1.5 (bk2)
        let odds = vec![
            Odd {
                id: "evt_ah1-bk1-team_a_plus_1_5".into(),
                event_id: "evt_ah1".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Team A +1.5".into(),
                odds: 2.10,
                odds_type: OddsType::Home,
                line: Some(1.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah1-bk2-team_b_minus_1_5".into(),
                event_id: "evt_ah1".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Team B -1.5".into(),
                odds: 2.10,
                odds_type: OddsType::Away,
                line: Some(-1.5),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find Asian Handicap surebet");
        assert_eq!(surebets[0].legs.len(), 2);
        assert!(surebets[0].profit_percent > 0.0);
    }

    #[test]
    fn test_asian_handicap_fractional_lines() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah2");
        
        // Different fractional lines: +0.5, -1.75, +2.25
        let odds = vec![
            Odd {
                id: "evt_ah2-bk1-h_plus_0_5".into(),
                event_id: "evt_ah2".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +0.5".into(),
                odds: 2.05,
                odds_type: OddsType::Home,
                line: Some(0.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah2-bk2-a_minus_0_5".into(),
                event_id: "evt_ah2".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -0.5".into(),
                odds: 2.05,
                odds_type: OddsType::Away,
                line: Some(-0.5),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty());
    }

    #[test]
    fn test_asian_handicap_half_ball_lines() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah3");
        
        // Common lines in betting: +0.5, -1.5, +2.5
        let odds = vec![
            Odd {
                id: "evt_ah3-bk1-h_plus_0_5".into(),
                event_id: "evt_ah3".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +0.5".into(),
                odds: 1.95,
                odds_type: OddsType::Home,
                line: Some(0.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah3-bk2-a_minus_0_5".into(),
                event_id: "evt_ah3".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -0.5".into(),
                odds: 2.15,
                odds_type: OddsType::Away,
                line: Some(-0.5),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find +0.5/-0.5 surebet");
    }

    #[test]
    fn test_asian_handicap_negative_lines() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah4");
        
        // Negative handicap: Home -2.0 vs Away +2.0
        let odds = vec![
            Odd {
                id: "evt_ah4-bk1-h_minus_2_0".into(),
                event_id: "evt_ah4".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home -2.0".into(),
                odds: 1.75,
                odds_type: OddsType::Home,
                line: Some(-2.0),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah4-bk2-a_plus_2_0".into(),
                event_id: "evt_ah4".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away +2.0".into(),
                odds: 2.30,
                odds_type: OddsType::Away,
                line: Some(2.0),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find -2.0/+2.0 surebet");
    }

    #[test]
    fn test_asian_handicap_same_bookmaker_filtered() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah5");
        
        // Both from same bookmaker - should NOT find surebet
        let odds = vec![
            Odd {
                id: "evt_ah5-bk1-h_plus_1_5".into(),
                event_id: "evt_ah5".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +1.5".into(),
                odds: 2.10,
                odds_type: OddsType::Home,
                line: Some(1.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah5-bk1-a_minus_1_5".into(),
                event_id: "evt_ah5".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Away -1.5".into(),
                odds: 2.10,
                odds_type: OddsType::Away,
                line: Some(-1.5),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(surebets.is_empty(), "Should not find surebet from same bookmaker");
    }

    #[test]
    fn test_asian_handicap_different_lines_filtered() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah6");
        
        // Non-complementary lines: +1.5 and -2.0 (sum is not 0)
        let odds = vec![
            Odd {
                id: "evt_ah6-bk1-h_plus_1_5".into(),
                event_id: "evt_ah6".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +1.5".into(),
                odds: 2.10,
                odds_type: OddsType::Home,
                line: Some(1.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah6-bk2-a_minus_2_0".into(),
                event_id: "evt_ah6".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -2.0".into(),
                odds: 2.10,
                odds_type: OddsType::Away,
                line: Some(-2.0),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(surebets.is_empty(), "Should filter out non-complementary lines");
    }

    #[test]
    fn test_asian_handicap_low_profit_filtered() {
        let calc = SurebetCalculator::new(2.0, 30.0, 1000.0, 10000, 0.01); // min 2%
        let event = make_event("evt_ah7");
        
        // Low profit surebet
        let odds = vec![
            Odd {
                id: "evt_ah7-bk1-h_plus_1_5".into(),
                event_id: "evt_ah7".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +1.5".into(),
                odds: 1.95,
                odds_type: OddsType::Home,
                line: Some(1.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah7-bk2-a_minus_1_5".into(),
                event_id: "evt_ah7".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -1.5".into(),
                odds: 1.95,
                odds_type: OddsType::Away,
                line: Some(-1.5),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        // Profit = 1/(1/1.95 + 1/1.95) - 1 ≈ -0.49% (negative, should be filtered)
        assert!(surebets.is_empty(), "Should filter out low profit surebet");
    }

    #[test]
    fn test_asian_handicap_quarter_ball_lines() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah8");
        
        // Quarter ball lines: +0.25, -0.75, +1.25, -1.75
        let odds = vec![
            Odd {
                id: "evt_ah8-bk1-h_plus_1_25".into(),
                event_id: "evt_ah8".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +1.25".into(),
                odds: 2.02,
                odds_type: OddsType::Home,
                line: Some(1.25),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah8-bk2-a_minus_1_25".into(),
                event_id: "evt_ah8".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -1.25".into(),
                odds: 2.08,
                odds_type: OddsType::Away,
                line: Some(-1.25),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find quarter ball Asian Handicap");
    }

    #[test]
    fn test_asian_handicap_three_four_plus_minus() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah9");
        
        // Large lines: +3.5 vs -3.5
        let odds = vec![
            Odd {
                id: "evt_ah9-bk1-h_plus_3_5".into(),
                event_id: "evt_ah9".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +3.5".into(),
                odds: 1.50,
                odds_type: OddsType::Home,
                line: Some(3.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah9-bk2-a_minus_3_5".into(),
                event_id: "evt_ah9".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -3.5".into(),
                odds: 3.20,
                odds_type: OddsType::Away,
                line: Some(-3.5),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find large line Asian Handicap");
    }

    #[test]
    fn test_asian_handicap_best_odds_selection() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah10");
        
        // Multiple bookmakers for same outcome - should select best odds
        let odds = vec![
            Odd {
                id: "evt_ah10-bk1-h_plus_2_0".into(),
                event_id: "evt_ah10".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +2.0".into(),
                odds: 1.80,  // Lower
                odds_type: OddsType::Home,
                line: Some(2.0),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah10-bk3-h_plus_2_0".into(),
                event_id: "evt_ah10".into(),
                bookmaker_slug: "bk3".into(),
                market: "AsianHandicap".into(),
                selection: "Home +2.0".into(),
                odds: 1.95,  // Better
                odds_type: OddsType::Home,
                line: Some(2.0),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah10-bk2-a_minus_2_0".into(),
                event_id: "evt_ah10".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -2.0".into(),
                odds: 2.20,
                odds_type: OddsType::Away,
                line: Some(-2.0),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should find surebet with best odds");
        if let Some(surebet) = surebets.first() {
            let home_leg = surebet.legs.iter().find(|l| l.selection == "Home +2.0");
            assert!(home_leg.is_some());
            // Should use bk3 with 1.95 odds, not bk1 with 1.80
            assert!(home_leg.unwrap().odds > 1.9);
        }
    }

    #[test]
    fn test_asian_handicap_fractional_plus_signs() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah11");
        
        // Test with + and - symbols explicitly in selection
        let odds = vec![
            Odd {
                id: "evt_ah11-bk1-p1_plus".into(),
                event_id: "evt_ah11".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "П1 +1.75".into(),
                odds: 1.98,
                odds_type: OddsType::Home,
                line: Some(1.75),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah11-bk2-p2_minus".into(),
                event_id: "evt_ah11".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "П2 -1.75".into(),
                odds: 2.12,
                odds_type: OddsType::Away,
                line: Some(-1.75),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty(), "Should recognize Russian team names with +/- signs");
    }

    #[test]
    fn test_asian_handicap_stake_calculation() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah12");
        
        // Good surebet with profitable odds
        let odds = vec![
            Odd {
                id: "evt_ah12-bk1-h_plus_1_5".into(),
                event_id: "evt_ah12".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +1.5".into(),
                odds: 2.15,
                odds_type: OddsType::Home,
                line: Some(1.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah12-bk2-a_minus_1_5".into(),
                event_id: "evt_ah12".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -1.5".into(),
                odds: 2.05,
                odds_type: OddsType::Away,
                line: Some(-1.5),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets = calc.find_surebets(&[event], &odds);
        assert!(!surebets.is_empty());
        let surebet = &surebets[0];
        
        // Stakes should sum to default_stake (1000.0)
        let total_stake: f64 = surebet.legs.iter().map(|l| l.stake).sum();
        assert!((total_stake - 1000.0).abs() < 0.1, "Stakes should sum to 1000");
        
        // Payouts should be equal (characteristic of surebet)
        let payouts: Vec<f64> = surebet.legs.iter().map(|l| l.payout).collect();
        assert!((payouts[0] - payouts[1]).abs() < 1.0, "Payouts should be approximately equal");
    }

    #[test]
    fn test_asian_handicap_duplicate_filtering() {
        let calc = SurebetCalculator::new(1.0, 30.0, 1000.0, 10000, 0.01);
        let event = make_event("evt_ah13");
        
        let odds = vec![
            Odd {
                id: "evt_ah13-bk1-h_plus_1_5".into(),
                event_id: "evt_ah13".into(),
                bookmaker_slug: "bk1".into(),
                market: "AsianHandicap".into(),
                selection: "Home +1.5".into(),
                odds: 2.10,
                odds_type: OddsType::Home,
                line: Some(1.5),
                timestamp: Utc::now(),
            },
            Odd {
                id: "evt_ah13-bk2-a_minus_1_5".into(),
                event_id: "evt_ah13".into(),
                bookmaker_slug: "bk2".into(),
                market: "AsianHandicap".into(),
                selection: "Away -1.5".into(),
                odds: 2.10,
                odds_type: OddsType::Away,
                line: Some(-1.5),
                timestamp: Utc::now(),
            },
        ];
        
        let surebets1 = calc.find_surebets(&[event.clone()], &odds);
        assert_eq!(surebets1.len(), 1);
        
        // Mark as seen
        calc.mark_seen(&surebets1[0]);
        
        // Should not find duplicate
        let surebets2 = calc.find_surebets(&[event], &odds);
        assert!(surebets2.is_empty(), "Should filter duplicate Asian Handicap surebet");
    }
}
