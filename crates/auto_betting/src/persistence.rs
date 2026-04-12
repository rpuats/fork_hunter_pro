use async_trait::async_trait;
use shared::{BookmakerAccount, BookmakerBalanceSnapshot, BookmakerSession};

#[derive(Debug, Clone, Default)]
pub struct ExecutionRegistrySnapshot {
    pub accounts: Vec<BookmakerAccount>,
    pub sessions: Vec<BookmakerSession>,
    pub balances: Vec<BookmakerBalanceSnapshot>,
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
}
