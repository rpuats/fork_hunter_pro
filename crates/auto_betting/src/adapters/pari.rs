use async_trait::async_trait;
use chrono::Utc;
use shared::{
    BetExecutionReceipt, BetExecutionRequest, BetExecutionStatus, BookmakerAccount,
    BookmakerAccountCapabilityMetadata, BookmakerAdapterAuthMetadata,
    BookmakerAdapterReadinessMetadata, BookmakerAdapterReadinessStage, BookmakerBalanceRefresh,
    BookmakerBalanceRefreshState, BookmakerBalanceSnapshot, BookmakerExecutionCapability,
    BookmakerExecutionMode, BookmakerSession, BookmakerSessionState, BookmakerSessionStatus,
    BookmakerSessionSyncState,
};

use crate::execution::BookmakerExecutionAdapter;

#[derive(Debug, Clone, Default)]
pub struct PariExecutionAdapter;

impl PariExecutionAdapter {
    pub const BOOKMAKER: &'static str = "pari";
    const API_BASE_URL: &'static str = "https://api.pari.ru";

    fn dry_run_message(request: &BetExecutionRequest) -> String {
        format!(
            "pari execution stub: dry-run only for {} {} on event {}",
            request.market, request.selection, request.event_id
        )
    }

    pub fn api_base_url(&self) -> &'static str {
        Self::API_BASE_URL
    }

    pub fn planned_endpoints(&self) -> [&'static str; 3] {
        ["session", "balance", "coupon/place"]
    }

    fn session_status(
        &self,
        account: &BookmakerAccount,
        session: Option<&BookmakerSession>,
    ) -> BookmakerSessionStatus {
        let checked_at = Utc::now();

        match session {
            None => BookmakerSessionStatus {
                account_id: Some(account.id),
                bookmaker: Self::BOOKMAKER.into(),
                sync_state: BookmakerSessionSyncState::NoSession,
                authenticated: false,
                can_refresh_balance: false,
                detail: Some("pari session is not configured".into()),
                checked_at,
            },
            Some(session) => {
                let (sync_state, authenticated, can_refresh_balance, detail) = match session.state {
                    BookmakerSessionState::Configured => (
                        BookmakerSessionSyncState::Configured,
                        false,
                        false,
                        "pari session configured but not authenticated",
                    ),
                    BookmakerSessionState::Active => (
                        BookmakerSessionSyncState::Authenticated,
                        true,
                        true,
                        "pari session authenticated; remote balance fetch disabled in safe mode",
                    ),
                    BookmakerSessionState::Expired => (
                        BookmakerSessionSyncState::Expired,
                        false,
                        false,
                        "pari session expired",
                    ),
                    BookmakerSessionState::Locked => (
                        BookmakerSessionSyncState::Locked,
                        false,
                        false,
                        "pari session locked",
                    ),
                    BookmakerSessionState::Disconnected => (
                        BookmakerSessionSyncState::Disconnected,
                        false,
                        false,
                        "pari session disconnected",
                    ),
                };

                BookmakerSessionStatus {
                    account_id: Some(account.id),
                    bookmaker: Self::BOOKMAKER.into(),
                    sync_state,
                    authenticated,
                    can_refresh_balance,
                    detail: Some(detail.into()),
                    checked_at,
                }
            }
        }
    }
}
// Implement the auth trait from auto_betting::auth
use crate::auth::BookmakerAuth;
use crate::auth; // for trait path
use crate::execution; // ensure module path availability
impl auth::BookmakerAuth for PariExecutionAdapter {
    async fn authorize(&self, account: &BookmakerAccount) -> Result<BookmakerSession, Box<dyn std::error::Error + Send + Sync>> {
        let session = BookmakerSession {
            account_id: account.id,
            bookmaker: Self::BOOKMAKER.to_string(),
            state: BookmakerSessionState::Active,
            token_hint: Some(format!("mock_token_{}", Self::BOOKMAKER)),
            last_synced_at: Utc::now(),
            expires_at: None,
        };
        Ok(session)
    }
}

#[async_trait]
impl BookmakerExecutionAdapter for PariExecutionAdapter {
    fn capability(&self) -> BookmakerExecutionCapability {
        BookmakerExecutionCapability {
            bookmaker: Self::BOOKMAKER.into(),
            supports_dry_run: true,
            supports_balance_snapshot: true,
            supports_bet_placement: true,
            supports_real_money: false,
            requires_session: true,
            account_metadata: BookmakerAccountCapabilityMetadata {
                api_base_url: Some(self.api_base_url().into()),
                planned_endpoints: self
                    .planned_endpoints()
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                supports_read_only_session_sync: true,
                supports_read_only_balance_refresh: true,
                remote_balance_fetch_enabled: false,
                auth: BookmakerAdapterAuthMetadata {
                    flow: "manual_cookie_session".into(),
                    requires_human_bootstrap: true,
                    session_bootstrap_enabled: false,
                    session_refresh_enabled: false,
                    persisted_snapshot_enabled: true,
                },
                readiness: BookmakerAdapterReadinessMetadata {
                    stage: BookmakerAdapterReadinessStage::SafeModePlacementReady,
                    safe_mode_only: true,
                    approval_reference_required: true,
                    operator_notes: vec![
                        "session/bootstrap remains operator-managed until auth flow is audited"
                            .into(),
                        "semi-real orchestration is allowed for rollout checks without remote submit"
                            .into(),
                    ],
                },
                notes: vec![
                    "safe-mode adapter exposes cached account state only".into(),
                    "semi-real submission path is enabled for arming and orchestration tests"
                        .into(),
                    "real session bootstrap and balance HTTP calls remain disabled".into(),
                ],
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
            bookmaker: Self::BOOKMAKER.into(),
            status: BetExecutionStatus::DryRun,
            mode: BookmakerExecutionMode::DryRun,
            accepted_stake: request.stake,
            accepted_odds: request.odds,
            message: Some(Self::dry_run_message(request)),
            placed_at: Utc::now(),
        })
    }

    async fn get_session_status(
        &self,
        account: &BookmakerAccount,
        session: Option<&BookmakerSession>,
    ) -> Result<BookmakerSessionStatus, String> {
        Ok(self.session_status(account, session))
    }

    async fn refresh_balance_snapshot(
        &self,
        account: &BookmakerAccount,
        session_status: &BookmakerSessionStatus,
        cached_snapshot: Option<&BookmakerBalanceSnapshot>,
    ) -> Result<BookmakerBalanceRefresh, String> {
        let checked_at = Utc::now();
        let snapshot = cached_snapshot.cloned();
        let state = match (&session_status.sync_state, snapshot.is_some()) {
            (BookmakerSessionSyncState::NoSession, _) => BookmakerBalanceRefreshState::NoSession,
            (BookmakerSessionSyncState::Configured, _) => {
                BookmakerBalanceRefreshState::SessionNotAuthenticated
            }
            (BookmakerSessionSyncState::Expired, _) => {
                BookmakerBalanceRefreshState::SessionNotAuthenticated
            }
            (BookmakerSessionSyncState::Locked, _) => {
                BookmakerBalanceRefreshState::SessionNotAuthenticated
            }
            (BookmakerSessionSyncState::Disconnected, _) => {
                BookmakerBalanceRefreshState::SessionNotAuthenticated
            }
            (BookmakerSessionSyncState::Authenticated, true) => {
                BookmakerBalanceRefreshState::CachedBalanceAvailable
            }
            (BookmakerSessionSyncState::Authenticated, false) => {
                BookmakerBalanceRefreshState::AuthenticatedBalanceUnavailable
            }
        };

        let detail = match state {
            BookmakerBalanceRefreshState::NoSession => {
                "pari balance refresh skipped: no session configured"
            }
            BookmakerBalanceRefreshState::SessionNotAuthenticated => {
                "pari balance refresh skipped: session is not authenticated"
            }
            BookmakerBalanceRefreshState::AuthenticatedBalanceUnavailable => {
                "pari session is authenticated but remote balance fetch is disabled in safe mode"
            }
            BookmakerBalanceRefreshState::CachedBalanceAvailable => {
                "pari returned cached balance snapshot; remote refresh remains disabled"
            }
        };

        Ok(BookmakerBalanceRefresh {
            account_id: Some(account.id),
            bookmaker: Self::BOOKMAKER.into(),
            state,
            session_status: session_status.clone(),
            snapshot,
            detail: Some(detail.into()),
            checked_at,
        })
    }

    async fn place_bet(
        &self,
        account: &BookmakerAccount,
        request: &BetExecutionRequest,
    ) -> Result<BetExecutionReceipt, String> {
        Ok(BetExecutionReceipt {
            ticket_id: None,
            account_id: Some(account.id),
            bookmaker: Self::BOOKMAKER.into(),
            status: BetExecutionStatus::Submitted,
            mode: account.mode.clone(),
            accepted_stake: request.stake,
            accepted_odds: request.odds,
            message: Some(
                "pari semi-real submission path reached; remote coupon placement remains disabled"
                    .into(),
            ),
            placed_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::BookmakerExecutionMode;
    use uuid::Uuid;

    fn account() -> BookmakerAccount {
        BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: PariExecutionAdapter::BOOKMAKER.into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        }
    }

    #[test]
    fn reports_configured_session_as_not_authenticated() {
        let adapter = PariExecutionAdapter;
        let account = account();
        let session = BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Configured,
            token_hint: Some("cfg...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        };

        let status =
            futures::executor::block_on(adapter.get_session_status(&account, Some(&session)))
                .expect("status should succeed");

        assert_eq!(status.sync_state, BookmakerSessionSyncState::Configured);
        assert!(!status.authenticated);
    }
}
