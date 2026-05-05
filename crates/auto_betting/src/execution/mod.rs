//! Execution module - State management and orchestration

pub mod execution_state;
pub mod scanner_bridge;

use async_trait::async_trait;
use crate::auth::BookmakerSessionMaterial;

// Use BookmakerExecutionCapability from shared
pub use shared::{
    BookmakerExecutionCapability, BookmakerAccount, BookmakerSession, BookmakerSessionStatus,
    BookmakerBalanceSnapshot, BookmakerBalanceRefresh, BetExecutionRequest, BetExecutionReceipt,
};

/// Trait for bookmaker execution adapter
#[async_trait]
pub trait BookmakerExecutionAdapter: Send + Sync {
    fn capability(&self) -> BookmakerExecutionCapability;
    async fn place_bet(&self, account: &BookmakerAccount, request: &BetExecutionRequest) -> Result<BetExecutionReceipt, String>;
    async fn dry_run(&self, account: Option<&BookmakerAccount>, request: &BetExecutionRequest) -> Result<BetExecutionReceipt, String>;
    async fn get_session_status(&self, account: &BookmakerAccount, session: Option<&BookmakerSession>, session_material: Option<&BookmakerSessionMaterial>) -> Result<BookmakerSessionStatus, String>;
    async fn refresh_balance_snapshot(&self, account: &BookmakerAccount, session_status: &BookmakerSessionStatus, cached_snapshot: Option<&BookmakerBalanceSnapshot>, session_material: Option<&BookmakerSessionMaterial>) -> Result<BookmakerBalanceRefresh, String>;
}

/// Noop execution adapter for testing
#[derive(Debug, Clone)]
pub struct NoopExecutionAdapter;

impl NoopExecutionAdapter {
    /// Create new noop adapter
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopExecutionAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BookmakerExecutionAdapter for NoopExecutionAdapter {
    fn capability(&self) -> BookmakerExecutionCapability {
        BookmakerExecutionCapability {
            bookmaker: "noop".to_string(),
            supports_dry_run: true,
            supports_balance_snapshot: true,
            supports_bet_placement: true,
            supports_real_money: false,
            requires_session: false,
            account_metadata: shared::BookmakerAccountCapabilityMetadata {
                api_base_url: None,
                planned_endpoints: vec![],
                supports_read_only_session_sync: false,
                supports_read_only_balance_refresh: false,
                remote_balance_fetch_enabled: false,
                auth: shared::BookmakerAdapterAuthMetadata {
                    flow: "noop".to_string(),
                    requires_human_bootstrap: false,
                    session_bootstrap_enabled: false,
                    session_refresh_enabled: false,
                    persisted_snapshot_enabled: false,
                },
                readiness: shared::BookmakerAdapterReadinessMetadata {
                    stage: shared::BookmakerAdapterReadinessStage::Stub,
                    safe_mode_only: true,
                    approval_reference_required: false,
                    operator_notes: vec![],
                },
                notes: vec![],
            },
        }
    }

    async fn place_bet(&self, _account: &BookmakerAccount, _request: &BetExecutionRequest) -> Result<BetExecutionReceipt, String> {
        Err("Not implemented".to_string())
    }

    async fn dry_run(&self, _account: Option<&BookmakerAccount>, _request: &BetExecutionRequest) -> Result<BetExecutionReceipt, String> {
        Err("Not implemented".to_string())
    }

    async fn get_session_status(&self, _account: &BookmakerAccount, _session: Option<&BookmakerSession>, _session_material: Option<&BookmakerSessionMaterial>) -> Result<BookmakerSessionStatus, String> {
        Err("Not implemented".to_string())
    }

    async fn refresh_balance_snapshot(&self, _account: &BookmakerAccount, _session_status: &BookmakerSessionStatus, _cached_snapshot: Option<&BookmakerBalanceSnapshot>, _session_material: Option<&BookmakerSessionMaterial>) -> Result<BookmakerBalanceRefresh, String> {
        Err("Not implemented".to_string())
    }
}

pub use execution_state::{
    ExecutionState, ForkExecution, ForkStatus, Fork, ForkLeg,
    AccountReadiness, BankrollPlan, StakeAllocation, StakingStrategy,
    DailyStats, GlobalLimits, ExecutionOrchestrator,
};
pub use scanner_bridge::{ScannerBridge, spawn_scanner_bridge};
