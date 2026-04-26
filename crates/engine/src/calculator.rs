use bloomfilter::Bloom;
use chrono::Utc;
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock;
use shared::odds::{calculate_stakes, calculate_surebet_profit};
use shared::{Event, Odd, Surebet, SurebetLeg};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

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
        Self::with_early_termination(
            min_profit,
            max_profit,
            default_stake,
            capacity,
            error_rate,
            min_profit * 3.0,
        )
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
                                debug!(
                                    "Early termination: profit {:.2}% exceeds ROI threshold {:.2}%",
                                    surebet.profit_percent, self.early_termination_roi
                                );
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
                (odd.odds * 10000.0).round() / 10000.0 // Round to 4 decimals for comparison
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
            cache.put(
                cache_key,
                ComboCache {
                    profit,
                    stakes: profit.and_then(|_| Some(calculate_stakes(odds, self.default_stake))),
                },
            );
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

    /// Группируем odds по рынку (market + line) для поиска вилок
    fn group_by_market<'a>(&self, odds: &'a [Odd]) -> HashMap<String, Vec<&'a Odd>> {
        let mut map: HashMap<String, Vec<&'a Odd>> = HashMap::new();
        for odd in odds {
            // Нормализуем название рынка для группировки Over/Under вместе
            let normalized_market = self.normalize_market_key(&odd.market, odd.line);
            map.entry(normalized_market).or_default().push(odd);
        }
        map
    }

    /// Нормализует ключ рынка: Over/Under → Total, учитывает line
    fn normalize_market_key(&self, market: &str, line: Option<f64>) -> String {
        let m = market.to_lowercase();
        let base = if m.contains("over") || m.contains("under")
            || m.contains("тб") || m.contains("тм")
            || m.contains("total")
        {
            "total"
        } else if m.contains("handicap") || m.contains("фора") || m.contains("asian") {
            "handicap"
        } else if m.contains("1x2") || m.contains("match") || m.contains("winner") {
            "1x2"
        } else {
            market
        };

        match line {
            Some(l) => format!("{}_{:.2}", base, l),
            None => base.to_string(),
        }
    }

    /// Ищем вилку для конкретного рынка между разными БК
    fn find_market_surebet(
        &self,
        event: &Event,
        _market_key: &str,
        odds: &[&Odd],
    ) -> Option<Surebet> {
        // Группируем по selection (исходу)
        let mut by_selection: HashMap<String, Vec<&Odd>> = HashMap::new();
        for odd in odds {
            let sel = odd.selection.to_lowercase();
            by_selection.entry(sel).or_default().push(odd);
        }

        let selections: Vec<String> = by_selection.keys().cloned().collect();
        if selections.len() < 2 {
            return None;
        }

        // Для 2-way: берём лучший кэф по каждому исходу с разных БК
        if selections.len() == 2 {
            let sel_a = &selections[0];
            let sel_b = &selections[1];

            let best_a = by_selection[sel_a]
                .iter()
                .max_by(|x, y| x.odds.partial_cmp(&y.odds).unwrap())?;
            let best_b = by_selection[sel_b]
                .iter()
                .max_by(|x, y| x.odds.partial_cmp(&y.odds).unwrap())?;

            // Не берём вилку с одного БК
            if best_a.bookmaker_slug == best_b.bookmaker_slug {
                return None;
            }

            let profit = self.calculate_profit_cached(&[best_a.odds, best_b.odds])?;
            if profit < self.min_profit || profit > self.max_profit {
                return None;
            }

            let stakes = self.calculate_stakes_cached(&[best_a.odds, best_b.odds]);
            let total_stake = self.default_stake;

            return Some(Surebet {
                id: Uuid::new_v4(),
                sport: event.sport,
                league: event.league.clone(),
                home_team: event.home_team.clone(),
                away_team: event.away_team.clone(),
                start_time: event.start_time,
                is_live: event.is_live,
                profit_percent: profit,
                total_stake,
                legs: vec![
                    SurebetLeg {
                        bookmaker: best_a.bookmaker_slug.clone(),
                        market: best_a.market.clone(),
                        selection: best_a.selection.clone(),
                        odds: best_a.odds,
                        line: best_a.line,
                        stake: stakes.first().copied().unwrap_or(total_stake / 2.0),
                        payout: stakes.first().copied().unwrap_or(total_stake / 2.0) * best_a.odds,
                        url: None,
                    },
                    SurebetLeg {
                        bookmaker: best_b.bookmaker_slug.clone(),
                        market: best_b.market.clone(),
                        selection: best_b.selection.clone(),
                        odds: best_b.odds,
                        line: best_b.line,
                        stake: stakes.get(1).copied().unwrap_or(total_stake / 2.0),
                        payout: stakes.get(1).copied().unwrap_or(total_stake / 2.0) * best_b.odds,
                        url: None,
                    },
                ],
                detected_at: Utc::now(),
                verified: false,
                mirror: false,
            });
        }

        // Для 3-way: 1X2
        if selections.len() == 3 {
            let mut best_per_sel: Vec<(&str, &Odd)> = Vec::new();
            for sel in &selections {
                if let Some(best) = by_selection[sel]
                    .iter()
                    .max_by(|x, y| x.odds.partial_cmp(&y.odds).unwrap())
                {
                    best_per_sel.push((sel.as_str(), best));
                }
            }

            if best_per_sel.len() < 3 {
                return None;
            }

            // Проверяем что не все с одного БК
            let bks: HashSet<&str> = best_per_sel.iter().map(|(_, o)| o.bookmaker_slug.as_str()).collect();
            if bks.len() < 2 {
                return None;
            }

            let odds_vals: Vec<f64> = best_per_sel.iter().map(|(_, o)| o.odds).collect();
            let profit = self.calculate_profit_cached(&odds_vals)?;
            if profit < self.min_profit || profit > self.max_profit {
                return None;
            }

            let stakes = self.calculate_stakes_cached(&odds_vals);
            let total_stake = self.default_stake;

            let legs: Vec<SurebetLeg> = best_per_sel
                .iter()
                .enumerate()
                .map(|(i, (_, o))| SurebetLeg {
                    bookmaker: o.bookmaker_slug.clone(),
                    market: o.market.clone(),
                    selection: o.selection.clone(),
                    odds: o.odds,
                    line: o.line,
                    stake: stakes.get(i).copied().unwrap_or(total_stake / 3.0),
                    payout: stakes.get(i).copied().unwrap_or(total_stake / 3.0) * o.odds,
                    url: None,
                })
                .collect();

            return Some(Surebet {
                id: Uuid::new_v4(),
                sport: event.sport,
                league: event.league.clone(),
                home_team: event.home_team.clone(),
                away_team: event.away_team.clone(),
                start_time: event.start_time,
                is_live: event.is_live,
                profit_percent: profit,
                total_stake,
                legs,
                detected_at: Utc::now(),
                verified: false,
                mirror: false,
            });
        }

        None
    }

    /// Генерирует уникальный ключ для вилки (для bloom filter дедупликации)
    fn surebet_key(&self, surebet: &Surebet) -> Vec<u8> {
        let mut parts: Vec<String> = surebet
            .legs
            .iter()
            .map(|leg| {
                format!(
                    "{}|{}|{}|{:.4}",
                    leg.bookmaker,
                    leg.market.to_lowercase(),
                    leg.selection.to_lowercase(),
                    (leg.odds * 10000.0).round() / 10000.0
                )
            })
            .collect();
        // Сортируем для стабильного ключа независимо от порядка ног
        parts.sort();
        parts.join("::").into_bytes()
    }
}
