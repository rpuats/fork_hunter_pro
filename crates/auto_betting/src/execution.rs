use async_trait::async_trait;
use chrono::Utc;
use shared::{
    BetExecutionReceipt, BetExecutionRequest, BetExecutionStatus, BookmakerAccount,
    BookmakerAccountCapabilityMetadata, BookmakerBalanceRefresh, BookmakerBalanceSnapshot,
    BookmakerExecutionCapability, BookmakerExecutionMode, BookmakerSession, BookmakerSessionStatus,
    BookmakerSessionSyncState,
};

#[async_trait]
pub trait BookmakerExecutionAdapter: Send + Sync {
    fn capability(&self) -> BookmakerExecutionCapability;

    async fn dry_run(
        &self,
        account: Option<&BookmakerAccount>,
        request: &BetExecutionRequest,
    ) -> Result<BetExecutionReceipt, String>;

    async fn get_session_status(
        &self,
        account: &BookmakerAccount,
        session: Option<&BookmakerSession>,
    ) -> Result<BookmakerSessionStatus, String>;

    async fn refresh_balance_snapshot(
        &self,
        account: &BookmakerAccount,
        session_status: &BookmakerSessionStatus,
        cached_snapshot: Option<&BookmakerBalanceSnapshot>,
    ) -> Result<BookmakerBalanceRefresh, String>;

    async fn place_bet(
        &self,
        account: &BookmakerAccount,
        request: &BetExecutionRequest,
    ) -> Result<BetExecutionReceipt, String>;
}

#[derive(Debug, Default, Clone)]
pub struct NoopExecutionAdapter {
    bookmaker: String,
}

impl NoopExecutionAdapter {
    pub fn new(bookmaker: impl Into<String>) -> Self {
        Self {
            bookmaker: bookmaker.into(),
        }
    }
}

#[async_trait]
impl BookmakerExecutionAdapter for NoopExecutionAdapter {
    fn capability(&self) -> BookmakerExecutionCapability {
        BookmakerExecutionCapability {
            bookmaker: self.bookmaker.clone(),
            supports_dry_run: true,
            supports_balance_snapshot: false,
            supports_bet_placement: false,
            supports_real_money: false,
            requires_session: false,
            account_metadata: BookmakerAccountCapabilityMetadata {
                api_base_url: None,
                planned_endpoints: Vec::new(),
                supports_read_only_session_sync: true,
                supports_read_only_balance_refresh: true,
                remote_balance_fetch_enabled: false,
                notes: vec!["noop adapter exposes cached state only".into()],
            },
        }
    }

    async fn dry_run(
        &self,
        account: Option<&BookmakerAccount>,
        request: &BetExecutionRequest,
    ) -> Result<BetExecutionReceipt, String> {
        Ok(BetExecutionReceipt {
            ticket_id: None,
            account_id: account.map(|account| account.id),
            bookmaker: request.bookmaker.clone(),
            status: BetExecutionStatus::DryRun,
            mode: if account.is_some() {
                BookmakerExecutionMode::DryRun
            } else {
                BookmakerExecutionMode::NoOp
            },
            accepted_stake: request.stake,
            accepted_odds: request.odds,
            message: Some("no execution adapter registered; logical placement only".into()),
            placed_at: Utc::now(),
        })
    }

    async fn get_session_status(
        &self,
        account: &BookmakerAccount,
        _session: Option<&BookmakerSession>,
    ) -> Result<BookmakerSessionStatus, String> {
        Ok(BookmakerSessionStatus {
            account_id: Some(account.id),
            bookmaker: self.bookmaker.clone(),
            sync_state: BookmakerSessionSyncState::NoSession,
            authenticated: false,
            can_refresh_balance: false,
            detail: Some("no execution adapter registered".into()),
            checked_at: Utc::now(),
        })
    }

    async fn refresh_balance_snapshot(
        &self,
        account: &BookmakerAccount,
        session_status: &BookmakerSessionStatus,
        cached_snapshot: Option<&BookmakerBalanceSnapshot>,
    ) -> Result<BookmakerBalanceRefresh, String> {
        let checked_at = Utc::now();
        let snapshot = cached_snapshot.cloned();
        let state = if snapshot.is_some() {
            shared::BookmakerBalanceRefreshState::CachedBalanceAvailable
        } else {
            shared::BookmakerBalanceRefreshState::NoSession
        };

        Ok(BookmakerBalanceRefresh {
            account_id: Some(account.id),
            bookmaker: self.bookmaker.clone(),
            state,
            session_status: session_status.clone(),
            snapshot,
            detail: Some("noop adapter returns cached balance only".into()),
            checked_at,
        })
    }

    async fn place_bet(
        &self,
        account: &BookmakerAccount,
        request: &BetExecutionRequest,
    ) -> Result<BetExecutionReceipt, String> {
        self.dry_run(Some(account), request).await
    }
}
