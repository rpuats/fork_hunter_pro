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
pub struct FonbetExecutionAdapter;

impl FonbetExecutionAdapter {
    pub const BOOKMAKER: &'static str = "fonbet";
    const API_BASE_URL: &'static str = "https://clientsapi24.fonbet.ru";

    fn session_expired(session: &BookmakerSession, checked_at: chrono::DateTime<Utc>) -> bool {
        matches!(session.state, BookmakerSessionState::Active)
            && session
                .expires_at
                .map(|expires_at| expires_at <= checked_at)
                .unwrap_or(false)
    }

    fn dry_run_message(request: &BetExecutionRequest) -> String {
        format!(
            "fonbet execution stub: dry-run only for {} {} on event {}",
            request.market, request.selection, request.event_id
        )
    }

    pub fn api_base_url(&self) -> &'static str {
        Self::API_BASE_URL
    }

    pub fn planned_endpoints(&self) -> [&'static str; 3] {
        ["loginById", "coupon/getMinMax", "coupon/register"]
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
                detail: Some("fonbet session is not configured".into()),
                checked_at,
            },
            Some(session) => {
                let (sync_state, authenticated, can_refresh_balance, detail) = match session.state {
                    BookmakerSessionState::Configured => (
                        BookmakerSessionSyncState::Configured,
                        false,
                        false,
                        "fonbet session configured but not authenticated",
                    ),
                    BookmakerSessionState::Active if Self::session_expired(session, checked_at) => {
                        (
                            BookmakerSessionSyncState::Expired,
                            false,
                            false,
                            "fonbet session expired according to local expiry timestamp",
                        )
                    }
                    BookmakerSessionState::Active => (
                        BookmakerSessionSyncState::Authenticated,
                        true,
                        true,
                        "fonbet session authenticated; remote balance fetch disabled in safe mode",
                    ),
                    BookmakerSessionState::Expired => (
                        BookmakerSessionSyncState::Expired,
                        false,
                        false,
                        "fonbet session expired",
                    ),
                    BookmakerSessionState::Locked => (
                        BookmakerSessionSyncState::Locked,
                        false,
                        false,
                        "fonbet session locked",
                    ),
                    BookmakerSessionState::Disconnected => (
                        BookmakerSessionSyncState::Disconnected,
                        false,
                        false,
                        "fonbet session disconnected",
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

#[async_trait]
impl BookmakerExecutionAdapter for FonbetExecutionAdapter {
    fn capability(&self) -> BookmakerExecutionCapability {
        BookmakerExecutionCapability {
            bookmaker: Self::BOOKMAKER.into(),
            supports_dry_run: true,
            supports_balance_snapshot: true,
            supports_bet_placement: false,
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
                    stage: BookmakerAdapterReadinessStage::AuthenticatedReadOnly,
                    safe_mode_only: true,
                    approval_reference_required: false,
                    operator_notes: vec![
                        "session status can be audited without enabling login bootstrap".into(),
                        "cached balance may be reused, but remote refresh remains disabled".into(),
                    ],
                },
                notes: vec![
                    "safe-mode adapter exposes cached account state only".into(),
                    "login and balance polling remain disabled until audited".into(),
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
                "fonbet balance refresh skipped: no session configured"
            }
            BookmakerBalanceRefreshState::SessionNotAuthenticated => {
                "fonbet balance refresh skipped: session is not authenticated"
            }
            BookmakerBalanceRefreshState::AuthenticatedBalanceUnavailable => {
                "fonbet session is authenticated but remote balance fetch is disabled in safe mode"
            }
            BookmakerBalanceRefreshState::CachedBalanceAvailable => {
                "fonbet returned cached balance snapshot; remote refresh remains disabled"
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
        self.dry_run(Some(account), request).await
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
            bookmaker: FonbetExecutionAdapter::BOOKMAKER.into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        }
    }

    #[test]
    fn reports_authenticated_balance_unavailable_without_cache() {
        let adapter = FonbetExecutionAdapter;
        let account = account();
        let status = BookmakerSessionStatus {
            account_id: Some(account.id),
            bookmaker: account.bookmaker.clone(),
            sync_state: BookmakerSessionSyncState::Authenticated,
            authenticated: true,
            can_refresh_balance: true,
            detail: None,
            checked_at: Utc::now(),
        };

        let refresh =
            futures::executor::block_on(adapter.refresh_balance_snapshot(&account, &status, None))
                .expect("refresh should succeed");

        assert_eq!(
            refresh.state,
            BookmakerBalanceRefreshState::AuthenticatedBalanceUnavailable
        );
        assert!(refresh.snapshot.is_none());
    }

    #[test]
    fn treats_expired_active_session_as_not_authenticated() {
        let adapter = FonbetExecutionAdapter;
        let account = account();
        let session = BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: Some(Utc::now() - chrono::Duration::minutes(5)),
        };

        let status = adapter.session_status(&account, Some(&session));

        assert_eq!(status.sync_state, BookmakerSessionSyncState::Expired);
        assert!(!status.authenticated);
        assert!(!status.can_refresh_balance);
        assert!(status
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("local expiry timestamp"));

        let refresh =
            futures::executor::block_on(adapter.refresh_balance_snapshot(&account, &status, None))
                .expect("refresh should succeed");

        assert_eq!(
            refresh.state,
            BookmakerBalanceRefreshState::SessionNotAuthenticated
        );
    }
}
