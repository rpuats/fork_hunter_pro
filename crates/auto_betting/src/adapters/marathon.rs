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

use crate::auth::BookmakerSessionMaterial;
use crate::execution::BookmakerExecutionAdapter;

#[derive(Debug, Clone, Default)]
pub struct MarathonExecutionAdapter;

impl MarathonExecutionAdapter {
    pub const BOOKMAKER: &'static str = "marathon";
    const API_BASE_URL: &'static str = "https://api.marathon.mock";

    fn dry_run_message(request: &BetExecutionRequest) -> String {
        format!(
            "marathon execution stub: dry-run only for {} {} on event {}",
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
        session_material: Option<&BookmakerSessionMaterial>,
    ) -> BookmakerSessionStatus {
        let checked_at = Utc::now();

        match session {
            None => BookmakerSessionStatus {
                account_id: Some(account.id),
                bookmaker: Self::BOOKMAKER.into(),
                sync_state: BookmakerSessionSyncState::NoSession,
                authenticated: false,
                can_refresh_balance: false,
                detail: Some("marathon session is not configured".into()),
                checked_at,
            },
            Some(session) => {
                let has_imported_material = session_material
                    .map(BookmakerSessionMaterial::has_credentials)
                    .unwrap_or(false);
                let (sync_state, authenticated, can_refresh_balance, detail) = match session.state {
                    BookmakerSessionState::Configured => (
                        BookmakerSessionSyncState::Configured,
                        false,
                        false,
                        "marathon session configured but not authenticated",
                    ),
                    BookmakerSessionState::Active => (
                        BookmakerSessionSyncState::Authenticated,
                        true,
                        true,
                        if has_imported_material {
                            "marathon imported browser session material is available; remote balance fetch disabled in safe mode"
                        } else {
                            "marathon session authenticated; remote balance fetch enabled"
                        },
                    ),
                    BookmakerSessionState::Expired => (
                        BookmakerSessionSyncState::Expired,
                        false,
                        false,
                        "marathon session expired",
                    ),
                    BookmakerSessionState::Locked => (
                        BookmakerSessionSyncState::Locked,
                        false,
                        false,
                        "marathon session locked",
                    ),
                    BookmakerSessionState::Disconnected => (
                        BookmakerSessionSyncState::Disconnected,
                        false,
                        false,
                        "marathon session disconnected",
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
impl BookmakerExecutionAdapter for MarathonExecutionAdapter {
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
                        "session/bootstrap remains operator-managed until auth flow is audited".into(),
                        "semi-real orchestration is allowed for rollout checks without remote submit".into(),
                    ],
                },
                notes: vec![
                    "safe-mode adapter exposes cached account state only".into(),
                    "semi-real submission path is enabled for arming and orchestration tests".into(),
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
        session_material: Option<&BookmakerSessionMaterial>,
    ) -> Result<BookmakerSessionStatus, String> {
        Ok(self.session_status(account, session, session_material))
    }

    async fn refresh_balance_snapshot(
        &self,
        account: &BookmakerAccount,
        session_status: &BookmakerSessionStatus,
        cached_snapshot: Option<&BookmakerBalanceSnapshot>,
        session_material: Option<&BookmakerSessionMaterial>,
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

        let has_imported_material = session_material
            .map(BookmakerSessionMaterial::has_credentials)
            .unwrap_or(false);
        let detail = match state {
            BookmakerBalanceRefreshState::NoSession => {
                "marathon balance refresh skipped: no session configured"
            }
            BookmakerBalanceRefreshState::SessionNotAuthenticated => {
                "marathon balance refresh skipped: session is not authenticated"
            }
            BookmakerBalanceRefreshState::AuthenticatedBalanceUnavailable => {
                if has_imported_material {
                    "marathon real session material imported; remote balance endpoint is not wired yet"
                } else {
                    "marathon session is authenticated but remote balance fetch is disabled in safe mode"
                }
            }
            BookmakerBalanceRefreshState::CachedBalanceAvailable => {
                if has_imported_material {
                    "marathon returned cached balance snapshot for imported browser session"
                } else {
                    "marathon returned cached balance snapshot; remote refresh remains disabled"
                }
            }
            BookmakerBalanceRefreshState::RemoteBalanceFetched => {
                "marathon balance fetched from live API"
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
                "marathon semi-real submission path reached; remote coupon placement remains disabled".into(),
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
            bookmaker: MarathonExecutionAdapter::BOOKMAKER.into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        }
    }

    #[test]
    fn authorize_returns_active_session_stub() {
        let adapter = MarathonExecutionAdapter;
        let account = account();
        futures::executor::block_on(async {
            let session = adapter
                .authorize(&account)
                .await
                .expect("should create mock session");
            assert_eq!(
                session.bookmaker,
                MarathonExecutionAdapter::BOOKMAKER.to_string()
            );
            // basic sanity: state should be Active
            assert_eq!(session.state, BookmakerSessionState::Active);
        });
    }
}

use crate::auth::BookmakerAuth;
use std::error::Error;

#[async_trait]
impl BookmakerAuth for MarathonExecutionAdapter {
    async fn authorize(
        &self,
        account: &BookmakerAccount,
    ) -> Result<BookmakerSession, Box<dyn Error + Send + Sync>> {
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
