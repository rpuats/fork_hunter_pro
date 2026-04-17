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
use parking_lot::RwLock;
use parsers::base::{BookmakerParser, ParserResult};
use parsers::circuit_breaker::{CircuitBreaker, CircuitState};
use persistence::freebet_lifecycle::FreebetLifecycleStore;
use persistence::history::SurebetHistory;
use shared::config::{FeatureFlags, ParserResultCaps, RuntimeProfile, ScannerConfig};
use shared::models::{ParserRuntimeSnapshot, RuntimeCircuitState, ScannerMetrics};
use shared::odds::OddsType;
use shared::Sport;
use shared::{BonusInfo, CorridorOpportunity, ExpressFork, OddsError};
use shared::{BusEvent, Event, EventBus, Odd};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::watch;
use tracing::{debug, info, warn};

use crate::freebet_lifecycle::{collect_freebet_lifecycle, persist_freebet_lifecycle_states};
use crate::parser_bulkhead::ParserExecutionBulkhead;
use crate::parser_result_validator::validate_parser_result;
use crate::runtime_metrics::ParserRuntimeStats;

#[derive(Debug, Clone)]
pub struct ScannerState {
    pub running: bool,
    pub last_metrics: Option<ScannerMetrics>,
    pub cycle_count: u64,
}

fn map_circuit_state(state: CircuitState) -> RuntimeCircuitState {
    match state {
        CircuitState::Closed => RuntimeCircuitState::Closed,
        CircuitState::HalfOpen => RuntimeCircuitState::HalfOpen,
        CircuitState::Open => RuntimeCircuitState::Open,
    }
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
    pub history: Arc<SurebetHistory>,
    pub freebet_lifecycle_store: Option<Arc<FreebetLifecycleStore>>,
    pub circuit_breakers: Arc<Mutex<HashMap<String, CircuitBreaker>>>,
    pub event_bus: Arc<EventBus>,
    pub runtime_profile: RuntimeProfile,
    pub feature_flags: FeatureFlags,
    pub scan_interval_secs: u64,
    pub request_timeout_secs: u64,
    pub per_bookmaker_timeout_secs: Arc<HashMap<String, u64>>,
    pub running: Arc<Mutex<bool>>,
    pub state_tx: watch::Sender<ScannerState>,
    pub state_rx: watch::Receiver<ScannerState>,
    parser_runtime: Arc<RwLock<HashMap<String, ParserRuntimeStats>>>,
    parser_execution_bulkhead: Arc<ParserExecutionBulkhead>,
    parser_result_caps: ParserResultCaps,
    // Pipeline кэш для мгновенного доступа калькулятора
    pipeline_cache: PipelineCache,
    // Кэш найденных вилок для API (ключ дедупликации → вилка)
    surebets_cache: Arc<parking_lot::RwLock<std::collections::HashMap<String, shared::Surebet>>>,
    // Кэш value bets
    value_bets_cache: Arc<parking_lot::RwLock<Vec<shared::ValueBet>>>,
    // Кэш ошибок в коэффициентах
    odds_errors_cache: Arc<parking_lot::RwLock<Vec<OddsError>>>,
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
        history: Arc<SurebetHistory>,
        event_bus: Arc<EventBus>,
        runtime_profile: RuntimeProfile,
        feature_flags: FeatureFlags,
        scan_interval_secs: u64,
        request_timeout_secs: u64,
        per_bookmaker_timeout_secs: HashMap<String, u64>,
    ) -> Self {
        let mut circuit_breakers = HashMap::new();
        for parser in &parsers {
            circuit_breakers.insert(parser.slug().to_string(), CircuitBreaker::new(5, 300, 3));
        }

        let (state_tx, state_rx) = watch::channel(ScannerState {
            running: false,
            last_metrics: None,
            cycle_count: 0,
        });
        let parser_runtime = parsers
            .iter()
            .map(|parser| {
                (
                    parser.slug().to_string(),
                    ParserRuntimeStats::new(parser.slug().to_string()),
                )
            })
            .collect();

        let pipeline_cache = PipelineCache::new();
        let default_scanner_config = ScannerConfig::default();
        let parser_execution_bulkhead = Arc::new(ParserExecutionBulkhead::from_runtime_defaults(
            runtime_profile,
            parsers.len(),
        ));

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
            history,
            freebet_lifecycle_store: None,
            circuit_breakers: Arc::new(Mutex::new(circuit_breakers)),
            event_bus,
            runtime_profile,
            feature_flags,
            scan_interval_secs,
            request_timeout_secs,
            per_bookmaker_timeout_secs: Arc::new(per_bookmaker_timeout_secs),
            running: Arc::new(Mutex::new(false)),
            state_tx,
            state_rx,
            parser_runtime: Arc::new(RwLock::new(parser_runtime)),
            parser_execution_bulkhead,
            parser_result_caps: default_scanner_config.parser_result_caps(runtime_profile),
            pipeline_cache,
            surebets_cache: Arc::new(parking_lot::RwLock::new(
                std::collections::HashMap::with_capacity(10000),
            )),
            value_bets_cache: Arc::new(parking_lot::RwLock::new(Vec::with_capacity(1000))),
            odds_errors_cache: Arc::new(parking_lot::RwLock::new(Vec::with_capacity(1000))),
        }
    }

    pub fn with_freebet_lifecycle_store(mut self, store: Arc<FreebetLifecycleStore>) -> Self {
        self.freebet_lifecycle_store = Some(store);
        self
    }

    pub fn with_parser_execution_config(mut self, scanner_config: &ScannerConfig) -> Self {
        self.parser_execution_bulkhead = Arc::new(ParserExecutionBulkhead::from_config(
            self.runtime_profile,
            scanner_config,
        ));
        self.parser_result_caps = scanner_config.parser_result_caps(self.runtime_profile);
        self
    }

    pub fn parser_execution_parallelism(&self) -> usize {
        self.parser_execution_bulkhead.max_parallelism()
    }

    pub fn parser_execution_strict_mode(&self) -> bool {
        self.parser_execution_bulkhead.strict_mode()
    }

    /// Получить последние вилки из кэша
    pub fn get_surebets(&self, limit: usize) -> Vec<shared::Surebet> {
        let cache = self.surebets_cache.read();
        let mut surebets: Vec<_> = cache.values().cloned().collect();
        surebets.sort_by(|a, b| {
            b.profit_percent
                .partial_cmp(&a.profit_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        surebets.into_iter().take(limit).collect()
    }

    /// Получить value bets
    pub fn get_value_bets(&self, limit: usize) -> Vec<shared::ValueBet> {
        let cache = self.value_bets_cache.read();
        cache.iter().take(limit).cloned().collect()
    }

    pub fn get_odds_errors(&self, limit: usize) -> Vec<OddsError> {
        let cache = self.odds_errors_cache.read();
        cache.iter().take(limit).cloned().collect()
    }

    /// Генерация ключа дедупликации
    fn surebet_dedup_key(surebet: &shared::Surebet) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            surebet.home_team,
            surebet.away_team,
            surebet
                .legs
                .first()
                .map(|l| l.market.clone())
                .unwrap_or_default(),
            surebet
                .legs
                .first()
                .map(|l| l.line.map(|x| format!("{:.1}", x)).unwrap_or_default())
                .unwrap_or_default(),
            surebet.is_live
        )
    }

    /// Загружаем синхронизированные данные для заблокированных БК
    fn synced_data_search_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Ok(current_dir) = std::env::current_dir() {
            roots.push(current_dir);
        }

        roots.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(".."),
        );

        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                roots.push(exe_dir.to_path_buf());
            }
        }

        roots.sort();
        roots.dedup();
        roots
    }

    fn resolve_synced_data_path(filename: &str) -> Option<PathBuf> {
        Self::synced_data_search_roots()
            .into_iter()
            .map(|root| root.join(filename))
            .find(|path| path.is_file())
    }

    fn load_synced_bk_data() -> (Vec<Event>, Vec<Odd>) {
        use std::fs;

        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        let bks = ["winline", "zenit", "betcity", "baltbet"];

        for bk in bks {
            let file_name = format!("{}_events_synced.json", bk);
            if let Some(file_path) = Self::resolve_synced_data_path(&file_name) {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(events_arr) = data.get("events").and_then(|v| v.as_array()) {
                            let now = chrono::Utc::now();
                            for (i, item) in events_arr.iter().enumerate() {
                                let home =
                                    item.get("home_team").and_then(|v| v.as_str()).unwrap_or("");
                                let away =
                                    item.get("away_team").and_then(|v| v.as_str()).unwrap_or("");
                                let league =
                                    item.get("league").and_then(|v| v.as_str()).unwrap_or("");
                                let is_live = item
                                    .get("is_live")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);

                                if home.is_empty() || away.is_empty() {
                                    continue;
                                }

                                let event_id = format!("{}-synced-{}", bk, i);
                                let sport = if league.contains("NBA") || league.contains("KHL") {
                                    Sport::Basketball
                                } else {
                                    Sport::Football
                                };

                                all_events.push(Event {
                                    id: event_id.clone(),
                                    sport,
                                    league: league.to_string(),
                                    home_team: home.to_string(),
                                    away_team: away.to_string(),
                                    start_time: None,
                                    is_live,
                                    bookmaker_slug: bk.to_string(),
                                    raw_url: None,
                                    extra: HashMap::new(),
                                });

                                // 1X2 odds
                                if let Some(odds_1x2) =
                                    item.get("odds_1x2").and_then(|v| v.as_array())
                                {
                                    if odds_1x2.len() >= 3 {
                                        if let (Some(o1), Some(o_x), Some(o2)) = (
                                            odds_1x2[0].as_f64(),
                                            odds_1x2[1].as_f64(),
                                            odds_1x2[2].as_f64(),
                                        ) {
                                            all_odds.push(Odd {
                                                id: format!("{}-1", event_id),
                                                event_id: event_id.clone(),
                                                bookmaker_slug: bk.to_string(),
                                                market: "1X2".into(),
                                                selection: "1".into(),
                                                odds: o1,
                                                odds_type: OddsType::Home,
                                                line: None,
                                                timestamp: now,
                                            });
                                            all_odds.push(Odd {
                                                id: format!("{}-X", event_id),
                                                event_id: event_id.clone(),
                                                bookmaker_slug: bk.to_string(),
                                                market: "1X2".into(),
                                                selection: "X".into(),
                                                odds: o_x,
                                                odds_type: OddsType::Draw,
                                                line: None,
                                                timestamp: now,
                                            });
                                            all_odds.push(Odd {
                                                id: format!("{}-2", event_id),
                                                event_id: event_id.clone(),
                                                bookmaker_slug: bk.to_string(),
                                                market: "1X2".into(),
                                                selection: "2".into(),
                                                odds: o2,
                                                odds_type: OddsType::Away,
                                                line: None,
                                                timestamp: now,
                                            });
                                        }
                                    }
                                }

                                // Total odds
                                if let Some(odds_over) =
                                    item.get("odds_total_over").and_then(|v| v.as_array())
                                {
                                    if let Some(o) = odds_over.get(0).and_then(|v| v.as_f64()) {
                                        all_odds.push(Odd {
                                            id: format!("{}-total-over", event_id),
                                            event_id: event_id.clone(),
                                            bookmaker_slug: bk.to_string(),
                                            market: "Total".into(),
                                            selection: "Over".into(),
                                            odds: o,
                                            odds_type: OddsType::Over,
                                            line: Some(2.5),
                                            timestamp: now,
                                        });
                                    }
                                }

                                if let Some(odds_under) =
                                    item.get("odds_total_under").and_then(|v| v.as_array())
                                {
                                    if let Some(o) = odds_under.get(0).and_then(|v| v.as_f64()) {
                                        all_odds.push(Odd {
                                            id: format!("{}-total-under", event_id),
                                            event_id: event_id.clone(),
                                            bookmaker_slug: bk.to_string(),
                                            market: "Total".into(),
                                            selection: "Under".into(),
                                            odds: o,
                                            odds_type: OddsType::Under,
                                            line: Some(2.5),
                                            timestamp: now,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        (all_events, all_odds)
    }

    fn should_use_offline_synced_events_fallback(&self) -> bool {
        self.feature_flags.offline_synced_events_fallback_enabled()
    }

    fn offline_synced_events_fallback_data(&self) -> (Vec<Event>, Vec<Odd>) {
        if self.should_use_offline_synced_events_fallback() {
            Self::load_synced_bk_data()
        } else {
            (Vec::new(), Vec::new())
        }
    }

    fn parser_timeout_secs(&self, slug: &str) -> u64 {
        self.per_bookmaker_timeout_secs
            .get(slug)
            .copied()
            .unwrap_or_else(|| default_parser_timeout_secs(slug, self.request_timeout_secs))
    }

    fn cycle_timeout_secs(&self) -> u64 {
        let parser_timeout = self
            .parsers
            .iter()
            .map(|parser| self.parser_timeout_secs(parser.slug()))
            .max()
            .unwrap_or(self.request_timeout_secs);

        parser_timeout.saturating_add(30).max(90)
    }

    pub async fn run_cycle(&self) -> ScannerMetrics {
        self.run_cycle_inner().await
    }

    pub async fn run_cycle_inner(&self) -> ScannerMetrics {
        debug!("Cycle starting...");
        let cycle_start = Instant::now();

        // ВСЕГДА фетчим данные — pipeline cache отключён
        info!("🔄 Fetching parsers...");
        let results = Self::fetch_parsers_parallel(
            &self.parsers,
            &self.circuit_breakers,
            self.runtime_profile,
            self.request_timeout_secs,
            self.per_bookmaker_timeout_secs.clone(),
            self.parser_runtime.clone(),
            self.parser_execution_bulkhead.clone(),
            self.parser_result_caps,
        )
        .await;
        let mut fetched_odds: Vec<Odd> = results.iter().flat_map(|e| e.odds.clone()).collect();
        let mut fetched_events: Vec<Event> =
            results.iter().flat_map(|e| e.events.clone()).collect();

        let synced = self.offline_synced_events_fallback_data();
        let synced_events_len = synced.0.len();
        let _synced_odds_len = synced.1.len();
        fetched_events.extend(synced.0);
        fetched_odds.extend(synced.1);

        if self.should_use_offline_synced_events_fallback() {
            info!(
                profile = ?self.runtime_profile,
                synced_events = synced_events_len,
                "Offline synced-events fallback is enabled"
            );
        } else {
            info!(
                profile = ?self.runtime_profile,
                "Offline synced-events fallback is disabled"
            );
        }

        info!(
            "📊 Fetched: {} events, {} odds from {} parsers (+ {} synced)",
            fetched_events.len(),
            fetched_odds.len(),
            results.len(),
            synced_events_len
        );
        for result in &results {
            info!(
                "  - {} events, {} odds",
                result.events.len(),
                result.odds.len()
            );
        }

        self.process_events(fetched_events, fetched_odds, cycle_start)
            .await
    }

    /// Быстрая обработка событий из кэша — КЛЮЧЕВОЕ ИЗМЕНЕНИЕ
    /// Группируем события по матчу (home+away+sport), потом ищем вилки МЕЖДУ БК
    async fn process_events(
        &self,
        events: Vec<Event>,
        all_odds: Vec<Odd>,
        cycle_start: Instant,
    ) -> ScannerMetrics {
        // ДЕДУПЛИКАЦИЯ + БАЛАНСИРОВКА по БК
        // Проблема: .take(5000) берёт все events от одной БК
        // Решение: равномерно распределяем по всем БК
        const MAX_EVENTS: usize = 5000;
        const MAX_ODDS: usize = 100_000;

        let mut events_by_bk: HashMap<String, Vec<Event>> = HashMap::new();
        for event in &events {
            events_by_bk
                .entry(event.bookmaker_slug.clone())
                .or_default()
                .push(event.clone());
        }

        let max_per_bk = MAX_EVENTS / events_by_bk.len().max(1);
        let mut calc_events = Vec::with_capacity(MAX_EVENTS);
        for (_, bk_events) in &events_by_bk {
            let take = bk_events.len().min(max_per_bk);
            calc_events.extend(bk_events.iter().take(take).cloned());
        }

        // Аналогично для odds
        let mut odds_by_bk: HashMap<String, Vec<Odd>> = HashMap::new();
        for odd in &all_odds {
            odds_by_bk
                .entry(odd.bookmaker_slug.clone())
                .or_default()
                .push(odd.clone());
        }

        let max_odds_per_bk = MAX_ODDS / odds_by_bk.len().max(1);
        let mut calc_odds = Vec::with_capacity(MAX_ODDS);
        for (_, bk_odds) in &odds_by_bk {
            let take = bk_odds.len().min(max_odds_per_bk);
            calc_odds.extend(bk_odds.iter().take(take).cloned());
        }

        info!(
            "📦 Balanced: {} events from {} BKs, {} odds from {} BKs",
            calc_events.len(),
            events_by_bk.len(),
            calc_odds.len(),
            odds_by_bk.len()
        );

        let normalized: Vec<Event> = calc_events
            .iter()
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
        // КЛЮЧЕВОЕ: odds должны быть только от событий в calc_events!
        let mut event_by_id: HashMap<String, &Event> = HashMap::with_capacity(calc_events.len());
        for event in &calc_events {
            event_by_id.insert(event.id.clone(), event);
        }

        // Фильтруем odds — только от событий в calc_events
        let calc_odds: Vec<Odd> = calc_odds
            .into_iter()
            .filter(|o| event_by_id.contains_key(&o.event_id))
            .collect();

        self.generosity_index.update(&normalized, &calc_odds);

        info!(
            "🧹 Odds filter: {} → {} odds (only from calc_events)",
            {
                // count would be before filter - we can't know, but log result
                calc_odds.len()
            },
            calc_odds.len()
        );

        let mut odds_by_match: HashMap<String, Vec<&Odd>> = HashMap::new();

        for odd in &calc_odds {
            if let Some(event) = event_by_id.get(&odd.event_id) {
                let fp = Self::event_fingerprint(event);
                odds_by_match.entry(fp).or_default().push(odd);
            }
        }

        // Статистика
        let matches_with_multi_bk = odds_by_match
            .iter()
            .filter(|(_, odds)| {
                let bks: std::collections::HashSet<&str> =
                    odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
                bks.len() >= 2
            })
            .count();

        info!(
            "📊 {} events → {} matches ({} with 2+ BK), {} total odds",
            calc_events.len(),
            matches.len(),
            matches_with_multi_bk,
            calc_odds.len()
        );

        // Show first 5 matches with multi-BK odds
        let mut shown = 0;
        for (fp, _match_events) in &matches {
            if shown >= 5 {
                break;
            }
            if let Some(odds) = odds_by_match.get(fp) {
                let bks: std::collections::HashSet<&str> =
                    odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
                if bks.len() >= 2 {
                    // Show market distribution
                    let markets: std::collections::HashMap<String, usize> = {
                        let mut m = std::collections::HashMap::new();
                        for o in odds {
                            *m.entry(o.market.clone()).or_insert(0) += 1;
                        }
                        m
                    };
                    let top_markets: Vec<_> = markets
                        .iter()
                        .map(|(k, v)| format!("{}:{}", k, v))
                        .take(5)
                        .collect();

                    info!(
                        "🎯 Match #{} '{}' — {} odds from {} BKs {:?} | Markets: {:?}",
                        shown + 1,
                        fp,
                        odds.len(),
                        bks.len(),
                        bks,
                        top_markets
                    );
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
            if match_odds.len() < 2 {
                continue;
            } // Нужны odds от 2+ БК

            checked_matches += 1;
            if checked_matches <= 3 {
                // Debug first 3 matches
                let bks: std::collections::HashSet<&str> = match_odds
                    .iter()
                    .map(|o| o.bookmaker_slug.as_str())
                    .collect();
                info!(
                    "Match #{} '{}' — {} odds from {} bookmakers: {:?}",
                    checked_matches,
                    fp,
                    match_odds.len(),
                    bks.len(),
                    bks
                );
            }

            // Ищем вилки между разными БК для этого матча
            let raw_surebets = self.find_cross_bk_surebets(match_events, &match_odds);
            let verification_odds: Vec<Odd> = match_odds.iter().map(|odd| (*odd).clone()).collect();

            // Фильтруем дубликаты через bloom filter
            let mut new_surebets = 0;
            let mut verified_surebets = 0;
            for surebet in &raw_surebets {
                if self.calculator.is_seen(surebet) {
                    continue; // Уже видели эту вилку
                }

                self.calculator.mark_seen(surebet);
                new_surebets += 1;

                let verification = self
                    .odds_verifier
                    .verify_surebet(surebet, &verification_odds)
                    .await;
                let is_verified = verification_passes(&verification, self.calculator.min_profit);

                if is_verified {
                    verified_surebets += 1;
                } else {
                    debug!(
                        surebet_id = %surebet.id,
                        profit_before = surebet.profit_percent,
                        profit_after = verification.profit_after,
                        changed_legs = ?verification.changed_legs,
                        "Surebet rejected by verifier"
                    );
                }

                // Создаём верифицированную вилку
                let mut verified_surebet = surebet.clone();
                verified_surebet.verified = is_verified;

                // Сохраняем в кэш для API с дедупликацией
                {
                    let mut cache = self.surebets_cache.write();
                    let dedup_key = Self::surebet_dedup_key(surebet);

                    // Вставляем/обновляем только если profit выше
                    let should_insert = match cache.get(&dedup_key) {
                        Some(existing) => surebet.profit_percent > existing.profit_percent,
                        None => true,
                    };

                    if should_insert {
                        cache.insert(dedup_key, verified_surebet.clone());
                    }
                }

                if let Err(error) = self.history.save(&verified_surebet).await {
                    warn!(
                        error = %error,
                        surebet_id = %verified_surebet.id,
                        "Failed to persist surebet history entry"
                    );
                }

                let payload = serde_json::to_value(&verified_surebet).unwrap_or_default();
                let _ = self.event_bus.publish(BusEvent::SurebetFound {
                    surebet_id: verified_surebet.id.to_string(),
                    payload,
                    timestamp: Utc::now(),
                });
            }

            if new_surebets > 0 {
                info!("🎯 Found {} NEW surebets for match '{}' ({} verified, {} total, {} duplicates)",
                      new_surebets, fp, verified_surebets, raw_surebets.len(), raw_surebets.len() - new_surebets);
            }
            total_surebets += new_surebets;
        }

        // Fallback: старый метод для сравнения
        let legacy_surebets = self.calculator.find_surebets(&normalized, &calc_odds);
        total_surebets += legacy_surebets.len();

        // Value bets detection
        let value_bets = self.value_detector.detect_values(&normalized, &calc_odds);
        {
            let mut cache = self.value_bets_cache.write();
            *cache = value_bets.into_iter().take(1000).collect();
        }

        let odds_errors = self
            .odds_error_detector
            .detect_event_aware_errors(&normalized, &calc_odds);
        {
            let mut cache = self.odds_errors_cache.write();
            *cache = odds_errors.into_iter().take(1000).collect();
        }

        // Freebet Hunter — поиск возможностей для отыгрыша фрибетов
        self.freebet_hunter
            .update_cache(normalized.clone(), calc_odds.clone());
        let freebets = self
            .freebet_hunter
            .find_opportunities(&normalized, &calc_odds);
        if !freebets.is_empty() {
            info!(count = freebets.len(), "Freebet opportunities found");
            for fb in &freebets {
                info!(
                    bookmaker = fb.bookmaker,
                    profit = fb.guaranteed_profit,
                    roi = fb.roi,
                    "Freebet: {} → {} vs {}, back:{}, lay:{}",
                    fb.bookmaker,
                    fb.event.home_team,
                    fb.event.away_team,
                    fb.back_odds,
                    fb.lay_odds
                );
            }
        }

        if let Some(store) = &self.freebet_lifecycle_store {
            let lifecycle_states = collect_freebet_lifecycle(
                freebets.clone(),
                self.bonus_hunter.as_ref(),
                self.bankroll_manager.as_ref(),
            );
            if let Err(error) =
                persist_freebet_lifecycle_states(store.as_ref(), &lifecycle_states).await
            {
                warn!(error = %error, "Failed to persist freebet lifecycle snapshot");
            }
        }

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

        info!(
            cycle_ms = cycle_time,
            events = normalized.len(),
            odds = calc_odds.len(),
            matches = matches.len(),
            surebets = total_surebets,
            "Cycle complete"
        );

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
        let live_state = if norm_event.is_live {
            "live"
        } else {
            "prematch"
        };

        // Sort to ensure consistent ordering regardless of home/away order
        let (first, second) = if home < away {
            (home, away)
        } else {
            (away, home)
        };
        // Включаем live/prematch статус, чтобы не склеивать разные состояния матча
        format!(
            "{:?}|{}|{}|{}|{}",
            event.sport, live_state, league, first, second
        )
    }

    /// Робастная нормализация названия команды
    fn normalize_team_name(name: &str) -> String {
        let mut s = name
            .to_lowercase()
            // Remove common prefixes
            .replace("фк ", "")
            .replace("ск ", "")
            .replace("пк ", "")
            .replace("фк", "")
            .replace("ск", "")
            .replace("пк", "")
            .replace("хк ", "")
            .replace("хк", "")
            .replace("фс ", "")
            .replace("фс", "")
            .replace("гк ", "")
            .replace("гк", "")
            .replace("фк ", "")
            .replace("фк", "")
            .replace("футбольный клуб ", "")
            // Remove common suffixes
            .replace(" москва", "")
            .replace(" спб", "")
            .replace(" спб.", "")
            .replace(" санкт-петербург", "")
            .replace(" с.-петербург", "")
            .replace(" петербург", "")
            .replace(" питер", "")
            .replace(" нижний новгород", "")
            .replace(" новгород", "")
            // Remove punctuation and extra chars
            .replace("(", "")
            .replace(")", "")
            .replace("-", " ")
            .replace(".", "")
            .replace(",", "")
            .replace("'", "")
            .replace("\"", "")
            .replace("_", " ")
            .trim()
            .to_string();

        // Collapse multiple spaces
        while s.contains("  ") {
            s = s.replace("  ", " ");
        }
        s.trim().to_string()
    }

    /// Ищем вилки МЕЖДУ разными БК для одного матча
    #[inline]
    fn find_cross_bk_surebets(&self, events: &[&Event], odds: &[&Odd]) -> Vec<shared::Surebet> {
        use shared::Surebet;
        use shared::SurebetLeg;
        use uuid::Uuid;

        let mut surebets = Vec::new();

        if odds.is_empty() {
            return surebets;
        }

        // Считаем уникальные БК
        let unique_bks: std::collections::HashSet<&str> =
            odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
        if unique_bks.len() < 2 {
            return surebets;
        }

        // Группируем odds по market+line
        let mut by_market: HashMap<String, Vec<&&Odd>> = HashMap::new();
        for odd in odds {
            let line_key = odd.line.map(|l| format!("{:.1}", l)).unwrap_or_default();
            let key = format!("{}|{}", odd.market.to_lowercase(), line_key);
            by_market.entry(key).or_default().push(odd);
        }

        // Для каждого рынка ищем вилки между БК
        for (market, market_odds) in &by_market {
            let lower = market.to_lowercase();

            if lower.starts_with("1x2") || lower.starts_with("исход") || lower.starts_with("match")
            {
                // 3-way: ищем 1, X, 2 от разных БК
                let ones: Vec<&&Odd> = market_odds
                    .iter()
                    .filter(|o| o.selection == "1" || o.selection.to_lowercase() == "п1")
                    .cloned()
                    .collect();
                let xs: Vec<&&Odd> = market_odds
                    .iter()
                    .filter(|o| o.selection == "X" || o.selection.to_lowercase() == "х")
                    .cloned()
                    .collect();
                let twos: Vec<&&Odd> = market_odds
                    .iter()
                    .filter(|o| o.selection == "2" || o.selection.to_lowercase() == "п2")
                    .cloned()
                    .collect();

                if ones.is_empty() || xs.is_empty() || twos.is_empty() {
                    continue;
                }

                for &o1 in &ones {
                    for &ox in &xs {
                        for &o2 in &twos {
                            // КРИТИЧНО: все 3 исхода должны быть от РАЗНЫХ БК
                            // Иначе это не арбитраж, а маржа одной БК
                            let bk1 = o1.bookmaker_slug.as_str();
                            let bkx = ox.bookmaker_slug.as_str();
                            let bk2 = o2.bookmaker_slug.as_str();

                            if bk1 == bkx || bk1 == bk2 || bkx == bk2 {
                                continue; // Минимум 2 одинаковые БК — не арбитраж
                            }

                            if let Some(profit) =
                                shared::odds::calculate_surebet_profit(&[o1.odds, ox.odds, o2.odds])
                            {
                                if profit < self.calculator.min_profit {
                                    continue;
                                }

                                let stakes = shared::odds::calculate_stakes(
                                    &[o1.odds, ox.odds, o2.odds],
                                    1000.0,
                                );
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
                }
            } else if lower.starts_with("total") || lower.starts_with("тотал") {
                // 2-way: Over/Under с той же линией от разных БК
                let overs: Vec<&&Odd> = market_odds
                    .iter()
                    .filter(|o| {
                        o.selection.to_lowercase().contains("over")
                            || o.selection.to_lowercase().contains("больше")
                            || o.selection.to_lowercase() == "тб"
                    })
                    .cloned()
                    .collect();
                let unders: Vec<&&Odd> = market_odds
                    .iter()
                    .filter(|o| {
                        o.selection.to_lowercase().contains("under")
                            || o.selection.to_lowercase().contains("меньше")
                            || o.selection.to_lowercase() == "тм"
                    })
                    .cloned()
                    .collect();

                for &o_over in &overs {
                    for &o_under in &unders {
                        if o_over.bookmaker_slug == o_under.bookmaker_slug {
                            continue;
                        }

                        // Проверяем что линии совпадают
                        if let (Some(l1), Some(l2)) = (o_over.line, o_under.line) {
                            if (l1 - l2).abs() > 0.1 {
                                continue;
                            }
                        }

                        if let Some(profit) =
                            shared::odds::calculate_surebet_profit(&[o_over.odds, o_under.odds])
                        {
                            if profit < self.calculator.min_profit {
                                continue;
                            } // Используем конфиг

                            let stakes = shared::odds::calculate_stakes(
                                &[o_over.odds, o_under.odds],
                                1000.0,
                            );
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
            }
        }

        surebets
    }

    /// Параллельный fetch всех парсеров — вынесен в отдель метод
    async fn fetch_parsers_parallel(
        parsers: &[Arc<dyn BookmakerParser + Send + Sync>],
        breakers: &Arc<Mutex<HashMap<String, CircuitBreaker>>>,
        runtime_profile: RuntimeProfile,
        request_timeout_secs: u64,
        per_bookmaker_timeout_secs: Arc<HashMap<String, u64>>,
        parser_runtime: Arc<RwLock<HashMap<String, ParserRuntimeStats>>>,
        parser_execution_bulkhead: Arc<ParserExecutionBulkhead>,
        parser_result_caps: ParserResultCaps,
    ) -> Vec<ParserResult> {
        use futures::future::join_all;

        let mut futures = Vec::new();

        for parser in parsers.iter() {
            let slug = parser.slug().to_string();
            let parser = parser.clone();
            let breakers = breakers.clone();
            let per_bookmaker_timeout_secs = per_bookmaker_timeout_secs.clone();
            let parser_runtime = parser_runtime.clone();
            let parser_execution_bulkhead = parser_execution_bulkhead.clone();

            let skip = {
                let brk = breakers.lock().unwrap();
                brk.get(&slug)
                    .map(|cb| !cb.allow_request())
                    .unwrap_or(false)
            };

            if skip {
                continue;
            }

            let fut = async move {
                let _permit = parser_execution_bulkhead.acquire().await;
                let timeout_secs = per_bookmaker_timeout_secs
                    .get(&slug)
                    .copied()
                    .unwrap_or_else(|| default_parser_timeout_secs(&slug, request_timeout_secs));
                let started = std::time::Instant::now();

                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    parser.fetch_all(),
                )
                .await;

                match result {
                    Ok(Ok(r)) => {
                        let previous_events = parser_runtime
                            .read()
                            .get(&slug)
                            .map(ParserRuntimeStats::events_parsed)
                            .filter(|events| *events > 0);
                        let validated_result = validate_parser_result(
                            r,
                            runtime_profile,
                            parser_result_caps,
                            previous_events,
                        );
                        let validation = &validated_result.validation;

                        if validation.accepts_result() {
                            if let Some(breaker) = breakers.lock().unwrap().get(&slug).cloned() {
                                breaker.record_success();
                            }
                            if let Some(runtime) = parser_runtime.write().get_mut(&slug) {
                                runtime.record_success(
                                    validated_result.result.timestamp,
                                    validated_result.result.fetch_time_ms as f64,
                                    validated_result.result.events.len() as u64,
                                    validated_result.result.odds.len() as u64,
                                    validation.status.clone(),
                                    validation.summary.clone(),
                                    validation.diagnostics.clone(),
                                );
                            }
                            if !matches!(validation.status, shared::ParserResultStatus::Healthy) {
                                warn!(
                                    parser = %slug,
                                    status = ?validation.status,
                                    message = validation.summary.as_deref().unwrap_or("post-fetch validation flagged result"),
                                    "Parser result degraded after fetch"
                                );
                            }
                            Some(validated_result.result)
                        } else {
                            if let Some(breaker) = breakers.lock().unwrap().get(&slug).cloned() {
                                breaker.record_failure();
                            }
                            let error = validation
                                .summary
                                .clone()
                                .unwrap_or_else(|| "post-fetch validation failed".into());
                            if let Some(runtime) = parser_runtime.write().get_mut(&slug) {
                                runtime.record_rejected_result(
                                    validated_result.result.timestamp,
                                    error.clone(),
                                    validated_result.result.fetch_time_ms as f64,
                                    validation.diagnostics.clone(),
                                );
                            }
                            warn!(parser = %slug, error = %error, "Parser result rejected after fetch");
                            None
                        }
                    }
                    Ok(Err(e)) => {
                        if let Some(breaker) = breakers.lock().unwrap().get(&slug).cloned() {
                            breaker.record_failure();
                        }
                        let error = e.to_string();
                        if let Some(runtime) = parser_runtime.write().get_mut(&slug) {
                            runtime.record_failure(
                                Utc::now(),
                                error.clone(),
                                started.elapsed().as_millis() as f64,
                            );
                        }
                        warn!(parser = %slug, error = %e, "Parser error");
                        None
                    }
                    Err(_) => {
                        if let Some(breaker) = breakers.lock().unwrap().get(&slug).cloned() {
                            breaker.record_failure();
                        }
                        if let Some(runtime) = parser_runtime.write().get_mut(&slug) {
                            runtime.record_failure(
                                Utc::now(),
                                format!("timeout after {timeout_secs}s"),
                                started.elapsed().as_millis() as f64,
                            );
                        }
                        warn!(parser = %slug, timeout_secs, "Parser timeout");
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
        info!(
            parser_parallelism = self.parser_execution_parallelism(),
            parser_bulkhead_strict_mode = self.parser_execution_strict_mode(),
            "Scanner starting..."
        );
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
            let cycle_timeout_secs = self.cycle_timeout_secs();
            let cycle_result = tokio::time::timeout(
                std::time::Duration::from_secs(cycle_timeout_secs),
                self.run_cycle_inner(),
            )
            .await;

            let metrics = match cycle_result {
                Ok(m) => m,
                Err(_) => {
                    warn!(timeout_secs = cycle_timeout_secs, "Scan cycle timed out");
                    let metrics: ScannerMetrics = ScannerMetrics {
                        cycle_time_ms: cycle_timeout_secs * 1000,
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

            let _ = self
                .event_bus
                .publish(shared::BusEvent::SystemAlert {
                    level: "info".into(),
                    message: format!(
                        "Cycle: {}ms, {} events, {} opportunities",
                        metrics.cycle_time_ms, metrics.events_parsed, metrics.surebets_found
                    ),
                    timestamp: Utc::now(),
                })
                .await;

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

    pub fn get_best_bonuses(&self, limit: usize) -> Vec<BonusInfo> {
        self.bonus_hunter.get_best_bonuses(limit)
    }

    pub fn parser_runtime_snapshots(&self) -> Vec<ParserRuntimeSnapshot> {
        let runtime = self.parser_runtime.read();
        let breakers = self.circuit_breakers.lock().unwrap();
        let mut snapshots = runtime
            .values()
            .map(|entry| {
                let circuit_state = breakers
                    .get(entry.bookmaker())
                    .map(CircuitBreaker::state)
                    .unwrap_or(CircuitState::Closed);
                entry.snapshot(map_circuit_state(circuit_state))
            })
            .collect::<Vec<_>>();
        snapshots.sort_by(|left, right| left.bookmaker.cmp(&right.bookmaker));
        snapshots
    }
}

fn verification_passes(
    verification: &engine::verifier::VerificationResult,
    min_profit: f64,
) -> bool {
    verification.verified
        && verification
            .profit_after
            .map(|profit| profit >= min_profit)
            .unwrap_or(false)
}

fn default_parser_timeout_secs(slug: &str, request_timeout_secs: u64) -> u64 {
    match slug {
        "winline" | "melbet" | "zenit" | "betcity" | "baltbet" | "betboom" | "ligastavok" => {
            request_timeout_secs.max(60)
        }
        _ => request_timeout_secs.max(30),
    }
}

#[cfg(test)]
mod tests {
    use super::{default_parser_timeout_secs, verification_passes, GhostScanner};
    use async_trait::async_trait;
    use auto_betting::engine::AutoBetEngine;
    use bankroll_manager::manager::BankrollManager;
    use bonus_hunter::hunter::BonusHunter;
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
    use engine::verifier::VerificationResult;
    use express_forks::ExpressForkScanner;
    use persistence::history::SurebetHistory;
    use shared::config::{FeatureFlag, FeatureFlags, RuntimeProfile, ScannerConfig};
    use shared::odds::OddsType;
    use shared::{
        AutoBetConfig, BankrollConfig, BonusConfig, EventBus, Odd, ParserResultStatus,
        RuntimeCircuitState, Sport,
    };
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[derive(Debug)]
    struct MockParser {
        slug: &'static str,
        response: Result<(Vec<shared::Event>, Vec<Odd>), &'static str>,
        delay_ms: u64,
    }

    #[async_trait]
    impl parsers::base::BookmakerParser for MockParser {
        fn name(&self) -> &str {
            self.slug
        }

        fn slug(&self) -> &str {
            self.slug
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn fetch_events(
            &self,
        ) -> Result<Vec<shared::Event>, Box<dyn std::error::Error + Send + Sync>> {
            self.fetch_all().await.map(|result| result.events)
        }

        async fn fetch_odds(
            &self,
            _event_id: &str,
        ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
            self.fetch_all().await.map(|result| result.odds)
        }

        async fn fetch_all(
            &self,
        ) -> Result<parsers::base::ParserResult, Box<dyn std::error::Error + Send + Sync>> {
            if self.delay_ms > 0 {
                sleep(Duration::from_millis(self.delay_ms)).await;
            }
            match &self.response {
                Ok((events, odds)) => Ok(parsers::base::ParserResult::new(
                    self.slug,
                    events.clone(),
                    odds.clone(),
                    25,
                )),
                Err(error) => Err((*error).into()),
            }
        }

        fn base_url(&self) -> &str {
            "https://example.invalid"
        }

        fn user_agent(&self) -> &str {
            "mock"
        }
    }

    fn make_test_event(id: &str, bookmaker: &str) -> shared::Event {
        shared::Event {
            id: id.into(),
            sport: Sport::Football,
            league: "Premier League".into(),
            home_team: "Arsenal".into(),
            away_team: "Chelsea".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: bookmaker.into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

    fn make_test_odd(event_id: &str, bookmaker: &str, selection: &str, odds: f64) -> Odd {
        Odd {
            id: format!("{event_id}-{bookmaker}-{selection}"),
            event_id: event_id.into(),
            bookmaker_slug: bookmaker.into(),
            market: "1X2".into(),
            selection: selection.into(),
            odds,
            odds_type: match selection {
                "1" => OddsType::Home,
                "X" => OddsType::Draw,
                _ => OddsType::Away,
            },
            line: None,
            timestamp: Utc::now(),
        }
    }

    async fn build_test_scanner() -> GhostScanner {
        build_test_scanner_with_parsers(Vec::new()).await
    }

    async fn build_test_scanner_with_parsers(
        parsers: Vec<Arc<dyn parsers::base::BookmakerParser + Send + Sync>>,
    ) -> GhostScanner {
        build_test_scanner_with_runtime(parsers, RuntimeProfile::Dev, FeatureFlag::Enabled).await
    }

    async fn build_test_scanner_with_runtime(
        parsers: Vec<Arc<dyn parsers::base::BookmakerParser + Send + Sync>>,
        runtime_profile: RuntimeProfile,
        offline_synced_events_fallback: FeatureFlag,
    ) -> GhostScanner {
        GhostScanner::new(
            parsers,
            Arc::new(SurebetCalculator::new(0.1, 25.0, 1000.0, 1024, 0.01)),
            Arc::new(Normalizer::new()),
            Arc::new(EventPool::new(10_000, 0.01, 10_000)),
            Arc::new(FreebetHunter::new(vec![1000.0], 1.0, 60)),
            Arc::new(GenerosityIndexCalc::new()),
            Arc::new(MirrorDetector::new(0.05)),
            Arc::new(MomentumScanner::new(0.1, 1000.0)),
            Arc::new(OddsErrorDetector::new(25.0, 3)),
            Arc::new(ValueDetector::new(1.0)),
            Arc::new(engine::verifier::OddsVerifier::new(1, 1, 60)),
            Arc::new(CorridorScanner::new(0.5)),
            Arc::new(ExpressForkScanner::new(3, 0.1, 1000.0)),
            Arc::new(BankrollManager::new(BankrollConfig::default())),
            Arc::new(BonusHunter::new(BonusConfig::default())),
            Arc::new(AutoBetEngine::new(AutoBetConfig::default())),
            Arc::new(SurebetHistory::new("memory").await.expect("history init")),
            Arc::new(EventBus::new()),
            runtime_profile,
            FeatureFlags {
                offline_synced_events_fallback,
            },
            30,
            30,
            HashMap::new(),
        )
        .with_parser_execution_config(&ScannerConfig {
            parallel_parsers: 4,
            production_parallel_parsers: Some(2),
            ..ScannerConfig::default()
        })
    }

    #[test]
    fn synced_data_search_roots_include_repo_root() {
        let expected_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");

        assert!(GhostScanner::synced_data_search_roots()
            .iter()
            .any(|root| root == &expected_root));
    }

    #[test]
    fn resolves_synced_data_file_from_repo_root() {
        let path = GhostScanner::resolve_synced_data_path("betcity_events_synced.json")
            .expect("synced data file should resolve from repository root");

        assert!(path.ends_with(Path::new("betcity_events_synced.json")));
        assert!(path.is_file());
    }

    #[tokio::test]
    async fn dev_profile_keeps_synced_fallback_enabled() {
        let scanner = build_test_scanner().await;

        assert_eq!(scanner.runtime_profile, RuntimeProfile::Dev);
        assert!(scanner.should_use_offline_synced_events_fallback());

        let (events, odds) = scanner.offline_synced_events_fallback_data();
        assert!(!events.is_empty());
        assert!(!odds.is_empty());
    }

    #[tokio::test]
    async fn production_profile_can_disable_synced_fallback() {
        let scanner = build_test_scanner_with_runtime(
            Vec::new(),
            RuntimeProfile::Production,
            FeatureFlag::Disabled,
        )
        .await;

        assert_eq!(scanner.runtime_profile, RuntimeProfile::Production);
        assert!(!scanner.should_use_offline_synced_events_fallback());

        let (events, odds) = scanner.offline_synced_events_fallback_data();
        assert!(events.is_empty());
        assert!(odds.is_empty());
    }

    #[test]
    fn default_timeout_extends_runtime_and_rollout_parsers() {
        assert_eq!(default_parser_timeout_secs("betboom", 30), 60);
        assert_eq!(default_parser_timeout_secs("ligastavok", 45), 60);
        assert_eq!(default_parser_timeout_secs("pari", 20), 30);
    }

    #[test]
    fn event_fingerprint_separates_live_from_prematch() {
        let base_event = shared::Event {
            id: "evt1".into(),
            sport: shared::Sport::Football,
            league: "Premier League".into(),
            home_team: "FC Barcelona".into(),
            away_team: "Real Madrid".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "bk1".into(),
            raw_url: None,
            extra: std::collections::HashMap::new(),
        };
        let mut live_event = base_event.clone();
        live_event.is_live = true;

        assert_ne!(
            GhostScanner::event_fingerprint(&base_event),
            GhostScanner::event_fingerprint(&live_event)
        );
    }

    #[test]
    fn verification_requires_profit_after_threshold() {
        let verification = VerificationResult {
            surebet_id: "test".into(),
            verified: true,
            profit_before: 1.5,
            profit_after: Some(0.05),
            changed_legs: Vec::new(),
            verified_at: Utc::now(),
        };

        assert!(!verification_passes(&verification, 0.1));
        assert!(verification_passes(&verification, 0.01));
    }

    #[tokio::test]
    async fn process_events_updates_generosity_index_from_scan_data() {
        let scanner = build_test_scanner().await;
        let events = vec![
            make_test_event("evt-bk1", "bk1"),
            make_test_event("evt-bk2", "bk2"),
        ];
        let odds = vec![
            make_test_odd("evt-bk1", "bk1", "1", 2.20),
            make_test_odd("evt-bk1", "bk1", "X", 3.40),
            make_test_odd("evt-bk1", "bk1", "2", 3.60),
            make_test_odd("evt-bk2", "bk2", "1", 2.00),
            make_test_odd("evt-bk2", "bk2", "X", 3.20),
            make_test_odd("evt-bk2", "bk2", "2", 3.30),
        ];

        let _ = scanner
            .process_events(events, odds, std::time::Instant::now())
            .await;

        let indices = scanner
            .generosity_index
            .get_indices_by_sport(Sport::Football);
        assert_eq!(indices.len(), 2);
        assert_eq!(indices[0].bookmaker, "bk1");
        assert_eq!(indices[0].total_events, 1);
        assert!(indices[0].best_odds_count > indices[1].best_odds_count);
    }

    #[tokio::test]
    async fn process_events_populates_odds_errors_cache() {
        let scanner = build_test_scanner().await;
        let events = vec![
            make_test_event("evt-pari", "pari"),
            make_test_event("evt-fonbet", "fonbet"),
            make_test_event("evt-marathon", "marathon"),
        ];
        let odds = vec![
            make_test_odd("evt-pari", "pari", "1", 10.0),
            make_test_odd("evt-fonbet", "fonbet", "1", 2.05),
            make_test_odd("evt-marathon", "marathon", "1", 2.0),
        ];

        let _ = scanner
            .process_events(events, odds, std::time::Instant::now())
            .await;

        let errors = scanner.get_odds_errors(10);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].bookmaker, "pari");
        assert_eq!(errors[0].event.id, "evt-pari");
    }

    #[tokio::test]
    async fn fetch_parsers_parallel_records_runtime_success() {
        let parser = Arc::new(MockParser {
            slug: "mock-success",
            response: Ok((
                vec![make_test_event("evt-success", "mock-success")],
                vec![make_test_odd("evt-success", "mock-success", "1", 2.0)],
            )),
            delay_ms: 0,
        });
        let scanner = build_test_scanner_with_parsers(vec![parser]).await;

        let results = GhostScanner::fetch_parsers_parallel(
            &scanner.parsers,
            &scanner.circuit_breakers,
            scanner.runtime_profile,
            scanner.request_timeout_secs,
            scanner.per_bookmaker_timeout_secs.clone(),
            scanner.parser_runtime.clone(),
            scanner.parser_execution_bulkhead.clone(),
            scanner.parser_result_caps,
        )
        .await;

        assert_eq!(results.len(), 1);

        let snapshots = scanner.parser_runtime_snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].bookmaker, "mock-success");
        assert_eq!(snapshots[0].events_parsed, 1);
        assert_eq!(snapshots[0].successful_runs, 1);
        assert_eq!(snapshots[0].total_runs, 1);
        assert_eq!(snapshots[0].consecutive_failures, 0);
        assert_eq!(snapshots[0].last_result_status, ParserResultStatus::Healthy);
        assert!(snapshots[0].last_success.is_some());
        assert!(matches!(
            snapshots[0].circuit_state,
            RuntimeCircuitState::Closed
        ));
    }

    #[tokio::test]
    async fn fetch_parsers_parallel_opens_circuit_after_repeated_failures() {
        let parser = Arc::new(MockParser {
            slug: "mock-failure",
            response: Err("boom"),
            delay_ms: 0,
        });
        let scanner = build_test_scanner_with_parsers(vec![parser]).await;

        for _ in 0..5 {
            let _ = GhostScanner::fetch_parsers_parallel(
                &scanner.parsers,
                &scanner.circuit_breakers,
                scanner.runtime_profile,
                scanner.request_timeout_secs,
                scanner.per_bookmaker_timeout_secs.clone(),
                scanner.parser_runtime.clone(),
                scanner.parser_execution_bulkhead.clone(),
                scanner.parser_result_caps,
            )
            .await;
        }

        let snapshots = scanner.parser_runtime_snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].bookmaker, "mock-failure");
        assert_eq!(snapshots[0].successful_runs, 0);
        assert_eq!(snapshots[0].total_runs, 5);
        assert_eq!(snapshots[0].consecutive_failures, 5);
        assert_eq!(snapshots[0].last_error.as_deref(), Some("boom"));
        assert_eq!(snapshots[0].last_result_status, ParserResultStatus::Failed);
        assert!(matches!(
            snapshots[0].circuit_state,
            RuntimeCircuitState::Open
        ));
    }

    #[tokio::test]
    async fn empty_parser_payload_is_degraded_in_dev_without_rejection() {
        let parser = Arc::new(MockParser {
            slug: "mock-empty-dev",
            response: Ok((Vec::new(), Vec::new())),
            delay_ms: 0,
        });
        let scanner = build_test_scanner_with_runtime(
            vec![parser],
            RuntimeProfile::Dev,
            FeatureFlag::Enabled,
        )
        .await;

        let results = GhostScanner::fetch_parsers_parallel(
            &scanner.parsers,
            &scanner.circuit_breakers,
            scanner.runtime_profile,
            scanner.request_timeout_secs,
            scanner.per_bookmaker_timeout_secs.clone(),
            scanner.parser_runtime.clone(),
            scanner.parser_execution_bulkhead.clone(),
            scanner.parser_result_caps,
        )
        .await;

        assert_eq!(results.len(), 1);
        let snapshot = scanner
            .parser_runtime_snapshots()
            .into_iter()
            .next()
            .expect("runtime snapshot");
        assert_eq!(snapshot.last_result_status, ParserResultStatus::Degraded);
        assert_eq!(snapshot.successful_runs, 0);
        assert_eq!(snapshot.total_runs, 1);
        assert_eq!(snapshot.last_error, None);
        assert!((snapshot.uptime_percent - 0.0).abs() < f64::EPSILON);
        assert!(snapshot
            .validation_checks
            .iter()
            .any(|check| check.code == "empty_payload"));
    }

    #[tokio::test]
    async fn empty_parser_payload_is_rejected_in_production() {
        let parser = Arc::new(MockParser {
            slug: "mock-empty-prod",
            response: Ok((Vec::new(), Vec::new())),
            delay_ms: 0,
        });
        let scanner = build_test_scanner_with_runtime(
            vec![parser],
            RuntimeProfile::Production,
            FeatureFlag::Disabled,
        )
        .await;

        let results = GhostScanner::fetch_parsers_parallel(
            &scanner.parsers,
            &scanner.circuit_breakers,
            scanner.runtime_profile,
            scanner.request_timeout_secs,
            scanner.per_bookmaker_timeout_secs.clone(),
            scanner.parser_runtime.clone(),
            scanner.parser_execution_bulkhead.clone(),
            scanner.parser_result_caps,
        )
        .await;

        assert!(results.is_empty());
        let snapshot = scanner
            .parser_runtime_snapshots()
            .into_iter()
            .next()
            .expect("runtime snapshot");
        assert_eq!(snapshot.last_result_status, ParserResultStatus::Failed);
        assert_eq!(snapshot.successful_runs, 0);
        assert_eq!(snapshot.total_runs, 1);
        assert_eq!(
            snapshot.last_error.as_deref(),
            Some("parser returned an empty payload")
        );
    }

    #[tokio::test]
    async fn parser_bulkhead_limits_parallel_fetches() {
        #[derive(Debug)]
        struct ConcurrencyProbe {
            active: AtomicUsize,
            peak: AtomicUsize,
        }

        impl ConcurrencyProbe {
            fn enter(&self) {
                let current = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                let mut observed = self.peak.load(Ordering::SeqCst);
                while current > observed {
                    match self.peak.compare_exchange(
                        observed,
                        current,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(actual) => observed = actual,
                    }
                }
            }

            fn exit(&self) {
                self.active.fetch_sub(1, Ordering::SeqCst);
            }

            fn peak(&self) -> usize {
                self.peak.load(Ordering::SeqCst)
            }
        }

        #[derive(Debug)]
        struct ConcurrencyTrackingParser {
            slug: &'static str,
            probe: Arc<ConcurrencyProbe>,
            delay_ms: u64,
        }

        #[async_trait]
        impl parsers::base::BookmakerParser for ConcurrencyTrackingParser {
            fn name(&self) -> &str {
                self.slug
            }

            fn slug(&self) -> &str {
                self.slug
            }

            fn is_enabled(&self) -> bool {
                true
            }

            async fn fetch_events(
                &self,
            ) -> Result<Vec<shared::Event>, Box<dyn std::error::Error + Send + Sync>> {
                self.fetch_all().await.map(|result| result.events)
            }

            async fn fetch_odds(
                &self,
                _event_id: &str,
            ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
                self.fetch_all().await.map(|result| result.odds)
            }

            async fn fetch_all(
                &self,
            ) -> Result<parsers::base::ParserResult, Box<dyn std::error::Error + Send + Sync>>
            {
                self.probe.enter();
                sleep(Duration::from_millis(self.delay_ms)).await;
                self.probe.exit();

                Ok(parsers::base::ParserResult::new(
                    self.slug,
                    vec![make_test_event(self.slug, self.slug)],
                    Vec::new(),
                    self.delay_ms,
                ))
            }

            fn base_url(&self) -> &str {
                "https://example.invalid"
            }

            fn user_agent(&self) -> &str {
                "mock"
            }
        }

        let probe = Arc::new(ConcurrencyProbe {
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        });
        let parsers: Vec<Arc<dyn parsers::base::BookmakerParser + Send + Sync>> = (0..5)
            .map(|idx| {
                Arc::new(ConcurrencyTrackingParser {
                    slug: Box::leak(format!("bulkhead-{idx}").into_boxed_str()),
                    probe: probe.clone(),
                    delay_ms: 75,
                }) as Arc<dyn parsers::base::BookmakerParser + Send + Sync>
            })
            .collect();

        let scanner = build_test_scanner_with_runtime(
            parsers,
            RuntimeProfile::Production,
            FeatureFlag::Disabled,
        )
        .await
        .with_parser_execution_config(&ScannerConfig {
            parallel_parsers: 5,
            production_parallel_parsers: Some(2),
            ..ScannerConfig::default()
        });

        let results = GhostScanner::fetch_parsers_parallel(
            &scanner.parsers,
            &scanner.circuit_breakers,
            scanner.runtime_profile,
            scanner.request_timeout_secs,
            scanner.per_bookmaker_timeout_secs.clone(),
            scanner.parser_runtime.clone(),
            scanner.parser_execution_bulkhead.clone(),
            scanner.parser_result_caps,
        )
        .await;

        assert_eq!(results.len(), 5);
        assert_eq!(scanner.parser_execution_parallelism(), 2);
        assert!(scanner.parser_execution_strict_mode());
        assert!(probe.peak() <= 2, "peak concurrency was {}", probe.peak());
    }
}
