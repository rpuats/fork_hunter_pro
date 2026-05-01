//! Betting Runner - Main betting execution loop

use crate::{
    auth::AuthManager,
    betting::{BetMode, BettingEngine, OperatorQueue, OperatorEvent, OperatorResponse},
    execution::{ExecutionOrchestrator, ForkStatus},
    BrowserPool,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

/// Betting runner state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunnerState {
    Idle,
    Running,
    Paused,
    Stopping,
    Stopped,
}

/// Betting runner configuration
#[derive(Debug, Clone)]
pub struct BettingRunnerConfig {
    pub mode: BetMode,
    pub check_interval_ms: u64,
    pub max_concurrent_forks: usize,
    pub auto_retry_failures: bool,
}

impl Default for BettingRunnerConfig {
    fn default() -> Self {
        Self {
            mode: BetMode::SemiAuto,
            check_interval_ms: 100,
            max_concurrent_forks: 5,
            auto_retry_failures: true,
        }
    }
}

/// Main betting runner
pub struct BettingRunner {
    orchestrator: Arc<TokioMutex<ExecutionOrchestrator>>,
    operator_queue: Arc<TokioMutex<OperatorQueue>>,
    auth_manager: Arc<TokioMutex<AuthManager>>,
    browser_pool: Arc<BrowserPool>,
    config: BettingRunnerConfig,
    state: RunnerState,
}

impl BettingRunner {
    pub fn new(
        orchestrator: Arc<TokioMutex<ExecutionOrchestrator>>,
        operator_queue: Arc<TokioMutex<OperatorQueue>>,
        auth_manager: Arc<TokioMutex<AuthManager>>,
        browser_pool: Arc<BrowserPool>,
        config: BettingRunnerConfig,
    ) -> Self {
        Self {
            orchestrator,
            operator_queue,
            auth_manager,
            browser_pool,
            config,
            state: RunnerState::Idle,
        }
    }

    /// Start the runner
    pub async fn start(&mut self) {
        if self.state == RunnerState::Running {
            warn!("Betting runner already running");
            return;
        }

        self.state = RunnerState::Running;
        info!("Betting runner started in {:?} mode", self.config.mode);

        let mut interval = interval(Duration::from_millis(self.config.check_interval_ms));

        while self.state == RunnerState::Running || self.state == RunnerState::Paused {
            interval.tick().await;

            if self.state == RunnerState::Paused {
                continue;
            }

            // Check for limits
            {
                let orchestrator = self.orchestrator.lock().await;
                if orchestrator.get_state().should_stop_due_to_limits() {
                    warn!("Limits reached, stopping runner");
                    self.state = RunnerState::Stopping;
                    break;
                }
            }

            // Process operator queue
            self.process_operator_queue().await;

            // Check active forks
            self.check_active_forks().await;

            // Update account readiness
            self.update_account_readiness().await;
        }

        self.state = RunnerState::Stopped;
        info!("Betting runner stopped");
    }

    /// Pause the runner
    pub fn pause(&mut self) {
        if self.state == RunnerState::Running {
            self.state = RunnerState::Paused;
            info!("Betting runner paused");
        }
    }

    /// Resume the runner
    pub fn resume(&mut self) {
        if self.state == RunnerState::Paused {
            self.state = RunnerState::Running;
            info!("Betting runner resumed");
        }
    }

    /// Stop the runner
    pub fn stop(&mut self) {
        self.state = RunnerState::Stopping;
        info!("Betting runner stopping...");
    }

    /// Get current state
    pub fn state(&self) -> RunnerState {
        self.state
    }

    /// Process operator queue items
    async fn process_operator_queue(&self) {
        let mut queue = self.operator_queue.lock().await;

        // Get current item if any
        if let Some(item) = queue.current() {
            // Check if expired
            if item.is_expired() {
                queue.resolve_current(true);
                info!("Queue item expired: {}", item.id());
            }
            return;
        }

        // Get next item
        if let Some(item) = queue.next() {
            info!("Processing queue item: {:?}", item);
            // Item is now set as current, waiting for operator response
        }

        drop(queue);
    }

    /// Check active forks for timeouts and status updates
    async fn check_active_forks(&self) {
        let orchestrator = self.orchestrator.lock().await;
        let state = orchestrator.get_state();
        let now = chrono::Utc::now();

        for (fork_id, execution) in &state.active_forks {
            // Check timeout
            if now > execution.timeout_at {
                if !execution.status.is_terminal() {
                    warn!("Fork {} timed out", fork_id);
                    // Timeout handling would go here
                }
            }

            // Check if partially executed forks need attention
            if execution.status == ForkStatus::PartiallyExecuted {
                // Handle partial execution
            }
        }
    }

    /// Update account readiness status
    async fn update_account_readiness(&self) {
        let auth_manager = self.auth_manager.lock().await;
        let mut orchestrator = self.orchestrator.lock().await;

        // Get list of bookmakers from orchestrator state
        let bookmakers: Vec<String> = orchestrator.get_state()
            .account_readiness
            .keys()
            .cloned()
            .collect();

        for bookmaker in bookmakers {
            if let Some(creds) = auth_manager.get_credentials(&bookmaker) {
                let readiness = crate::execution::AccountReadiness {
                    bookmaker: bookmaker.clone(),
                    authenticated: creds.status == crate::auth::AuthStatus::Authenticated,
                    balance: creds.balance.unwrap_or(rust_decimal::Decimal::ZERO),
                    session_valid: creds.cookies.is_some(),
                    last_check: chrono::Utc::now(),
                    can_place_bets: creds.balance.unwrap_or(rust_decimal::Decimal::ZERO) > rust_decimal::Decimal::ZERO,
                };

                orchestrator.get_state_mut().update_account_readiness(bookmaker, readiness);
            }
        }
    }

    /// Set execution mode
    pub async fn set_mode(&self, mode: BetMode) {
        let mut orchestrator = self.orchestrator.lock().await;
        orchestrator.set_mode(mode);
        info!("Execution mode changed to: {:?}", mode);
    }

    /// Get current mode
    pub async fn get_mode(&self) -> BetMode {
        let orchestrator = self.orchestrator.lock().await;
        orchestrator.get_mode()
    }
}

/// Spawn betting runner task
pub fn spawn_betting_runner(
    orchestrator: Arc<TokioMutex<ExecutionOrchestrator>>,
    operator_queue: Arc<TokioMutex<OperatorQueue>>,
    auth_manager: Arc<TokioMutex<AuthManager>>,
    browser_pool: Arc<BrowserPool>,
    config: BettingRunnerConfig,
) -> (BettingRunnerHandle, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    let mut runner = BettingRunner::new(
        orchestrator,
        operator_queue,
        auth_manager,
        browser_pool,
        config,
    );

    let handle = tokio::spawn(async move {
        // Listen for control commands
        let control_task = tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    RunnerCommand::Start => {}
                    RunnerCommand::Pause => runner.pause(),
                    RunnerCommand::Resume => runner.resume(),
                    RunnerCommand::Stop => runner.stop(),
                    RunnerCommand::SetMode(mode) => {
                        runner.set_mode(mode).await;
                    }
                }
            }
        });

        // Run main loop
        runner.start().await;

        // Stop control task
        control_task.abort();
    });

    (BettingRunnerHandle { tx }, handle)
}

/// Control handle for betting runner
pub struct BettingRunnerHandle {
    tx: tokio::sync::mpsc::Sender<RunnerCommand>,
}

impl BettingRunnerHandle {
    pub async fn start(&self) {
        let _ = self.tx.send(RunnerCommand::Start).await;
    }

    pub async fn pause(&self) {
        let _ = self.tx.send(RunnerCommand::Pause).await;
    }

    pub async fn resume(&self) {
        let _ = self.tx.send(RunnerCommand::Resume).await;
    }

    pub async fn stop(&self) {
        let _ = self.tx.send(RunnerCommand::Stop).await;
    }

    pub async fn set_mode(&self, mode: BetMode) {
        let _ = self.tx.send(RunnerCommand::SetMode(mode)).await;
    }
}

/// Runner commands
enum RunnerCommand {
    Start,
    Pause,
    Resume,
    Stop,
    SetMode(BetMode),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_state() {
        assert!(RunnerState::Running != RunnerState::Stopped);
        assert!(RunnerState::Idle != RunnerState::Running);
    }

    #[test]
    fn test_runner_config_default() {
        let config = BettingRunnerConfig::default();
        assert_eq!(config.mode, BetMode::SemiAuto);
        assert_eq!(config.check_interval_ms, 100);
        assert_eq!(config.max_concurrent_forks, 5);
    }
}
