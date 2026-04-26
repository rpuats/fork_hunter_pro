use chrono::Utc;
use shared::{
    AutoBetConfig, AutoBetStatus, BetExecutionRequest, BetExecutionStatus, BetPlacement, BetResult,
    BetStatus, Event, StakeValidationDecision, StakeValidationRequest, Surebet,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

use super::approval::{build_surebet_execution_plan, ApprovalGateDecision, SurebetExecutionPlan};
use super::limiter::{BetLimiter, BetLimiterStats};
use super::persistence::{
    ExecutionLedgerAction, ExecutionLedgerEntry, ExecutionLedgerPersistence,
    ExecutionStatePersistence,
};
use super::registry::ExecutionRegistry;
use super::state_machine::{
    ExecutionStateMachine, ExecutionStateSnapshot, ExecutionStateTransition,
};
use super::stealth::StealthBetting;
use super::validator::StakeValidator;

#[derive(Clone)]
pub struct AutoBetEngine {
    config: Arc<parking_lot::RwLock<AutoBetConfig>>,
    limiter: Arc<parking_lot::Mutex<BetLimiter>>,
    stealth: Arc<StealthBetting>,
    registry: Arc<ExecutionRegistry>,
    history: Arc<parking_lot::Mutex<Vec<BetPlacement>>>,
    state_snapshots: Arc<parking_lot::Mutex<HashMap<uuid::Uuid, ExecutionStateSnapshot>>>,
    ledger: Option<Arc<dyn ExecutionLedgerPersistence>>,
    state_persistence: Option<Arc<dyn ExecutionStatePersistence>>,
    running: Arc<parking_lot::RwLock<bool>>,
    emergency_stopped: Arc<parking_lot::RwLock<bool>>,
    total_profit: Arc<parking_lot::RwLock<f64>>,
    today_profit: Arc<parking_lot::RwLock<f64>>,
    bets_today: Arc<parking_lot::RwLock<u32>>,
    bets_total: Arc<parking_lot::RwLock<u64>>,
    errors_today: Arc<parking_lot::RwLock<u32>>,
}

impl AutoBetEngine {
    pub fn new(config: AutoBetConfig) -> Self {
        Self::with_registry(config, Arc::new(ExecutionRegistry::new()))
    }

    pub fn with_registry(config: AutoBetConfig, registry: Arc<ExecutionRegistry>) -> Self {
        Self::new_inner(config, registry, None, None)
    }

    pub fn with_registry_and_ledger(
        config: AutoBetConfig,
        registry: Arc<ExecutionRegistry>,
        ledger: Arc<dyn ExecutionLedgerPersistence>,
    ) -> Self {
        Self::new_inner(config, registry, Some(ledger), None)
    }

    pub fn with_registry_ledger_and_state(
        config: AutoBetConfig,
        registry: Arc<ExecutionRegistry>,
        ledger: Arc<dyn ExecutionLedgerPersistence>,
        state_persistence: Arc<dyn ExecutionStatePersistence>,
    ) -> Self {
        Self::new_inner(config, registry, Some(ledger), Some(state_persistence))
    }

    fn new_inner(
        config: AutoBetConfig,
        registry: Arc<ExecutionRegistry>,
        ledger: Option<Arc<dyn ExecutionLedgerPersistence>>,
        state_persistence: Option<Arc<dyn ExecutionStatePersistence>>,
    ) -> Self {
        let limiter = BetLimiter::new(
            config.max_bets_per_hour,
            config.max_daily_stake,
            config.delay_between_bets_ms,
        );

        Self {
            config: Arc::new(parking_lot::RwLock::new(config)),
            limiter: Arc::new(parking_lot::Mutex::new(limiter)),
            stealth: Arc::new(StealthBetting::new()),
            registry,
            history: Arc::new(parking_lot::Mutex::new(Vec::new())),
            state_snapshots: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            ledger,
            state_persistence,
            running: Arc::new(parking_lot::RwLock::new(false)),
            emergency_stopped: Arc::new(parking_lot::RwLock::new(false)),
            total_profit: Arc::new(parking_lot::RwLock::new(0.0)),
            today_profit: Arc::new(parking_lot::RwLock::new(0.0)),
            bets_today: Arc::new(parking_lot::RwLock::new(0)),
            bets_total: Arc::new(parking_lot::RwLock::new(0)),
            errors_today: Arc::new(parking_lot::RwLock::new(0)),
        }
    }

    pub async fn plan_surebet_execution(
        &self,
        surebet: &Surebet,
    ) -> Result<SurebetExecutionPlan, String> {
        build_surebet_execution_plan(self.registry.as_ref(), surebet).await
    }

    pub async fn place_surebet(&self, surebet: &Surebet) -> Result<Vec<BetPlacement>, String> {
        if !*self.running.read() {
            return Err("Auto-betting is not running".into());
        }

        if *self.emergency_stopped.read() {
            return Err("Emergency stop activated".into());
        }

        if surebet.profit_percent < self.config.read().min_profit_percent {
            return Err("Profit below minimum threshold".into());
        }

        let execution_plan = self.plan_surebet_execution(surebet).await?;
        if !execution_plan.executable {
            let reasons = execution_plan.blocking_reasons().join("; ");
            return Err(if reasons.is_empty() {
                "surebet execution blocked by rollout approval gate".into()
            } else {
                reasons
            });
        }

        let mut placements: Vec<BetPlacement> = Vec::new();

        for leg_plan in execution_plan.ranked_legs {
            if matches!(
                leg_plan.decision,
                ApprovalGateDecision::Reject | ApprovalGateDecision::RequireOperatorApproval
            ) {
                return Err(leg_plan.reasons.join("; "));
            }

            let leg = &leg_plan.leg;
            self.stealth.wait_stealth().await;

            let stake = leg_plan.validation.adjusted_stake;

            {
                let mut limiter = self.limiter.lock();
                if let Err(e) = limiter.can_bet(stake) {
                    warn!(error = e.to_string(), "Bet limit reached");
                    return Err(e.to_string());
                }
            }

            let event = Event {
                id: format!("surebet-{}-{}", surebet.id, leg.bookmaker),
                sport: surebet.sport.clone(),
                league: surebet.league.clone(),
                home_team: surebet.home_team.clone(),
                away_team: surebet.away_team.clone(),
                start_time: surebet.start_time,
                is_live: surebet.is_live,
                bookmaker_slug: leg.bookmaker.clone(),
                raw_url: leg.url.clone(),
                extra: Default::default(),
            };

            let execution_request = BetExecutionRequest {
                bookmaker: leg.bookmaker.clone(),
                event_id: event.id.clone(),
                market: leg.market.clone(),
                selection: leg.selection.clone(),
                odds: leg.odds,
                stake,
                allow_dry_run: !matches!(leg_plan.decision, ApprovalGateDecision::AllowSubmission),
                reference: Some(surebet.id.to_string()),
            };

            let execution = self.registry.execute_bet(&execution_request).await?;
            let placement_status = placement_status_from_execution(&execution.status);
            let placement_error = match execution.status {
                BetExecutionStatus::Blocked | BetExecutionStatus::Rejected => {
                    execution.message.clone()
                }
                _ => None,
            };

            let placement = BetPlacement {
                id: uuid::Uuid::new_v4(),
                bookmaker: leg.bookmaker.clone(),
                event,
                market: leg.market.clone(),
                selection: leg.selection.clone(),
                odds: leg.odds,
                stake,
                status: placement_status,
                placed_at: Utc::now(),
                execution: Some(execution),
                result: None,
                error: placement_error,
            };

            self.limiter.lock().record_bet(stake);
            if matches!(placement.status, BetStatus::Error) {
                // Best-effort rollback for already accepted legs when fork execution breaks mid-flight.
                for executed in &placements {
                    if matches!(executed.status, BetStatus::Pending | BetStatus::Placed) {
                        let _ = self.cancel_bet(executed.id);
                    }
                }
                return Err(placement
                    .error
                    .clone()
                    .unwrap_or_else(|| "execution leg failed".to_string()));
            }
            placements.push(placement);
        }

        for p in &placements {
            self.history.lock().push(p.clone());
            self.record_ledger_entry(ExecutionLedgerAction::Placed, p);
        }

        *self.bets_today.write() += placements.len() as u32;
        *self.bets_total.write() += placements.len() as u64;

        info!(
            surebet_id = surebet.id.to_string(),
            legs = placements.len(),
            "Surebet placed"
        );
        Ok(placements)
    }

    pub fn record_result(&self, bet_id: uuid::Uuid, result: BetResult) {
        let mut history = self.history.lock();
        for bet in history.iter_mut() {
            if bet.id == bet_id {
                bet.status = BetStatus::Settled;
                bet.result = Some(result.clone());

                match &result {
                    BetResult::Won(payout) => {
                        let profit = payout - bet.stake;
                        *self.total_profit.write() += profit;
                        *self.today_profit.write() += profit;
                        info!(bet_id = bet_id.to_string(), profit, "Bet won");
                    }
                    BetResult::Lost => {
                        let loss = -bet.stake;
                        *self.total_profit.write() += loss;
                        *self.today_profit.write() += loss;
                        info!(bet_id = bet_id.to_string(), loss, "Bet lost");
                    }
                    BetResult::Void => {
                        info!(bet_id = bet_id.to_string(), "Bet voided");
                    }
                    BetResult::Cashout(amount) => {
                        let profit = amount - bet.stake;
                        *self.total_profit.write() += profit;
                        *self.today_profit.write() += profit;
                        info!(bet_id = bet_id.to_string(), profit, "Bet cashed out");
                    }
                }
                let snapshot = bet.clone();
                self.record_ledger_entry(ExecutionLedgerAction::Updated, &snapshot);
                break;
            }
        }
    }

    pub fn emergency_stop(&self) {
        *self.emergency_stopped.write() = true;
        *self.running.write() = false;
        error!("Emergency stop activated");
    }

    pub fn start(&self) {
        *self.running.write() = true;
        *self.emergency_stopped.write() = false;
        info!("Auto-betting engine started");
    }

    pub fn stop(&self) {
        *self.running.write() = false;
        info!("Auto-betting engine stopped");
    }

    pub fn get_status(&self) -> AutoBetStatus {
        AutoBetStatus {
            enabled: self.config.read().enabled,
            running: *self.running.read(),
            bets_placed_today: *self.bets_today.read(),
            bets_placed_total: *self.bets_total.read(),
            profit_today: *self.today_profit.read(),
            profit_total: *self.total_profit.read(),
            last_bet: self.history.lock().last().map(|b| b.placed_at),
            errors_today: *self.errors_today.read(),
            emergency_stopped: *self.emergency_stopped.read(),
        }
    }

    pub fn get_limiter_stats(&self) -> BetLimiterStats {
        self.limiter.lock().get_stats()
    }

    /// Разместить одну ставку (обёртка для прямого вызова)
    pub async fn place_bet(
        &self,
        bookmaker: &str,
        event_id: &str,
        market: &str,
        selection: &str,
        odds: f64,
        stake: f64,
    ) -> Result<BetPlacement, String> {
        if !*self.running.read() {
            return Err("Auto-betting is not running".into());
        }
        if *self.emergency_stopped.read() {
            return Err("Emergency stop activated".into());
        }

        {
            let mut limiter = self.limiter.lock();
            if let Err(e) = limiter.can_bet(stake) {
                warn!(error = e.to_string(), "Bet limit reached");
                return Err(e.to_string());
            }
        }

        let validation = StakeValidator::validate(&StakeValidationRequest {
            bookmaker: bookmaker.to_string(),
            desired_stake: stake,
            min_stake: None,
            max_stake: Some(self.config.read().max_stake_per_bet),
            bookmaker_available_balance: None,
            bankroll_available_balance: None,
            allow_auto_adjust: true,
        });
        if matches!(validation.decision, StakeValidationDecision::Reject) {
            return Err(validation.reasons.join("; "));
        }

        let adjusted_stake = validation.adjusted_stake;

        let event = Event {
            id: event_id.to_string(),
            sport: shared::Sport::Football,
            league: String::new(),
            home_team: String::new(),
            away_team: String::new(),
            start_time: None,
            is_live: false,
            bookmaker_slug: bookmaker.to_string(),
            raw_url: None,
            extra: Default::default(),
        };

        let execution_request = BetExecutionRequest {
            bookmaker: bookmaker.to_string(),
            event_id: event_id.to_string(),
            market: market.to_string(),
            selection: selection.to_string(),
            odds,
            stake: adjusted_stake,
            allow_dry_run: true,
            reference: None,
        };

        let execution = self.registry.execute_bet(&execution_request).await?;
        let placement_status = placement_status_from_execution(&execution.status);
        let placement_error = match execution.status {
            BetExecutionStatus::Blocked | BetExecutionStatus::Rejected => execution.message.clone(),
            _ => None,
        };

        let placement = BetPlacement {
            id: uuid::Uuid::new_v4(),
            bookmaker: bookmaker.to_string(),
            event,
            market: market.to_string(),
            selection: selection.to_string(),
            odds,
            stake: adjusted_stake,
            status: placement_status,
            placed_at: Utc::now(),
            execution: Some(execution),
            result: None,
            error: placement_error,
        };

        self.limiter.lock().record_bet(adjusted_stake);
        self.history.lock().push(placement.clone());
        self.record_ledger_entry(ExecutionLedgerAction::Placed, &placement);
        *self.bets_today.write() += 1;
        *self.bets_total.write() += 1;

        Ok(placement)
    }

    /// Отменить ставку (пометить как cancelled)
    pub fn cancel_bet(&self, bet_id: uuid::Uuid) -> Result<(), String> {
        let mut history = self.history.lock();
        for bet in history.iter_mut() {
            if bet.id == bet_id {
                if matches!(bet.status, BetStatus::Placed | BetStatus::Pending) {
                    bet.status = BetStatus::Cancelled;
                    info!(bet_id = bet_id.to_string(), "Bet cancelled");
                    let snapshot = bet.clone();
                    self.record_ledger_entry(ExecutionLedgerAction::Updated, &snapshot);
                    return Ok(());
                }
                return Err(format!("Cannot cancel bet in state: {:?}", bet.status));
            }
        }
        Err("Bet not found".to_string())
    }

    pub fn get_history(&self, limit: usize) -> Vec<BetPlacement> {
        let history = self.history.lock();
        history.iter().rev().take(limit).cloned().collect()
    }

    pub fn execution_registry(&self) -> Arc<ExecutionRegistry> {
        Arc::clone(&self.registry)
    }

    pub fn reset_daily(&self) {
        *self.today_profit.write() = 0.0;
        *self.bets_today.write() = 0;
        *self.errors_today.write() = 0;
        self.limiter.lock().reset_daily();
    }

    fn record_ledger_entry(&self, action: ExecutionLedgerAction, placement: &BetPlacement) {
        let entry = ExecutionLedgerEntry {
            placement: placement.clone(),
            action,
            recorded_at: Utc::now(),
        };

        if let Some(ledger) = &self.ledger {
            ledger.record(entry.clone());
        }

        let previous = self.latest_execution_state_snapshot(placement.id);
        let Ok((snapshot, transition)) =
            ExecutionStateMachine::snapshot_from_entry(previous.as_ref(), &entry)
        else {
            warn!(placement_id = %placement.id, "Skipping execution state persistence because transition validation failed");
            return;
        };

        self.state_snapshots
            .lock()
            .insert(snapshot.placement_id, snapshot.clone());

        if let Some(state_persistence) = self.state_persistence.as_ref().map(Arc::clone) {
            spawn_state_persistence(state_persistence, snapshot, transition);
        }
    }

    fn latest_execution_state_snapshot(
        &self,
        placement_id: uuid::Uuid,
    ) -> Option<ExecutionStateSnapshot> {
        self.state_snapshots.lock().get(&placement_id).cloned()
    }
}

fn spawn_state_persistence(
    persistence: Arc<dyn ExecutionStatePersistence>,
    snapshot: ExecutionStateSnapshot,
    transition: ExecutionStateTransition,
) {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            if let Err(error) = persistence.record_transition(&transition).await {
                tracing::warn!(error = %error, placement_id = %transition.placement_id, "execution state transition persistence failed");
                return;
            }

            if let Err(error) = persistence.save_snapshot(&snapshot).await {
                tracing::warn!(error = %error, placement_id = %snapshot.placement_id, "execution state snapshot persistence failed");
            }
        });
    } else {
        tracing::warn!(placement_id = %snapshot.placement_id, "execution state persistence skipped because no Tokio runtime is active");
    }
}

fn placement_status_from_execution(status: &BetExecutionStatus) -> BetStatus {
    match status {
        BetExecutionStatus::Pending | BetExecutionStatus::DryRun | BetExecutionStatus::Armed => {
            BetStatus::Pending
        }
        BetExecutionStatus::Submitted | BetExecutionStatus::Accepted => BetStatus::Placed,
        BetExecutionStatus::Rejected | BetExecutionStatus::Blocked => BetStatus::Error,
        BetExecutionStatus::Skipped => BetStatus::Cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::ApprovalGateDecision;
    use crate::state_machine::ExecutionStatePhase;
    use shared::{
        AutoBetConfig, BookmakerAccount, BookmakerBalanceSnapshot, BookmakerExecutionMode,
        BookmakerSession, BookmakerSessionState, Surebet, SurebetLeg,
    };
    use std::sync::Mutex;
    use tokio::sync::Mutex as AsyncMutex;
    use uuid::Uuid;

    #[derive(Default)]
    struct TestLedger {
        entries: Mutex<Vec<ExecutionLedgerEntry>>,
    }

    impl ExecutionLedgerPersistence for TestLedger {
        fn record(&self, entry: ExecutionLedgerEntry) {
            self.entries.lock().unwrap().push(entry);
        }
    }

    #[derive(Default)]
    struct TestStatePersistence {
        snapshots: AsyncMutex<Vec<ExecutionStateSnapshot>>,
        transitions: AsyncMutex<Vec<ExecutionStateTransition>>,
    }

    #[async_trait::async_trait]
    impl ExecutionStatePersistence for TestStatePersistence {
        async fn load_snapshots(&self) -> Result<Vec<ExecutionStateSnapshot>, String> {
            Ok(self.snapshots.lock().await.clone())
        }

        async fn save_snapshot(&self, snapshot: &ExecutionStateSnapshot) -> Result<(), String> {
            self.snapshots.lock().await.push(snapshot.clone());
            Ok(())
        }

        async fn record_transition(
            &self,
            transition: &ExecutionStateTransition,
        ) -> Result<(), String> {
            self.transitions.lock().await.push(transition.clone());
            Ok(())
        }
    }

    fn make_test_surebet() -> Surebet {
        Surebet {
            id: uuid::Uuid::new_v4(),
            sport: shared::Sport::Football,
            league: "Test League".into(),
            home_team: "Team A".into(),
            away_team: "Team B".into(),
            start_time: None,
            is_live: false,
            profit_percent: 5.0,
            total_stake: 1000.0,
            legs: vec![
                SurebetLeg {
                    bookmaker: "bk1".into(),
                    market: "1X2".into(),
                    selection: "1".into(),
                    odds: 2.10,
                    line: None,
                    stake: 500.0,
                    payout: 1050.0,
                    url: None,
                },
                SurebetLeg {
                    bookmaker: "bk2".into(),
                    market: "1X2".into(),
                    selection: "2".into(),
                    odds: 2.10,
                    line: None,
                    stake: 500.0,
                    payout: 1050.0,
                    url: None,
                },
            ],
            detected_at: Utc::now(),
            verified: false,
            mirror: false,
        }
    }

    fn register_ready_bookmaker(
        registry: &ExecutionRegistry,
        bookmaker: &str,
        mode: BookmakerExecutionMode,
    ) {
        let account_id = Uuid::new_v4();
        registry.register_account(BookmakerAccount {
            id: account_id,
            bookmaker: bookmaker.into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode,
            created_at: Utc::now(),
            last_used_at: None,
        });
        registry.upsert_session(BookmakerSession {
            account_id,
            bookmaker: bookmaker.into(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id,
            bookmaker: bookmaker.into(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 8_000.0,
            exposure: 2_000.0,
            captured_at: Utc::now(),
        });
    }

    fn make_rollout_surebet() -> Surebet {
        Surebet {
            id: Uuid::new_v4(),
            sport: shared::Sport::Football,
            league: "Test League".into(),
            home_team: "Team A".into(),
            away_team: "Team B".into(),
            start_time: None,
            is_live: false,
            profit_percent: 3.0,
            total_stake: 1_000.0,
            legs: vec![
                SurebetLeg {
                    bookmaker: "pari".into(),
                    market: "1X2".into(),
                    selection: "1".into(),
                    odds: 2.05,
                    line: None,
                    stake: 500.0,
                    payout: 1_025.0,
                    url: None,
                },
                SurebetLeg {
                    bookmaker: "fonbet".into(),
                    market: "1X2".into(),
                    selection: "2".into(),
                    odds: 2.05,
                    line: None,
                    stake: 500.0,
                    payout: 1_025.0,
                    url: None,
                },
            ],
            detected_at: Utc::now(),
            verified: false,
            mirror: false,
        }
    }

    #[test]
    fn test_engine_start_stop() {
        let engine = AutoBetEngine::new(AutoBetConfig::default());

        engine.start();
        let status = engine.get_status();
        assert!(status.running);
        assert!(!status.emergency_stopped);

        engine.stop();
        let status = engine.get_status();
        assert!(!status.running);
    }

    #[test]
    fn test_engine_emergency_stop() {
        let engine = AutoBetEngine::new(AutoBetConfig::default());
        engine.start();

        engine.emergency_stop();
        let status = engine.get_status();
        assert!(!status.running);
        assert!(status.emergency_stopped);
    }

    #[test]
    fn test_place_surebet_when_stopped() {
        let engine = AutoBetEngine::new(AutoBetConfig::default());
        let surebet = make_test_surebet();
        let result = futures::executor::block_on(engine.place_surebet(&surebet));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_status_initial() {
        let engine = AutoBetEngine::new(AutoBetConfig::default());
        let status = engine.get_status();
        assert!(!status.running);
        assert_eq!(status.bets_placed_today, 0);
        assert_eq!(status.profit_today, 0.0);
    }

    #[test]
    fn test_engine_exposes_safe_default_execution_registry() {
        let engine = AutoBetEngine::new(AutoBetConfig::default());
        let capability = engine.execution_registry().get_capability("pari");

        assert!(capability.supports_dry_run);
        assert!(capability.supports_bet_placement);
        assert!(!capability.supports_real_money);
    }

    #[tokio::test]
    async fn test_plan_surebet_ranks_ready_leg_ahead_of_pari_rollout_gate() {
        let registry = Arc::new(ExecutionRegistry::new());
        register_ready_bookmaker(
            registry.as_ref(),
            "pari",
            BookmakerExecutionMode::SemiRealReady,
        );
        register_ready_bookmaker(registry.as_ref(), "fonbet", BookmakerExecutionMode::DryRun);
        let engine = AutoBetEngine::with_registry(AutoBetConfig::default(), registry);

        let plan = engine
            .plan_surebet_execution(&make_rollout_surebet())
            .await
            .expect("plan should succeed");

        assert!(!plan.executable);
        assert_eq!(plan.ranked_legs[0].leg.bookmaker, "fonbet");
        assert_eq!(
            plan.ranked_legs[0].decision,
            ApprovalGateDecision::AllowDryRun
        );
        assert_eq!(plan.ranked_legs[1].leg.bookmaker, "pari");
        assert_eq!(
            plan.ranked_legs[1].decision,
            ApprovalGateDecision::RequireOperatorApproval
        );
        assert!(plan.ranked_legs[1].placement_requested);
    }

    #[tokio::test]
    async fn test_place_surebet_fails_before_execution_when_pari_gate_blocks_rollout() {
        let registry = Arc::new(ExecutionRegistry::new());
        register_ready_bookmaker(
            registry.as_ref(),
            "pari",
            BookmakerExecutionMode::SemiRealReady,
        );
        register_ready_bookmaker(registry.as_ref(), "fonbet", BookmakerExecutionMode::DryRun);
        let engine = AutoBetEngine::with_registry(AutoBetConfig::default(), registry.clone());
        engine.start();

        let error = engine
            .place_surebet(&make_rollout_surebet())
            .await
            .expect_err("rollout gate should block execution");

        assert!(error.contains("pari rollout gate"));
        assert!(engine.get_history(10).is_empty());
        assert!(registry
            .get_account("fonbet")
            .expect("fonbet account should exist")
            .last_used_at
            .is_none());
    }

    #[tokio::test]
    async fn test_engine_records_ledger_entries_for_place_and_result() {
        let ledger = Arc::new(TestLedger::default());
        let engine = AutoBetEngine::with_registry_and_ledger(
            AutoBetConfig::default(),
            Arc::new(ExecutionRegistry::new()),
            ledger.clone(),
        );
        engine.start();

        let placement = engine
            .place_bet("pari", "event-1", "1X2", "1", 2.1, 500.0)
            .await
            .unwrap();
        engine.record_result(placement.id, BetResult::Won(1050.0));

        let entries = ledger.entries.lock().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].action, ExecutionLedgerAction::Placed);
        assert_eq!(entries[1].action, ExecutionLedgerAction::Updated);
        assert_eq!(entries[1].placement.status, BetStatus::Settled);
    }

    #[tokio::test]
    async fn test_engine_records_execution_state_transitions_when_enabled() {
        let ledger = Arc::new(TestLedger::default());
        let state = Arc::new(TestStatePersistence::default());
        let engine = AutoBetEngine::with_registry_ledger_and_state(
            AutoBetConfig::default(),
            Arc::new(ExecutionRegistry::new()),
            ledger,
            state.clone(),
        );
        engine.start();

        let placement = engine
            .place_bet("pari", "event-1", "1X2", "1", 2.1, 500.0)
            .await
            .unwrap();
        engine.record_result(placement.id, BetResult::Won(1050.0));
        tokio::task::yield_now().await;

        let snapshots = state.snapshots.lock().await;
        let transitions = state.transitions.lock().await;
        assert_eq!(snapshots.len(), 2);
        assert_eq!(transitions.len(), 2);
        assert_eq!(
            transitions[0].to_phase,
            ExecutionStatePhase::PendingPlacement
        );
        assert_eq!(transitions[1].to_phase, ExecutionStatePhase::Settled);
        assert_eq!(snapshots[1].sequence, 2);
    }
}
