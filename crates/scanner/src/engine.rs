use auto_betting::engine::AutoBetEngine;
use bankroll_manager::BankrollManager;
use bonus_hunter::BonusHunter;
use chrono::Utc;
use corridor_scanner::CorridorScanner;
use engine::calculator::SurebetCalculator;
use engine::event_pool::EventPool;
use engine::freebet::FreebetHunter;
use engine::generosity::GenerosityIndexCalc;
use engine::mirror::MirrorDetector;
use engine::momentum::MomentumScanner;
use engine::normalizer::Normalizer;
use engine::odds_errors::OddsErrorDetector;
use engine::value::ValueDetector;
use engine::verifier::OddsVerifier;
use express_forks::ExpressForkScanner;
use futures::future::join_all;
use parsers::base::{BookmakerParser, ParserResult};
use parsers::circuit_breaker::CircuitBreaker;
use parking_lot::RwLock;
use shared::models::ScannerMetrics;
use shared::{EventBus, Event, Odd, BusEvent};
use shared::{ExpressFork, CorridorOpportunity, BonusInfo, ValueBet};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::watch;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ScannerState {
    pub running: bool,
    pub last_metrics: Option<ScannerMetrics>,
    pub cycle_count: u64,
}

/// Pipeline кэш — данные парсеров обновляются в фоне, калькулятор читает мгновенно
#[derive(Clone)]
struct PipelineCache {
    pub events: Arc<RwLock<Vec<Event>>>,
    pub odds: Arc<RwLock<Vec<Odd>>>,
    pub last_update: Arc<RwLock<Instant>>,
    pub is_fresh: Arc<RwLock<bool>>,
}

impl PipelineCache {
    fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            odds: Arc::new(RwLock::new(Vec::new())),
            last_update: Arc::new(RwLock::new(Instant::now())),
            is_fresh: Arc::new(RwLock::new(false)),
        }
    }

    fn update(&self, events: Vec<Event>, odds: Vec<Odd>) {
        *self.events.write() = events;
        *self.odds.write() = odds;
        *self.last_update.write() = Instant::now();
        *self.is_fresh.write() = true;
    }

    fn clone_data(&self) -> (Vec<Event>, Vec<Odd>) {
        let events = self.events.read().clone();
        let odds = self.odds.read().clone();
        (events, odds)
    }

    fn take(&self) -> (Vec<Event>, Vec<Odd>) {
        self.clone_data()
    }

    fn age_ms(&self) -> u128 {
        self.last_update.read().elapsed().as_millis()
    }
}

#[derive(Clone)]
pub struct GhostScanner {
    pub parsers: Vec<Arc<dyn BookmakerParser + Send + Sync>>,
    pub calculator: Arc<SurebetCalculator>,
    pub normalizer: Arc<Normalizer>,
    pub event_pool: Arc<EventPool>,
    pub freebet_hunter: Arc<FreebetHunter>,
    pub generosity_index: Arc<GenerosityIndexCalc>,
    pub mirror_detector: Arc<MirrorDetector>,
    pub momentum_scanner: Arc<MomentumScanner>,
    pub odds_error_detector: Arc<OddsErrorDetector>,
    pub value_detector: Arc<ValueDetector>,
    pub odds_verifier: Arc<OddsVerifier>,
    pub corridor_scanner: Arc<CorridorScanner>,
    pub express_fork_scanner: Arc<ExpressForkScanner>,
    pub bankroll_manager: Arc<BankrollManager>,
    pub bonus_hunter: Arc<BonusHunter>,
    pub auto_bet_engine: Arc<AutoBetEngine>,
    pub circuit_breakers: Arc<Mutex<HashMap<String, CircuitBreaker>>>,
    pub event_bus: Arc<EventBus>,
    pub scan_interval_secs: u64,
    pub running: Arc<Mutex<bool>>,
    pub state_tx: watch::Sender<ScannerState>,
    pub state_rx: watch::Receiver<ScannerState>,
    // Pipeline кэш для мгновенного доступа калькулятора
    pipeline_cache: PipelineCache,
}

impl GhostScanner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        parsers: Vec<Arc<dyn BookmakerParser + Send + Sync>>,
        calculator: Arc<SurebetCalculator>,
        normalizer: Arc<Normalizer>,
        event_pool: Arc<EventPool>,
        freebet_hunter: Arc<FreebetHunter>,
        generosity_index: Arc<GenerosityIndexCalc>,
        mirror_detector: Arc<MirrorDetector>,
        momentum_scanner: Arc<MomentumScanner>,
        odds_error_detector: Arc<OddsErrorDetector>,
        value_detector: Arc<ValueDetector>,
        odds_verifier: Arc<OddsVerifier>,
        corridor_scanner: Arc<CorridorScanner>,
        express_fork_scanner: Arc<ExpressForkScanner>,
        bankroll_manager: Arc<BankrollManager>,
        bonus_hunter: Arc<BonusHunter>,
        auto_bet_engine: Arc<AutoBetEngine>,
        event_bus: Arc<EventBus>,
        scan_interval_secs: u64,
    ) -> Self {
        let mut circuit_breakers = HashMap::new();
        for parser in &parsers {
            circuit_breakers.insert(
                parser.slug().to_string(),
                CircuitBreaker::new(5, 300, 3),
            );
        }

        let (state_tx, state_rx) = watch::channel(ScannerState {
            running: false,
            last_metrics: None,
            cycle_count: 0,
        });

        let pipeline_cache = PipelineCache::new();

        Self {
            parsers,
            calculator,
            normalizer,
            event_pool,
            freebet_hunter,
            generosity_index,
            mirror_detector,
            momentum_scanner,
            odds_error_detector,
            value_detector,
            odds_verifier,
            corridor_scanner,
            express_fork_scanner,
            bankroll_manager,
            bonus_hunter,
            auto_bet_engine,
            circuit_breakers: Arc::new(Mutex::new(circuit_breakers)),
            event_bus,
            scan_interval_secs,
            running: Arc::new(Mutex::new(false)),
            state_tx,
            state_rx,
            pipeline_cache,
        }
    }

    pub async fn run_cycle(&self) -> ScannerMetrics {
        self.run_cycle_inner().await
    }

    pub async fn run_cycle_inner(&self) -> ScannerMetrics {
        debug!("Cycle starting...");
        let cycle_start = Instant::now();

        // PIPELINE MODE: 
        // 1. Если кэш пуст — запускаем fetch и ЖДЁМ первый результат
        // 2. Если кэш заполнен — мгновенно обрабатываем
        // 3. Параллельно запускаем следующий fetch в фоне
        
        let (events, all_odds) = self.pipeline_cache.clone_data();
        let event_count = events.len();
        let odd_count = all_odds.len();
        
        if event_count == 0 || odd_count == 0 {
            // ПЕРВЫЙ ЗАПУСК: ждём первый fetch
            info!("Pipeline cache empty — fetching parsers for first time...");
            let cache = self.pipeline_cache.clone();
            let parsers = self.parsers.clone();
            let breakers = self.circuit_breakers.clone();
            
            // Fetch parsers synchronously for first cycle
            info!("Pipeline cache empty — fetching parsers for first time...");
            let cache = self.pipeline_cache.clone();
            let parsers = self.parsers.clone();
            let breakers = self.circuit_breakers.clone();
            let results = Self::fetch_parsers_parallel(&parsers, &breakers).await;
            let fetched_odds: Vec<Odd> = results.iter().flat_map(|e| e.odds.clone()).collect();
            let fetched_events: Vec<Event> = results.iter().flat_map(|e| e.events.clone()).collect();
            
            // DEBUG: покажем что вернули парсеры
            info!("📊 Parser results: {} events, {} odds from {} parsers", 
                  fetched_events.len(), fetched_odds.len(), results.len());
            for result in &results {
                info!("  - {} events, {} odds", result.events.len(), result.odds.len());
            }
            
            cache.update(fetched_events.clone(), fetched_odds.clone());
            info!("First fetch complete: {} events, {} odds", fetched_events.len(), fetched_odds.len());
            
            // Now process
            self.process_events(fetched_events, fetched_odds, cycle_start)
        } else {
            // КЭШ ГОТОВ: запускаем фоновое обновление и обрабатываем мгновенно
            let age = self.pipeline_cache.age_ms();
            debug!("Processing cached data (age={}ms): {} events, {} odds", 
                   age, event_count, odd_count);
            
            // Запускаем fetch в фоне для СЛЕДУЮЩЕГО цикла
            let cache = self.pipeline_cache.clone();
            let parsers = self.parsers.clone();
            let breakers = self.circuit_breakers.clone();
            tokio::spawn(async move {
                let results = Self::fetch_parsers_parallel(&parsers, &breakers).await;
                let new_odds: Vec<Odd> = results.iter().flat_map(|e| e.odds.clone()).collect();
                let new_events: Vec<Event> = results.iter().flat_map(|e| e.events.clone()).collect();
                cache.update(new_events, new_odds);
            });
            
            self.process_events(events, all_odds, cycle_start)
        }
    }

    /// Быстрая обработка событий из кэша — КЛЮЧЕВОЕ ИЗМЕНЕНИЕ
    /// Группируем события по матчу (home+away+sport), потом ищем вилки МЕЖДУ БК
    fn process_events(&self, events: Vec<Event>, all_odds: Vec<Odd>, cycle_start: Instant) -> ScannerMetrics {
        // HIGH PERFORMANCE: 5000 событий, 100K odds
        const MAX_EVENTS: usize = 5000;
        const MAX_ODDS: usize = 100_000;

        let calc_events: Vec<Event> = events.into_iter().take(MAX_EVENTS).collect();
        let calc_odds: Vec<Odd> = all_odds.into_iter().take(MAX_ODDS).collect();

        let normalized: Vec<Event> = calc_events.iter()
            .map(|e| self.normalizer.normalize_event(e.clone()))
            .collect();

        for event in &normalized {
            self.event_pool.insert(event.clone());
        }

        // КЛЮЧЕВОЙ ШАГ: группируем события по fingerprint матча
        let mut matches: HashMap<String, Vec<&Event>> = HashMap::new();
        for event in &normalized {
            let fp = Self::event_fingerprint(event);
            matches.entry(fp).or_default().push(event);
        }

        // Группируем odds по fingerprint
        // КЛЮЧЕВОЕ ИЗМЕНЕНИЕ: используем HashMap для быстрого поиска по event_id
        let mut event_by_id: HashMap<String, &Event> = HashMap::with_capacity(normalized.len());
        for event in &normalized {
            event_by_id.insert(event.id.clone(), event);
        }
        
        let mut odds_by_match: HashMap<String, Vec<&Odd>> = HashMap::new();
        for odd in &calc_odds {
            if let Some(event) = event_by_id.get(&odd.event_id) {
                let fp = Self::event_fingerprint(event);
                odds_by_match.entry(fp).or_default().push(odd);
            }
        }

        // DEBUG: print stats
        let total_odds_with_fp = odds_by_match.values().map(|v| v.len()).sum::<usize>();
        let matches_with_odds = odds_by_match.len();
        let matches_with_multi_bk = odds_by_match.iter()
            .filter(|(_, odds)| {
                let bks: std::collections::HashSet<&str> = odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
                bks.len() >= 2
            })
            .count();
        
        info!("📊 {} events → {} matches ({} with odds, {} with 2+ BK), {} total odds",
              calc_events.len(), matches.len(), matches_with_odds, matches_with_multi_bk, calc_odds.len());

        // Show first 5 matches with multi-BK odds
        let mut shown = 0;
        for (fp, match_events) in &matches {
            if shown >= 5 { break; }
            if let Some(odds) = odds_by_match.get(fp) {
                let bks: std::collections::HashSet<&str> = odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
                if bks.len() >= 2 {
                    // Show market distribution
                    let markets: std::collections::HashMap<String, usize> = {
                        let mut m = std::collections::HashMap::new();
                        for o in odds {
                            *m.entry(o.market.clone()).or_insert(0) += 1;
                        }
                        m
                    };
                    let top_markets: Vec<_> = markets.iter()
                        .map(|(k,v)| format!("{}:{}", k, v))
                        .take(5)
                        .collect();
                    
                    info!("🎯 Match #{} '{}' — {} odds from {} BKs {:?} | Markets: {:?}",
                          shown + 1, fp, odds.len(), bks.len(), bks, top_markets);
                    shown += 1;
                }
            }
        }

        // Ищем вилки ДЛЯ КАЖДОГО матча — между ВСЕМИ БК
        let mut total_surebets = 0;
        let mut checked_matches = 0;
        
        for (fp, match_events) in &matches {
            // Собираем все odds для этого матча от ВСЕХ БК
            let match_odds = odds_by_match.get(fp).cloned().unwrap_or_default();
            if match_odds.len() < 2 { continue; } // Нужны odds от 2+ БК

            checked_matches += 1;
            if checked_matches <= 3 {
                // Debug first 3 matches
                let bks: std::collections::HashSet<&str> = match_odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
                info!("Match #{} '{}' — {} odds from {} bookmakers: {:?}", 
                      checked_matches, fp, match_odds.len(), bks.len(), bks);
            }

            // Ищем вилки между разными БК для этого матча
            let surebets = self.find_cross_bk_surebets(match_events, &match_odds);
            if !surebets.is_empty() {
                info!("Found {} surebets for match '{}'", surebets.len(), fp);
            }
            total_surebets += surebets.len();

            for surebet in &surebets {
                let payload = serde_json::to_value(surebet).unwrap_or_default();
                let _ = self.event_bus.publish(BusEvent::SurebetFound {
                    surebet_id: surebet.id.to_string(),
                    payload,
                    timestamp: Utc::now(),
                });
            }
        }

        // Fallback: старый метод для сравнения
        let legacy_surebets = self.calculator.find_surebets(&normalized, &calc_odds);
        total_surebets += legacy_surebets.len();

        let cycle_time = cycle_start.elapsed().as_millis() as u64;
        
        let active = {
            let breakers = self.circuit_breakers.lock().unwrap();
            breakers.values().filter(|cb| cb.allow_request()).count()
        };
        let failed = self.parsers.len() - active;

        let metrics = ScannerMetrics {
            cycle_time_ms: cycle_time,
            events_parsed: normalized.len(),
            surebets_found: total_surebets,
            active_bookmakers: active,
            failed_bookmakers: failed,
            cache_hit_rate: 0.0,
            memory_mb: 0.0,
            timestamp: Utc::now(),
        };
        
        info!(cycle_ms = cycle_time, events = normalized.len(), odds = calc_odds.len(), 
              matches = matches.len(), surebets = total_surebets, "Cycle complete");

        let next_cycle_count = self.state_rx.borrow().cycle_count + 1;
        self.state_tx.send_replace(ScannerState {
            running: true,
            last_metrics: Some(metrics.clone()),
            cycle_count: next_cycle_count,
        });

        metrics
    }

    /// Fingerprint матча для матчинга между БК — РОБАСТНАЯ нормализация + FUZZY
    fn event_fingerprint(event: &Event) -> String {
        let norm = engine::normalizer::Normalizer::new();
        let norm_event = norm.normalize_event(event.clone());
        
        // Нормализуем команды: уже нормализованы Normalizer, дочищаем
        let home = Self::normalize_team_name(&norm_event.home_team);
        let away = Self::normalize_team_name(&norm_event.away_team);
        // Лига уже нормализована (напр "Premier League"), приводим к lowercase для fingerprint
        let league = norm_event.league.to_lowercase().replace(" ", "");
        
        // Sort to ensure consistent ordering regardless of home/away order
        let (first, second) = if home < away { (home, away) } else { (away, home) };
        // Включаем лигу для лучшей точности (меньше ложных матчей)
        format!("{:?}|{}|{}|{}", event.sport, league, first, second)
    }

    /// Робастная нормализация названия команды
    fn normalize_team_name(name: &str) -> String {
        let mut s = name.to_lowercase()
            // Remove common prefixes
            .replace("фк ", "").replace("ск ", "").replace("пк ", "")
            .replace("фк", "").replace("ск", "").replace("пк", "")
            .replace("хк ", "").replace("хк", "")
            .replace("фс ", "").replace("фс", "")
            .replace("гк ", "").replace("гк", "")
            .replace("фк ", "").replace("фк", "")
            .replace("футбольный клуб ", "")
            // Remove common suffixes
            .replace(" москва", "").replace(" спб", "").replace(" спб.", "")
            .replace(" санкт-петербург", "").replace(" с.-петербург", "")
            .replace(" петербург", "").replace(" питер", "")
            .replace(" нижний новгород", "").replace(" новгород", "")
            // Remove punctuation and extra chars
            .replace("(", "").replace(")", "").replace("-", " ")
            .replace(".", "").replace(",", "").replace("'", "")
            .replace("\"", "").replace("_", " ")
            .trim().to_string();

        // Collapse multiple spaces
        while s.contains("  ") {
            s = s.replace("  ", " ");
        }
        s.trim().to_string()
    }

    /// Ищем вилки МЕЖДУ разными БК для одного матча
    fn find_cross_bk_surebets(&self, events: &[&Event], odds: &[&Odd]) -> Vec<shared::Surebet> {
        use shared::Surebet;
        use shared::SurebetLeg;
        use uuid::Uuid;

        let mut surebets = Vec::new();

        if odds.is_empty() { return surebets; }

        // Считаем уникальные БК
        let unique_bks: std::collections::HashSet<&str> = odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
        if unique_bks.len() < 2 {
            return surebets; // Нужны минимум 2 БК
        }

        // Debug: первые 10 odds
        if odds.len() <= 30 {
            let sample: Vec<String> = odds.iter().take(10).map(|o|
                format!("{}:{}@{:.2}", o.bookmaker_slug, o.selection, o.odds)
            ).collect();
            debug!("Checking {} odds from {} BKs: {:?}", odds.len(), unique_bks.len(), sample);
        }

        // Группируем odds по market+line
        let mut by_market: HashMap<String, Vec<&&Odd>> = HashMap::new();
        for odd in odds {
            let line_key = odd.line.map(|l| format!("{:.1}", l)).unwrap_or_default();
            let key = format!("{}|{}", odd.market.to_lowercase(), line_key);
            by_market.entry(key).or_default().push(odd);
        }

        // Debug: покажем топ рынков
        let mut market_counts: Vec<_> = by_market.iter().map(|(k, v)| (k, v.len())).collect();
        market_counts.sort_by(|a, b| b.1.cmp(&a.1));
        if !market_counts.is_empty() {
            debug!("Top markets: {:?}", &market_counts[..market_counts.len().min(5)]);
        }

        // Для каждого рынка ищем вилки между БК
        for (market, market_odds) in &by_market {
            let lower = market.to_lowercase();
            
            if lower.starts_with("1x2") || lower.starts_with("исход") || lower.starts_with("match") {
                // 3-way: ищем 1, X, 2 от разных БК
                let ones: Vec<&&Odd> = market_odds.iter().filter(|o| o.selection == "1" || o.selection.to_lowercase() == "п1").cloned().collect();
                let xs: Vec<&&Odd> = market_odds.iter().filter(|o| o.selection == "X" || o.selection.to_lowercase() == "х").cloned().collect();
                let twos: Vec<&&Odd> = market_odds.iter().filter(|o| o.selection == "2" || o.selection.to_lowercase() == "п2").cloned().collect();

                if ones.is_empty() || xs.is_empty() || twos.is_empty() { 
                    if market_odds.len() >= 3 {
                        info!("Market '{}' has {} odds but missing outcomes: 1={}, X={}, 2={}", 
                              market, market_odds.len(), ones.len(), xs.len(), twos.len());
                    }
                    continue; 
                }
                
                info!("Market '{}' — {}x1, {}xX, {}x2 from different BKs", market, ones.len(), xs.len(), twos.len());

                for &o1 in &ones {
                    for &ox in &xs {
                        for &o2 in &twos {
                            // Проверяем что минимум 2 разные БК
                            let bks: std::collections::HashSet<&str> = [o1.bookmaker_slug.as_str(), ox.bookmaker_slug.as_str(), o2.bookmaker_slug.as_str()].iter().cloned().collect();
                            if bks.len() < 2 { continue; }

                            if let Some(profit) = shared::odds::calculate_surebet_profit(&[o1.odds, ox.odds, o2.odds]) {
                                if profit < self.calculator.min_profit { continue; } // Используем конфиг

                                let stakes = shared::odds::calculate_stakes(&[o1.odds, ox.odds, o2.odds], 1000.0);
                                let payout = stakes[0] * o1.odds;

                                let first_event = events.first().copied().unwrap();
                                surebets.push(Surebet {
                                    id: Uuid::new_v4(),
                                    sport: first_event.sport,
                                    league: first_event.league.clone(),
                                    home_team: first_event.home_team.clone(),
                                    away_team: first_event.away_team.clone(),
                                    start_time: first_event.start_time,
                                    is_live: first_event.is_live,
                                    profit_percent: profit,
                                    total_stake: 1000.0,
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
                }
            } else if lower.starts_with("total") || lower.starts_with("тотал") {
                // 2-way: Over/Under с той же линией от разных БК
                let overs: Vec<&&Odd> = market_odds.iter()
                    .filter(|o| o.selection.to_lowercase().contains("over") || o.selection.to_lowercase().contains("больше") || o.selection.to_lowercase() == "тб")
                    .cloned().collect();
                let unders: Vec<&&Odd> = market_odds.iter()
                    .filter(|o| o.selection.to_lowercase().contains("under") || o.selection.to_lowercase().contains("меньше") || o.selection.to_lowercase() == "тм")
                    .cloned().collect();

                for &o_over in &overs {
                    for &o_under in &unders {
                        if o_over.bookmaker_slug == o_under.bookmaker_slug { continue; }
                        
                        // Проверяем что линии совпадают
                        if let (Some(l1), Some(l2)) = (o_over.line, o_under.line) {
                            if (l1 - l2).abs() > 0.1 { continue; }
                        }

                        if let Some(profit) = shared::odds::calculate_surebet_profit(&[o_over.odds, o_under.odds]) {
                            if profit < self.calculator.min_profit { continue; } // Используем конфиг

                            let stakes = shared::odds::calculate_stakes(&[o_over.odds, o_under.odds], 1000.0);
                            let payout = stakes[0] * o_over.odds;

                            let first_event = events.first().copied().unwrap();
                            surebets.push(Surebet {
                                id: Uuid::new_v4(),
                                sport: first_event.sport,
                                league: first_event.league.clone(),
                                home_team: first_event.home_team.clone(),
                                away_team: first_event.away_team.clone(),
                                start_time: first_event.start_time,
                                is_live: first_event.is_live,
                                profit_percent: profit,
                                total_stake: 1000.0,
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
        }

        surebets
    }

    /// Параллельный fetch всех парсеров — вынесен в отдель метод
    async fn fetch_parsers_parallel(
        parsers: &[Arc<dyn BookmakerParser + Send + Sync>],
        breakers: &Arc<Mutex<HashMap<String, CircuitBreaker>>>,
    ) -> Vec<ParserResult> {
        use futures::future::join_all;
        
        let mut futures = Vec::new();
        
        for parser in parsers.iter() {
            let slug = parser.slug().to_string();
            let parser = parser.clone();
            let breakers = breakers.clone();

            let skip = {
                let brk = breakers.lock().unwrap();
                brk.get(&slug).map(|cb| !cb.allow_request()).unwrap_or(false)
            };

            if skip { continue; }

            let fut = async move {
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    parser.fetch_all()
                ).await;

                match result {
                    Ok(Ok(r)) => Some(r),
                    Ok(Err(e)) => {
                        warn!(parser = %slug, error = %e, "Parser error");
                        None
                    }
                    Err(_) => {
                        warn!(parser = %slug, "Parser timeout");
                        None
                    }
                }
            };
            futures.push(fut);
        }

        let results: Vec<Option<_>> = join_all(futures).await;
        results.into_iter().flatten().collect()
    }

    pub async fn start(&self) {
        info!("Scanner starting...");
        {
            let mut running = self.running.lock().unwrap();
            *running = true;
        }
        info!("GhostScanner started, entering scan loop");

        loop {
            let is_running = { *self.running.lock().unwrap() };
            if !is_running {
                info!("GhostScanner loop exiting");
                break;
            }

            debug!("Starting scan cycle");
            let cycle_result = tokio::time::timeout(
                std::time::Duration::from_secs(180),
                self.run_cycle_inner()
            ).await;

            let metrics = match cycle_result {
                Ok(m) => m,
                Err(_) => {
                    warn!("Scan cycle TIMED OUT after 180s");
                    let metrics: ScannerMetrics = ScannerMetrics {
                        cycle_time_ms: 180000,
                        events_parsed: 0,
                        surebets_found: 0,
                        active_bookmakers: 0,
                        failed_bookmakers: 0,
                        cache_hit_rate: 0.0,
                        memory_mb: 0.0,
                        timestamp: Utc::now(),
                    };
                    metrics
                }
            };
            info!(
                cycle_ms = metrics.cycle_time_ms,
                events = metrics.events_parsed,
                surebets = metrics.surebets_found,
                "Scan cycle completed"
            );

            // Update global scanner state for API without holding a watch borrow during send
            let current = self.state_rx.borrow().clone();
            self.state_tx.send_replace(ScannerState {
                running: current.running,
                last_metrics: Some(metrics.clone()),
                cycle_count: current.cycle_count + 1,
            });
            debug!(
                cycle_ms = metrics.cycle_time_ms,
                events = metrics.events_parsed,
                surebets = metrics.surebets_found,
                "Scan cycle completed"
            );

            let _ = self.event_bus.publish(shared::BusEvent::SystemAlert {
                level: "info".into(),
                message: format!("Cycle: {}ms, {} events, {} opportunities",
                    metrics.cycle_time_ms, metrics.events_parsed, metrics.surebets_found),
                timestamp: Utc::now(),
            }).await;

            tokio::time::sleep(std::time::Duration::from_secs(self.scan_interval_secs)).await;
        }

        info!("GhostScanner stopped");
    }

    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    pub fn get_corridors(&self, limit: usize) -> Vec<CorridorOpportunity> {
        self.corridor_scanner.get_recent(limit)
    }

    pub fn get_express_forks(&self, limit: usize) -> Vec<ExpressFork> {
        self.express_fork_scanner.get_recent(limit)
    }

    pub fn get_value_bets(&self, events: &[Event], all_odds: &[Odd]) -> Vec<ValueBet> {
        self.value_detector.detect_values(events, all_odds)
    }

    pub fn get_best_bonuses(&self, limit: usize) -> Vec<BonusInfo> {
        self.bonus_hunter.get_best_bonuses(limit)
    }
}
