use chrono::Utc;
use shared::{
    AutoBetConfig, AutoBetStatus, BetExecutionRequest, BetExecutionStatus, BetPlacement, BetResult,
    BetStatus, Event, StakeValidationDecision, StakeValidationRequest, Surebet,
};
use std::sync::Arc;
use tracing::{error, info, warn};

use super::limiter::{BetLimiter, BetLimiterStats};
use super::registry::ExecutionRegistry;
use super::stealth::StealthBetting;
use super::validator::StakeValidator;

#[derive(Clone)]
pub struct AutoBetEngine {
    config: Arc<parking_lot::RwLock<AutoBetConfig>>,
    limiter: Arc<parking_lot::Mutex<BetLimiter>>,
    stealth: Arc<StealthBetting>,
    registry: Arc<ExecutionRegistry>,
    history: Arc<parking_lot::Mutex<Vec<BetPlacement>>>,
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
            running: Arc::new(parking_lot::RwLock::new(false)),
            emergency_stopped: Arc::new(parking_lot::RwLock::new(false)),
            total_profit: Arc::new(parking_lot::RwLock::new(0.0)),
            today_profit: Arc::new(parking_lot::RwLock::new(0.0)),
            bets_today: Arc::new(parking_lot::RwLock::new(0)),
            bets_total: Arc::new(parking_lot::RwLock::new(0)),
            errors_today: Arc::new(parking_lot::RwLock::new(0)),
        }
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

        let mut placements = Vec::new();

        for leg in &surebet.legs {
            self.stealth.wait_stealth().await;

            let bookmaker_balance = self
                .registry
                .refresh_balance_snapshot(&leg.bookmaker)
                .await
                .map(|refresh| refresh.snapshot)
                .unwrap_or_else(|error| {
                    warn!(bookmaker = leg.bookmaker.as_str(), error = %error, "Balance refresh failed");
                    self.registry.get_balance_snapshot(&leg.bookmaker)
                });

            let validation = StakeValidator::validate(&StakeValidationRequest {
                bookmaker: leg.bookmaker.clone(),
                desired_stake: leg.stake,
                min_stake: None,
                max_stake: Some(self.config.read().max_stake_per_bet),
                bookmaker_available_balance: bookmaker_balance
                    .as_ref()
                    .map(|snapshot| snapshot.available_balance),
                bankroll_available_balance: None,
                allow_auto_adjust: true,
            });
            if matches!(validation.decision, StakeValidationDecision::Reject) {
                return Err(validation.reasons.join("; "));
            }

            let stake = validation.adjusted_stake;

            let mut limiter = self.limiter.lock();
            if let Err(e) = limiter.can_bet(stake) {
                warn!(error = e.to_string(), "Bet limit reached");
                return Err(e.to_string());
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
                allow_dry_run: true,
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

            limiter.record_bet(stake);
            placements.push(placement);
        }

        for p in &placements {
            self.history.lock().push(p.clone());
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

        let mut limiter = self.limiter.lock();
        if let Err(e) = limiter.can_bet(stake) {
            warn!(error = e.to_string(), "Bet limit reached");
            return Err(e.to_string());
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
            BetExecutionStatus::Blocked | BetExecutionStatus::Rejected => {
                execution.message.clone()
            }
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

        limiter.record_bet(adjusted_stake);
        self.history.lock().push(placement.clone());
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
    use shared::{AutoBetConfig, Surebet, SurebetLeg};

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
}
