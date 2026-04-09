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
use parsers::base::BookmakerParser;
use parsers::circuit_breaker::CircuitBreaker;
use shared::models::ScannerMetrics;
use shared::{EventBus, Event, Odd};
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
        }
    }

    pub async fn run_cycle(&self) -> ScannerMetrics {
        self.run_cycle_inner().await
    }

    pub async fn run_cycle_inner(&self) -> ScannerMetrics {
        eprintln!("[CYCLE] Starting...");
        info!("Starting scan cycle...");
        let cycle_start = Instant::now();

        eprintln!("[CYCLE] Fetching parsers...");
        let all_events = self.fetch_all_parsers().await;
        let total_raw: usize = all_events.iter().map(|e| e.events.len()).sum();
        eprintln!("[CYCLE] Parsers returned {} events from {} parsers", total_raw, all_events.len());
        info!("fetch_all_parsers returned {} raw events from {} parsers", total_raw, all_events.len());

        let all_odds: Vec<Odd> = all_events.iter().flat_map(|e| e.odds.clone()).collect();
        let events: Vec<Event> = all_events.iter().flat_map(|e| e.events.clone()).collect();
        eprintln!("[CYCLE] Flattened: {} events, {} odds", events.len(), all_odds.len());

        // Ограничиваем: берём больше событий для лучшего поиска вилок
        eprintln!("[CYCLE] Limiting events...");
        const MAX_EVENTS_FOR_CALC: usize = 500;
        let calc_events: Vec<Event> = if events.len() > MAX_EVENTS_FOR_CALC {
            eprintln!("[CYCLE] Taking first 500 of {} events", events.len());
            events.iter().take(MAX_EVENTS_FOR_CALC).cloned().collect()
        } else {
            events.clone()
        };
        eprintln!("[CYCLE] Limited to {} events", calc_events.len());

        // Фильтруем odds: берём больше для лучшей детекции
        eprintln!("[CYCLE] Filtering odds...");
        let calc_odds: Vec<Odd> = all_odds.iter().take(10000).cloned().collect();
        eprintln!("[CYCLE] Filtered to {} odds", calc_odds.len());

        eprintln!("[CYCLE] Processing {} events and {} odds...", calc_events.len(), calc_odds.len());

        eprintln!("[CYCLE] Normalizing events...");
        let normalized_events: Vec<Event> = calc_events
            .iter()
            .map(|e| self.normalizer.normalize_event(e.clone()))
            .collect();
        eprintln!("[CYCLE] Normalized {} events", normalized_events.len());

        for event in &normalized_events {
            self.event_pool.insert(event.clone());
        }
        eprintln!("[CYCLE] Inserted events into pool");

        eprintln!("[CYCLE] Calling calculator with {} events, {} odds...", normalized_events.len(), calc_odds.len());
        eprintln!("[CYCLE] First event: {:?}", normalized_events.first().map(|e| &e.id));
        eprintln!("[CYCLE] First odd: {:?}", calc_odds.first());

        let surebets = self.calculator.find_surebets(&normalized_events, &calc_odds);
        eprintln!("[CYCLE] Calculator returned {} surebets", surebets.len());

        let cycle_time = cycle_start.elapsed().as_millis() as u64;
        eprintln!("[CYCLE] Cycle time: {}ms", cycle_time);

        let active = {
            let breakers = self.circuit_breakers.lock().unwrap();
            breakers.values().filter(|cb| cb.allow_request()).count()
        };
        let failed = self.parsers.len() - active;
        eprintln!("[CYCLE] Active: {}, Failed: {}", active, failed);

        let metrics = ScannerMetrics {
            cycle_time_ms: cycle_time,
            events_parsed: normalized_events.len(),
            surebets_found: surebets.len(),
            active_bookmakers: active,
            failed_bookmakers: failed,
            cache_hit_rate: 0.0,
            memory_mb: 0.0,
            timestamp: Utc::now(),
        };
        eprintln!("[CYCLE] Created metrics: events={}, surebets={}", metrics.events_parsed, metrics.surebets_found);

        // Update global scanner state for API
        // Don't borrow state_rx — just create new state directly
        let next_cycle_count = self.state_rx.borrow().cycle_count + 1;
        let new_state = ScannerState {
            running: true,
            last_metrics: Some(metrics.clone()),
            cycle_count: next_cycle_count,
        };
        eprintln!("[CYCLE] Sending state...");
        use std::io::Write;
        let _ = std::io::stderr().flush();
        // Use send_replace to avoid blocking
        let _ = self.state_tx.send(new_state);
        eprintln!("[CYCLE] State sent");
        let _ = std::io::stderr().flush();
        eprintln!("[CYCLE] Cycle complete");

        metrics
    }

    pub async fn start(&self) {
        eprintln!("[SCANNER] start() called, acquiring running lock...");
        {
            let mut running = self.running.lock().unwrap();
            *running = true;
        }
        eprintln!("[SCANNER] running set to true, entering scan loop");
        info!("GhostScanner started, entering scan loop");

        loop {
            let is_running = { *self.running.lock().unwrap() };
            if !is_running {
                eprintln!("[SCANNER] loop exiting");
                info!("GhostScanner loop exiting");
                break;
            }

            eprintln!("[SCANNER] starting scan cycle...");
            info!("Starting scan cycle...");
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
            let _ = self.state_tx.send(ScannerState {
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

    async fn fetch_all_parsers(&self) -> Vec<parsers::base::ParserResult> {
        use futures::future::join_all;

        eprintln!("[PARSERS] fetch_all_parsers starting for {} parsers", self.parsers.len());
        info!("fetch_all_parsers: starting parallel fetch for {} parsers", self.parsers.len());

        let mut futures = Vec::new();

        for parser in self.parsers.iter() {
            let slug = parser.slug().to_string();
            let parser = parser.clone();

            // Проверяем circuit breaker
            let skip = {
                let breakers = self.circuit_breakers.lock().unwrap();
                breakers.get(&slug).map(|cb| !cb.allow_request()).unwrap_or(false)
            };

            if skip {
                warn!(parser = %slug, "Circuit breaker open, skipping");
                continue;
            }

            info!(parser = %slug, "Starting parser fetch");
            let fut = async move {
                info!(parser = %slug, "Parser fetch beginning");
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(180),
                    parser.fetch_all()
                ).await;

                match result {
                    Ok(Ok(r)) => {
                        info!(parser = %slug, events = r.events.len(), odds = r.odds.len(), "Parser OK");
                        Some(r)
                    }
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

        info!("Running {} parser futures in parallel", futures.len());
        let results: Vec<Option<_>> = join_all(futures).await;
        let count = results.iter().filter(|r| r.is_some()).count();
        info!("fetch_all_parsers completed: {} parsers returned results", count);
        results.into_iter().flatten().collect()
    }
}
