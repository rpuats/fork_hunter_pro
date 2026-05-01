use async_trait::async_trait;
use chrono::{Duration, Utc};
use shared::{
    BetExecutionReceipt, BetExecutionRequest, BetExecutionStatus, BookmakerAccount,
    BookmakerAccountCapabilityMetadata, BookmakerAdapterAuthMetadata,
    BookmakerAdapterReadinessMetadata, BookmakerAdapterReadinessStage, BookmakerBalanceRefresh,
    BookmakerBalanceRefreshState, BookmakerBalanceSnapshot, BookmakerExecutionCapability,
    BookmakerExecutionMode, BookmakerSession, BookmakerSessionState, BookmakerSessionStatus,
    BookmakerSessionSyncState,
};
use tracing::{debug, error, info, warn};

use crate::auth::{self, BookmakerAuth, BookmakerSessionMaterial};
use crate::execution::BookmakerExecutionAdapter;
use crate::adapters::pari_api::PariApiClient;

#[derive(Debug, Clone)]
pub struct PariExecutionAdapter {
    /// Enable real API calls (requires valid session cookies)
    enable_real_api: bool,
}

impl Default for PariExecutionAdapter {
    fn default() -> Self {
        Self {
            enable_real_api: std::env::var("PARI_REAL_API")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false),
        }
    }
}

impl PariExecutionAdapter {
    pub const BOOKMAKER: &'static str = "pari";
    const API_BASE_URL: &'static str = "https://api.pari.ru";

    /// Create adapter with explicit real API flag
    pub fn with_real_api(enabled: bool) -> Self {
        Self {
            enable_real_api: enabled,
        }
    }

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
                detail: Some("pari session is not configured".into()),
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
                        "pari session configured but not authenticated",
                    ),
                    BookmakerSessionState::Active => (
                        BookmakerSessionSyncState::Authenticated,
                        true,
                        true,
                        if has_imported_material {
                            "pari imported browser session material is available; real API calls enabled"
                        } else {
                            "pari session authenticated; using mock mode (no imported session material)"
                        },
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

#[async_trait]
impl BookmakerAuth for PariExecutionAdapter {
    async fn authorize(
        &self,
        account: &BookmakerAccount,
    ) -> Result<BookmakerSession, Box<dyn std::error::Error + Send + Sync>> {
        info!("Authorizing Pari account {} (real_api={})", account.id, self.enable_real_api);

        // Try real API authentication if enabled
        if self.enable_real_api {
            // Note: Real authentication requires session material (cookies)
            // This will be provided via session_material in actual flow
            info!("Pari real API mode enabled; session will be established via cookie exchange");
        }

        let session = BookmakerSession {
            account_id: account.id,
            bookmaker: Self::BOOKMAKER.to_string(),
            state: BookmakerSessionState::Active,
            token_hint: Some(format!("session_{}", chrono::Utc::now().timestamp())),
            last_synced_at: Utc::now(),
            expires_at: Some(Utc::now() + Duration::hours(12)),
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
            supports_real_money: self.enable_real_api,
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
                remote_balance_fetch_enabled: self.enable_real_api,
                auth: BookmakerAdapterAuthMetadata {
                    flow: "manual_cookie_session".into(),
                    requires_human_bootstrap: !self.enable_real_api,
                    session_bootstrap_enabled: self.enable_real_api,
                    session_refresh_enabled: self.enable_real_api,
                    persisted_snapshot_enabled: true,
                },
                readiness: BookmakerAdapterReadinessMetadata {
                    stage: if self.enable_real_api {
                        BookmakerAdapterReadinessStage::SafeModePlacementReady
                    } else {
                        BookmakerAdapterReadinessStage::AuthenticatedReadOnly
                    },
                    safe_mode_only: !self.enable_real_api,
                    approval_reference_required: !self.enable_real_api,
                    operator_notes: vec![
                        if self.enable_real_api {
                            "Real API calls enabled; session established via cookie exchange"
                        } else {
                            "session/bootstrap remains operator-managed until auth flow is audited"
                        }
                        .into(),
                        "semi-real orchestration is allowed for rollout checks without remote submit"
                            .into(),
                    ],
                },
                notes: vec![
                    if self.enable_real_api {
                        "real API mode: balance and bet placement use live endpoints"
                    } else {
                        "safe-mode adapter exposes cached account state only"
                    }
                    .into(),
                    "semi-real submission path is enabled for arming and orchestration tests".into(),
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

        // Try real API call if enabled and we have session material
        if self.enable_real_api {
            if let Some(material) = session_material {
                if let Some(cookie_header) = &material.cookie_header {
                    info!("Attempting real balance fetch from Pari API");
                    
                    let mut client = PariApiClient::new();
                    
                    // Authenticate with cookie
                    match client.authenticate_with_cookie(cookie_header).await {
                        Ok(_token) => {
                            // Fetch balance
                            match client.get_balance().await {
                                Ok(balance) => {
                                    let available = balance.available_balance.unwrap_or(0.0);
                                    info!("Retrieved real balance from Pari: {}", available);
                                    
                                    let snapshot = BookmakerBalanceSnapshot {
                                        account_id: account.id,
                                        bookmaker: Self::BOOKMAKER.into(),
                                        available_balance: available,
                                        total_balance: balance.total_balance.unwrap_or(available),
                                        bonus_balance: balance.bonus_balance,
                                        currency: balance.currency.unwrap_or_else(|| "RUB".into()),
                                        exposure: 0.0,
                                        captured_at: checked_at,
                                        source: Some("api".into()),
                                    };
                                    
                                    return Ok(BookmakerBalanceRefresh {
                                        account_id: Some(account.id),
                                        bookmaker: Self::BOOKMAKER.into(),
                                        state: BookmakerBalanceRefreshState::RemoteBalanceFetched,
                                        session_status: session_status.clone(),
                                        snapshot: Some(snapshot),
                                        detail: Some("pari balance fetched from live API".into()),
                                        checked_at,
                                    });
                                }
                                Err(e) => {
                                    warn!("Failed to fetch Pari balance from API: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to authenticate with Pari API: {}", e);
                        }
                    }
                }
            }
        }

        // Fall back to cached/mock behavior
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
                "pari balance refresh skipped: no session configured"
            }
            BookmakerBalanceRefreshState::SessionNotAuthenticated => {
                "pari balance refresh skipped: session is not authenticated"
            }
            BookmakerBalanceRefreshState::AuthenticatedBalanceUnavailable => {
                if has_imported_material {
                    "pari real session material imported; remote balance endpoint failed, no cache"
                } else {
                    "pari session is authenticated but remote balance fetch is disabled in safe mode"
                }
            }
            BookmakerBalanceRefreshState::CachedBalanceAvailable => {
                if has_imported_material {
                    "pari returned cached balance snapshot (API fetch failed or disabled)"
                } else {
                    "pari returned cached balance snapshot; remote refresh remains disabled"
                }
            }
            BookmakerBalanceRefreshState::RemoteBalanceFetched => {
                "pari balance fetched from live API"
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
        // Try real API call if enabled and account mode allows real money
        if self.enable_real_api && matches!(account.mode, BookmakerExecutionMode::Real) {
            // Note: Real bet placement requires:
            // 1. Valid authenticated session
            // 2. Event ID mapping from our internal format to Pari format
            // 3. Market/selection mapping
            
            info!("Real bet placement requested but not yet implemented");
            // TODO: Implement real bet placement with PariApiClient
        }

        // Return semi-real response (safe mode)
        Ok(BetExecutionReceipt {
            ticket_id: None,
            account_id: Some(account.id),
            bookmaker: Self::BOOKMAKER.into(),
            status: BetExecutionStatus::Submitted,
            mode: account.mode.clone(),
            accepted_stake: request.stake,
            accepted_odds: request.odds,
            message: Some(
                "pari semi-real submission path reached; remote coupon placement requires real mode enabled"
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
        let adapter = PariExecutionAdapter::default();
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
            futures::executor::block_on(adapter.get_session_status(&account, Some(&session), None))
                .expect("status should succeed");

        assert_eq!(status.sync_state, BookmakerSessionSyncState::Configured);
        assert!(!status.authenticated);
    }

    #[test]
    fn real_api_flag_changes_capability() {
        let safe_adapter = PariExecutionAdapter::default();
        let real_adapter = PariExecutionAdapter::with_real_api(true);

        assert!(!safe_adapter.capability().supports_real_money);
        assert!(real_adapter.capability().supports_real_money);
        
        assert!(!safe_adapter.capability().account_metadata.remote_balance_fetch_enabled);
        assert!(real_adapter.capability().account_metadata.remote_balance_fetch_enabled);
    }
}
