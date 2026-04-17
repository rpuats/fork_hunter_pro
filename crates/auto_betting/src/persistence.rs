use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::{
    BetPlacement, BookmakerAccount, BookmakerAuthSnapshot, BookmakerBalanceSnapshot,
    BookmakerSession,
};

use crate::state_machine::{ExecutionStateSnapshot, ExecutionStateTransition};

#[derive(Debug, Clone, Default)]
pub struct ExecutionRegistrySnapshot {
    pub accounts: Vec<BookmakerAccount>,
    pub sessions: Vec<BookmakerSession>,
    pub balances: Vec<BookmakerBalanceSnapshot>,
    pub auth_snapshots: Vec<BookmakerAuthSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ExecutionLedgerAction {
    Placed,
    Updated,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionLedgerEntry {
    pub placement: BetPlacement,
    pub action: ExecutionLedgerAction,
    pub recorded_at: DateTime<Utc>,
}

#[async_trait]
pub trait ExecutionRegistryPersistence: Send + Sync {
    async fn load_snapshot(&self) -> Result<ExecutionRegistrySnapshot, String>;

    async fn save_account(&self, account: &BookmakerAccount) -> Result<(), String>;

    async fn save_session(&self, session: &BookmakerSession) -> Result<(), String>;

    async fn save_balance_snapshot(
        &self,
        snapshot: &BookmakerBalanceSnapshot,
    ) -> Result<(), String>;

    async fn save_auth_snapshot(&self, snapshot: &BookmakerAuthSnapshot) -> Result<(), String>;
}

pub trait ExecutionLedgerPersistence: Send + Sync {
    fn record(&self, entry: ExecutionLedgerEntry);
}

#[async_trait]
pub trait ExecutionStatePersistence: Send + Sync {
    async fn load_snapshots(&self) -> Result<Vec<ExecutionStateSnapshot>, String>;

    async fn save_snapshot(&self, snapshot: &ExecutionStateSnapshot) -> Result<(), String>;

    async fn record_transition(&self, transition: &ExecutionStateTransition) -> Result<(), String>;
}
