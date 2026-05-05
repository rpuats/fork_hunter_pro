//! Scanner Bridge - Connects scanner events to execution orchestrator

use crate::{
    auth::AuthManager,
    betting::{BetInstruction, BetMode, BettingError, OperatorQueue, item_factory},
    execution::{ExecutionOrchestrator, Fork, ForkLeg, ForkStatus, AccountReadiness},
    BrowserPool,
    performance::get_global_monitor,
};
use anyhow::Result;
use base64;
use engine::filters::{FilterConfig, FilterEngine};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use shared::BusEvent;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Bridge between scanner and execution
pub struct ScannerBridge {
    orchestrator: Arc<TokioMutex<ExecutionOrchestrator>>,
    auth_manager: Arc<TokioMutex<AuthManager>>,
    browser_pool: Arc<BrowserPool>,
    operator_queue: Arc<TokioMutex<OperatorQueue>>,
    filter_engine: FilterEngine,
}

impl ScannerBridge {
    pub fn new(
        orchestrator: Arc<TokioMutex<ExecutionOrchestrator>>,
        auth_manager: Arc<TokioMutex<AuthManager>>,
        browser_pool: Arc<BrowserPool>,
        operator_queue: Arc<TokioMutex<OperatorQueue>>,
    ) -> Self {
        Self {
            orchestrator,
            auth_manager,
            browser_pool,
            operator_queue,
            filter_engine: FilterEngine::new(),
        }
    }

    /// Handle scanner event
    pub async fn handle_event(&self, event: BusEvent) {
        match event {
            BusEvent::SurebetDetected { payload, .. } => {
                info!("Surebet detected");
                // self.process_surebet(surebet).await;
            }
            BusEvent::OddsChanged { event_id, bookmaker, old_odds, new_odds, .. } => {
                // STUB: ignoring odds changes for now
                let _ = (event_id, bookmaker, old_odds, new_odds);
            }
            BusEvent::EventExpired { event_id, .. } => {
                self.handle_event_expired(event_id).await;
            }
            _ => {}
        }
    }

    /// Process detected surebet
    async fn process_surebet(&self, surebet: shared::Surebet) {
        let start = std::time::Instant::now();

        // Check if we should process this surebet
        if !self.should_process_surebet(&surebet).await {
            return;
        }

        // Convert surebet to fork
        let fork = self.convert_surebet_to_fork(&surebet);

        // Check account readiness for all bookmakers
        let ready = self.check_account_readiness(&fork).await;
        if !ready {
            warn!("Accounts not ready for fork: {}", fork.id);
            return;
        }

        // Start fork execution
        let mut orchestrator = self.orchestrator.lock().await;
        let fork_id = match orchestrator.on_fork_detected(fork.clone()) {
            Some(id) => id,
            None => {
                warn!("Fork rejected by orchestrator: {}", fork.id);
                return;
            }
        };

        // Calculate stakes
        let allocations = orchestrator.calculate_stakes(fork_id);
        drop(orchestrator);

        // Create bet instructions for each leg
        for (i, leg) in fork.legs.iter().enumerate() {
            if let Some(allocation) = allocations.get(i) {
                let bet = BetInstruction::new(
                    fork_id,
                    leg.bookmaker.clone(),
                    fork.event.clone(),
                    leg.market.clone(),
                    leg.selection.clone(),
                    leg.odds,
                    allocation.stake,
                    self.get_execution_mode().await,
                );

                // Execute based on mode
                match self.get_execution_mode().await {
                    BetMode::Auto => {
                        self.execute_auto_bet(bet).await;
                    }
                    BetMode::SemiAuto => {
                        self.prepare_semi_auto_bet(bet).await;
                    }
                    BetMode::Manual => {
                        self.prepare_manual_bet(bet).await;
                    }
                }
            }
        }

        let duration = start.elapsed();
        let duration_ms = duration.as_secs_f64() * 1000.0;

        // Record performance metric
        if let Some(monitor) = get_global_monitor() {
            monitor.record("fork_to_display", duration_ms, true).await;
        }

        info!("Fork {} execution started in {:.1}ms", fork_id, duration_ms);
    }

    /// Check if surebet should be processed
    async fn should_process_surebet(&self, surebet: &shared::Surebet) -> bool {
        // Check filters
        if !self.filter_engine.should_process_surebet(surebet) {
            return false;
        }

        // Check orchestrator limits
        let orchestrator = self.orchestrator.lock().await;
        if orchestrator.get_state().should_stop_due_to_limits() {
            warn!("Stopping due to limits reached");
            return false;
        }

        true
    }

    /// Check if all required accounts are ready
    async fn check_account_readiness(&self, fork: &Fork) -> bool {
        let auth_manager = self.auth_manager.lock().await;
        
        for bookmaker in &fork.bookmakers {
            if let Some(creds) = auth_manager.get_credentials(bookmaker) {
                let readiness = AccountReadiness {
                    bookmaker: bookmaker.clone(),
                    authenticated: creds.status == crate::auth::AuthStatus::Authenticated,
                    balance: creds.balance.unwrap_or(Decimal::ZERO),
                    session_valid: creds.cookies.is_some(),
                    last_check: chrono::Utc::now(),
                    can_place_bets: creds.balance.unwrap_or(Decimal::ZERO) > Decimal::ZERO,
                };

                if !readiness.is_ready() {
                    warn!("Account {} not ready: {:?}", bookmaker, readiness);
                    return false;
                }
            } else {
                warn!("No credentials for {}", bookmaker);
                return false;
            }
        }

        true
    }

    /// Convert surebet to fork
    fn convert_surebet_to_fork(&self, surebet: &shared::Surebet) -> Fork {
        let legs: Vec<ForkLeg> = surebet.legs.iter().map(|leg| {
            ForkLeg {
                bookmaker: leg.bookmaker.clone(),
                market: leg.market.clone(),
                selection: leg.selection.clone(),
                odds: Decimal::from_f64(leg.odds).unwrap_or(Decimal::ZERO),
                stake: Decimal::ZERO, // Will be calculated
            }
        }).collect();

        let bookmakers: Vec<String> = legs.iter().map(|l| l.bookmaker.clone()).collect();

        Fork {
            id: Uuid::new_v4(),
            bookmakers,
            event: format!("{} vs {}", surebet.home_team, surebet.away_team),
            sport: surebet.sport.to_string(),
            league: surebet.league.clone(),
            profit_percent: Decimal::from_f64(surebet.profit_percent).unwrap_or(Decimal::ZERO),
            legs,
            detected_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(60),
        }
    }

    /// Get current execution mode
    async fn get_execution_mode(&self) -> BetMode {
        let orchestrator = self.orchestrator.lock().await;
        orchestrator.get_mode()
    }

    /// Execute auto bet
    async fn execute_auto_bet(&self, bet: BetInstruction) {
        info!("Executing auto bet: {} on {}", bet.id, bet.bookmaker_id);

        // Get session
        let session = {
            let auth_manager = self.auth_manager.lock().await;
            auth_manager.get_session(&bet.bookmaker_id).cloned()
        };

        if session.is_none() {
            error!("No session for {}", bet.bookmaker_id);
            return;
        }

        let session = session.unwrap();

        // Get browser
        let browser = match self.browser_pool.get_browser().await {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to get browser: {}", e);
                return;
            }
        };

        // Place bet (using session.cookies)
        match crate::betting::place_auto_bet(&bet, &session.cookies, &browser).await {
            Ok(result) => {
                info!("Auto bet placed: {:?}", result);
                
                // Update orchestrator
                let mut orchestrator = self.orchestrator.lock().await;
                orchestrator.get_state_mut().complete_bet(&bet.id, result);
            }
            Err(e) => {
                error!("Auto bet failed: {}", e);
                
                // Clone data before locking to avoid borrow issues
                let bet_id = bet.id.clone();
                let bet_fork_id = bet.fork_id;
                let bet_bookmaker_id = bet.bookmaker_id.clone();
                let bet_event_name = bet.event_name.clone();
                let bet_market = bet.market.clone();
                let bet_selection = bet.selection.clone();
                let bet_odds = bet.odds;
                let bet_stake = bet.stake;
                
                // Add to operator queue for manual handling
                let mut queue = self.operator_queue.lock().await;
                queue.push(item_factory::bet_confirmation(
                    bet_id,
                    bet_fork_id,
                    bet_bookmaker_id,
                    bet_event_name,
                    bet_market,
                    bet_selection,
                    bet_odds,
                    bet_stake,
                    bet_odds,
                    None,
                    60,
                ));
            }
        }
    }

    /// Prepare semi-auto bet
    async fn prepare_semi_auto_bet(&self, bet: BetInstruction) {
        info!("Preparing semi-auto bet: {} on {}", bet.id, bet.bookmaker_id);

        // Get session
        let session = {
            let auth_manager = self.auth_manager.lock().await;
            auth_manager.get_session(&bet.bookmaker_id).cloned()
        };

        if session.is_none() {
            error!("No session for {}", bet.bookmaker_id);
            return;
        }

        let session = session.unwrap();

        // Get browser
        let browser = match self.browser_pool.get_browser().await {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to get browser: {}", e);
                return;
            }
        };

        // Create channels for operator communication
        let (operator_tx, _operator_event_rx) = tokio::sync::mpsc::channel::<crate::betting::OperatorEvent>(10);
        let (_response_tx, mut operator_rx) = tokio::sync::mpsc::channel::<crate::betting::OperatorResponse>(10);

        // Place semi-auto bet (using session.cookies)
        match crate::betting::place_semi_auto_bet(&bet, &session.cookies, &browser, operator_tx, &mut operator_rx).await {
            Ok(result) => {
                info!("Semi-auto bet completed: {:?}", result);
                
                // Update orchestrator
                let mut orchestrator = self.orchestrator.lock().await;
                orchestrator.get_state_mut().complete_bet(&bet.id, result);
            }
            Err(e) => {
                error!("Semi-auto bet failed: {}", e);
            }
        }
    }

    /// Prepare manual bet
    async fn prepare_manual_bet(&self, bet: BetInstruction) {
        info!("Preparing manual bet: {} on {}", bet.id, bet.bookmaker_id);

        // Get session
        let session = {
            let auth_manager = self.auth_manager.lock().await;
            auth_manager.get_session(&bet.bookmaker_id).cloned()
        };

        if session.is_none() {
            error!("No session for {}", bet.bookmaker_id);
            return;
        }

        let session = session.unwrap();

        // Get browser
        let browser = match self.browser_pool.get_browser().await {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to get browser: {}", e);
                return;
            }
        };

        // Prepare manual bet (using session.cookies)
        match crate::betting::prepare_manual_bet(&bet, &session.cookies, &browser).await {
            Ok(result) => {
                info!("Manual bet prepared: {:?}", result);
                
                // Add to operator queue
                let mut queue = self.operator_queue.lock().await;
                queue.push(item_factory::bet_confirmation(
                    bet.id.clone(),
                    bet.fork_id,
                    bet.bookmaker_id.clone(),
                    bet.event_name.clone(),
                    bet.market.clone(),
                    bet.selection.clone(),
                    bet.odds,
                    bet.stake,
                    bet.odds,
                    result.screenshot.map(|s| base64::encode(&s)),
                    300, // 5 minutes for manual
                ));
            }
            Err(e) => {
                error!("Manual bet preparation failed: {}", e);
            }
        }
    }

    /// Handle odds changed event
    async fn handle_odds_changed(&self, event_id: String, bookmaker: String, old_odds: Decimal, new_odds: Decimal) {
        let orchestrator = self.orchestrator.lock().await;
        let state = orchestrator.get_state();

        // Find affected forks
        for (fork_id, execution) in &state.active_forks {
            for leg in &execution.fork.legs {
                if leg.bookmaker == bookmaker {
                    warn!("Odds changed for fork {}: {} -> {}", fork_id, old_odds, new_odds);
                    
                    // Add to operator queue
                    let mut queue = self.operator_queue.lock().await;
                    queue.push(item_factory::odds_changed(
                        *fork_id,
                        bookmaker.clone(),
                        old_odds,
                        new_odds,
                    ));
                }
            }
        }
    }

    /// Handle event expired
    async fn handle_event_expired(&self, event_id: String) {
        let mut orchestrator = self.orchestrator.lock().await;
        
        // Find affected forks and mark as expired
        let state = orchestrator.get_state_mut();
        for (fork_id, execution) in &mut state.active_forks.clone() {
            // If event matches, expire the fork
            if execution.fork.event.contains(&event_id) {
                state.update_fork_status(*fork_id, ForkStatus::Expired);
                info!("Fork {} expired due to event expiry", fork_id);
            }
        }
    }
}

/// Spawn scanner bridge task
pub fn spawn_scanner_bridge(
    orchestrator: Arc<TokioMutex<ExecutionOrchestrator>>,
    auth_manager: Arc<TokioMutex<AuthManager>>,
    browser_pool: Arc<BrowserPool>,
    operator_queue: Arc<TokioMutex<OperatorQueue>>,
    mut event_rx: tokio::sync::mpsc::Receiver<BusEvent>,
) -> tokio::task::JoinHandle<()> {
    let bridge = ScannerBridge::new(
        orchestrator,
        auth_manager,
        browser_pool,
        operator_queue,
    );

    tokio::spawn(async move {
        info!("Scanner bridge started");
        
        while let Some(event) = event_rx.recv().await {
            bridge.handle_event(event).await;
        }
        
        info!("Scanner bridge stopped");
    })
}
