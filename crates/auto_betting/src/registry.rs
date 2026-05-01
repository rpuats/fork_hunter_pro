use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use shared::{
    BetExecutionReceipt, BetExecutionRequest, BetExecutionStatus, BookmakerAccount,
    BookmakerAdapterReadinessStage, BookmakerAuthSnapshot, BookmakerAuthState,
    BookmakerBalanceRefresh, BookmakerBalanceRefreshState, BookmakerBalanceSnapshot,
    BookmakerExecutionCapability, BookmakerExecutionMode, BookmakerSession, BookmakerSessionState,
    BookmakerSessionStatus, BookmakerSessionSyncState,
};

use crate::adapters::register_builtin_adapters;
use crate::auth::{BookmakerSessionMaterial, BookmakerSessionMaterialSummary};
use crate::execution::{BookmakerExecutionAdapter, NoopExecutionAdapter};
use crate::persistence::ExecutionRegistryPersistence;

pub struct ExecutionRegistry {
    accounts: DashMap<String, BookmakerAccount>,
    sessions: DashMap<String, BookmakerSession>,
    session_materials: DashMap<String, BookmakerSessionMaterial>,
    balances: DashMap<String, BookmakerBalanceSnapshot>,
    auth_snapshots: DashMap<String, BookmakerAuthSnapshot>,
    adapters: DashMap<String, Arc<dyn BookmakerExecutionAdapter>>,
    persistence: Option<Arc<dyn ExecutionRegistryPersistence>>,
}

impl ExecutionRegistry {
    pub fn new() -> Self {
        Self::new_inner(None)
    }

    pub fn with_persistence(persistence: Arc<dyn ExecutionRegistryPersistence>) -> Self {
        Self::new_inner(Some(persistence))
    }

    fn new_inner(persistence: Option<Arc<dyn ExecutionRegistryPersistence>>) -> Self {
        let registry = Self {
            accounts: DashMap::new(),
            sessions: DashMap::new(),
            session_materials: DashMap::new(),
            balances: DashMap::new(),
            auth_snapshots: DashMap::new(),
            adapters: DashMap::new(),
            persistence,
        };

        register_builtin_adapters(&registry);
        registry
    }

    pub async fn restore_persisted_state(&self) -> Result<(), String> {
        let Some(persistence) = self.persistence.as_ref() else {
            return Ok(());
        };

        let snapshot = persistence.load_snapshot().await?;

        for account in snapshot.accounts {
            self.accounts.insert(account.bookmaker.clone(), account);
        }

        for session in snapshot.sessions {
            self.sessions.insert(session.bookmaker.clone(), session);
        }

        for balance in snapshot.balances {
            self.balances.insert(balance.bookmaker.clone(), balance);
        }

        for auth_snapshot in snapshot.auth_snapshots {
            self.auth_snapshots
                .insert(auth_snapshot.bookmaker.clone(), auth_snapshot);
        }

        Ok(())
    }

    pub fn register_account(&self, account: BookmakerAccount) {
        let bookmaker = account.bookmaker.clone();
        self.accounts
            .insert(account.bookmaker.clone(), account.clone());
        self.persist_account(account);
        self.refresh_auth_snapshot_for_bookmaker(&bookmaker);
    }

    pub fn get_account(&self, bookmaker: &str) -> Option<BookmakerAccount> {
        self.accounts.get(bookmaker).map(|entry| entry.clone())
    }

    pub fn update_account_control_state(
        &self,
        bookmaker: &str,
        enabled: bool,
        mode: BookmakerExecutionMode,
    ) -> Result<BookmakerAccount, String> {
        if !matches!(
            mode,
            BookmakerExecutionMode::Disabled
                | BookmakerExecutionMode::DryRun
                | BookmakerExecutionMode::Armed
        ) {
            return Err(
                "operator control updates are restricted to disabled/dry-run/armed modes".into(),
            );
        }

        validate_operator_control_state_transition(
            self.get_account(bookmaker).as_ref(),
            self.get_session(bookmaker).as_ref(),
            self.get_balance_snapshot(bookmaker).as_ref(),
            &self.get_capability(bookmaker),
            enabled,
            &mode,
        )?;

        let mut account = self
            .accounts
            .get_mut(bookmaker)
            .ok_or_else(|| format!("bookmaker account '{bookmaker}' not found"))?;

        account.enabled = enabled;
        account.mode = mode;

        let updated = (*account).clone();
        drop(account);

        self.persist_account(updated.clone());
        self.refresh_auth_snapshot_for_bookmaker(bookmaker);
        Ok(updated)
    }

    pub fn list_bookmakers(&self) -> Vec<String> {
        let mut bookmakers = std::collections::BTreeSet::new();

        for entry in self.accounts.iter() {
            bookmakers.insert(entry.key().clone());
        }

        for entry in self.sessions.iter() {
            bookmakers.insert(entry.key().clone());
        }

        for entry in self.balances.iter() {
            bookmakers.insert(entry.key().clone());
        }

        for entry in self.auth_snapshots.iter() {
            bookmakers.insert(entry.key().clone());
        }

        for entry in self.adapters.iter() {
            bookmakers.insert(entry.key().clone());
        }

        bookmakers.into_iter().collect()
    }

    pub fn upsert_session(&self, session: BookmakerSession) {
        let bookmaker = session.bookmaker.clone();
        self.sessions.insert(bookmaker.clone(), session.clone());
        self.persist_session(session);
        self.refresh_auth_snapshot_for_bookmaker(&bookmaker);
    }

    pub fn get_session(&self, bookmaker: &str) -> Option<BookmakerSession> {
        self.sessions.get(bookmaker).map(|entry| entry.clone())
    }

    pub fn upsert_session_material(
        &self,
        bookmaker: impl Into<String>,
        material: BookmakerSessionMaterial,
    ) {
        let bookmaker = bookmaker.into();
        self.session_materials.insert(bookmaker.clone(), material);
        self.refresh_auth_snapshot_for_bookmaker(&bookmaker);
    }

    pub fn get_session_material(&self, bookmaker: &str) -> Option<BookmakerSessionMaterial> {
        self.session_materials
            .get(bookmaker)
            .map(|entry| entry.clone())
    }

    pub fn get_session_material_summary(
        &self,
        bookmaker: &str,
    ) -> Option<BookmakerSessionMaterialSummary> {
        self.session_materials
            .get(bookmaker)
            .map(|entry| entry.summary())
    }

    pub fn upsert_balance_snapshot(&self, snapshot: BookmakerBalanceSnapshot) {
        let bookmaker = snapshot.bookmaker.clone();
        self.balances.insert(bookmaker.clone(), snapshot.clone());
        self.persist_balance_snapshot(snapshot);
        self.refresh_auth_snapshot_for_bookmaker(&bookmaker);
    }

    pub fn get_balance_snapshot(&self, bookmaker: &str) -> Option<BookmakerBalanceSnapshot> {
        self.balances.get(bookmaker).map(|entry| entry.clone())
    }

    pub fn list_balance_snapshots(&self) -> Vec<BookmakerBalanceSnapshot> {
        let mut snapshots: Vec<_> = self.balances.iter().map(|entry| entry.clone()).collect();
        snapshots.sort_by(|left, right| left.bookmaker.cmp(&right.bookmaker));
        snapshots
    }

    pub fn get_auth_snapshot(&self, bookmaker: &str) -> Option<BookmakerAuthSnapshot> {
        self.auth_snapshots
            .get(bookmaker)
            .map(|entry| entry.clone())
    }

    fn refresh_auth_snapshot_for_bookmaker(&self, bookmaker: &str) {
        if bookmaker.trim().is_empty() {
            return;
        }

        let snapshot = self.compute_auth_snapshot(bookmaker);
        self.auth_snapshots
            .insert(bookmaker.to_string(), snapshot.clone());
        self.persist_auth_snapshot(snapshot);
    }

    fn compute_auth_snapshot(&self, bookmaker: &str) -> BookmakerAuthSnapshot {
        let account = self.get_account(bookmaker);
        let session = self.get_session(bookmaker);
        let balance = self.get_balance_snapshot(bookmaker);
        let capability = self.get_capability(bookmaker);
        let captured_at = Utc::now();

        let auth_state = match session.as_ref() {
            None => BookmakerAuthState::NoSession,
            Some(session)
                if session
                    .expires_at
                    .is_some_and(|expires_at| expires_at <= captured_at) =>
            {
                BookmakerAuthState::Expired
            }
            Some(session) => match session.state {
                BookmakerSessionState::Configured => BookmakerAuthState::Configured,
                BookmakerSessionState::Active => BookmakerAuthState::Authenticated,
                BookmakerSessionState::Expired => BookmakerAuthState::Expired,
                BookmakerSessionState::Locked => BookmakerAuthState::Locked,
                BookmakerSessionState::Disconnected => BookmakerAuthState::Disconnected,
            },
        };
        let authenticated = matches!(auth_state, BookmakerAuthState::Authenticated);

        let placement_mode_enabled = account
            .as_ref()
            .map(|item| item.enabled && item.mode.allows_submission_path())
            .unwrap_or(false);
        let real_money_enabled = placement_mode_enabled && capability.supports_real_money;
        let safe_mode_blocked = placement_mode_enabled && !capability.supports_real_money;
        let readiness_stage = if real_money_enabled {
            BookmakerAdapterReadinessStage::RealMoneyReady
        } else if safe_mode_blocked {
            BookmakerAdapterReadinessStage::SafeModePlacementReady
        } else if authenticated {
            BookmakerAdapterReadinessStage::AuthenticatedReadOnly
        } else {
            BookmakerAdapterReadinessStage::SessionBootstrapPending
        };

        let detail = Some(match readiness_stage {
            BookmakerAdapterReadinessStage::RealMoneyReady => {
                format!("{bookmaker} adapter is authenticated and real-money capable")
            }
            BookmakerAdapterReadinessStage::SafeModePlacementReady => format!(
                "{bookmaker} adapter reached safe-mode placement readiness; submit remains blocked"
            ),
            BookmakerAdapterReadinessStage::AuthenticatedReadOnly => format!(
                "{bookmaker} adapter is authenticated for read-only checks with cached balance state"
            ),
            BookmakerAdapterReadinessStage::SessionBootstrapPending => format!(
                "{bookmaker} adapter still requires operator-managed session/bootstrap readiness"
            ),
        });

        BookmakerAuthSnapshot {
            account_id: account.as_ref().map(|item| item.id),
            bookmaker: bookmaker.to_string(),
            auth_state,
            readiness_stage,
            mode: account.as_ref().map(|item| item.mode.clone()),
            enabled: account.as_ref().map(|item| item.enabled).unwrap_or(false),
            cached_balance_available: balance.is_some(),
            submit_enabled: placement_mode_enabled,
            real_money_enabled,
            safe_mode_blocked,
            session_last_synced_at: session.as_ref().map(|item| item.last_synced_at),
            balance_captured_at: balance.as_ref().map(|item| item.captured_at),
            last_authenticated_at: session
                .as_ref()
                .filter(|item| authenticated && matches!(item.state, BookmakerSessionState::Active))
                .map(|item| item.last_synced_at),
            detail,
            captured_at,
        }
    }

    pub fn register_adapter(
        &self,
        bookmaker: impl Into<String>,
        adapter: Arc<dyn BookmakerExecutionAdapter>,
    ) {
        self.adapters.insert(bookmaker.into(), adapter);
    }

    pub fn get_capability(&self, bookmaker: &str) -> BookmakerExecutionCapability {
        self.adapters
            .get(bookmaker)
            .map(|entry| entry.value().capability())
            .unwrap_or_else(|| NoopExecutionAdapter::new(bookmaker).capability())
    }

    pub async fn refresh_session_status(
        &self,
        bookmaker: &str,
    ) -> Result<BookmakerSessionStatus, String> {
        let account = self.get_account(bookmaker);
        let session = self.get_session(bookmaker);
        let session_material = self.get_session_material(bookmaker);

        let Some(account) = account else {
            self.refresh_auth_snapshot_for_bookmaker(bookmaker);
            return Ok(BookmakerSessionStatus {
                account_id: None,
                bookmaker: bookmaker.to_string(),
                sync_state: BookmakerSessionSyncState::NoSession,
                authenticated: false,
                can_refresh_balance: false,
                detail: Some("no account configured for bookmaker".into()),
                checked_at: Utc::now(),
            });
        };

        let status = if let Some(adapter) = self
            .adapters
            .get(bookmaker)
            .map(|entry| Arc::clone(entry.value()))
        {
            adapter
                .get_session_status(&account, session.as_ref(), session_material.as_ref())
                .await?
        } else {
            NoopExecutionAdapter::new(bookmaker)
                .get_session_status(&account, session.as_ref(), session_material.as_ref())
                .await?
        };

        if let Some(mut existing_session) = session {
            existing_session.state = map_sync_state_to_session_state(&status.sync_state);
            existing_session.last_synced_at = status.checked_at;
            self.upsert_session(existing_session);
        } else {
            self.refresh_auth_snapshot_for_bookmaker(bookmaker);
        }

        Ok(status)
    }

    pub async fn refresh_balance_snapshot(
        &self,
        bookmaker: &str,
    ) -> Result<BookmakerBalanceRefresh, String> {
        let Some(account) = self.get_account(bookmaker) else {
            let session_status = self.refresh_session_status(bookmaker).await?;
            return Ok(BookmakerBalanceRefresh {
                account_id: None,
                bookmaker: bookmaker.to_string(),
                state: BookmakerBalanceRefreshState::NoSession,
                session_status,
                snapshot: self.get_balance_snapshot(bookmaker),
                detail: Some("balance refresh skipped because no account is configured".into()),
                checked_at: Utc::now(),
            });
        };

        let session_status = self.refresh_session_status(bookmaker).await?;
        let cached_snapshot = self.get_balance_snapshot(bookmaker);
        let session_material = self.get_session_material(bookmaker);

        let refresh = if let Some(adapter) = self
            .adapters
            .get(bookmaker)
            .map(|entry| Arc::clone(entry.value()))
        {
            adapter
                .refresh_balance_snapshot(
                    &account,
                    &session_status,
                    cached_snapshot.as_ref(),
                    session_material.as_ref(),
                )
                .await?
        } else {
            NoopExecutionAdapter::new(bookmaker)
                .refresh_balance_snapshot(
                    &account,
                    &session_status,
                    cached_snapshot.as_ref(),
                    session_material.as_ref(),
                )
                .await?
        };

        if let Some(snapshot) = refresh.snapshot.clone() {
            self.upsert_balance_snapshot(snapshot.clone());
        } else {
            self.refresh_auth_snapshot_for_bookmaker(bookmaker);
        }

        Ok(refresh)
    }

    pub async fn execute_bet(
        &self,
        request: &BetExecutionRequest,
    ) -> Result<BetExecutionReceipt, String> {
        validate_execution_request(request)?;

        let bookmaker = request.bookmaker.as_str();
        let capability = self.get_capability(bookmaker);
        let account = self.get_account(bookmaker);

        if let Some(mut account) = account.clone() {
            account.last_used_at = Some(Utc::now());
            self.accounts.insert(bookmaker.to_string(), account.clone());
            self.persist_account(account);
        }

        if let Some(adapter) = self
            .adapters
            .get(bookmaker)
            .map(|entry| Arc::clone(entry.value()))
        {
            if let Some(account) = account.as_ref() {
                let receipt = if !account.enabled
                    || matches!(account.mode, BookmakerExecutionMode::Disabled)
                {
                    blocked_receipt(
                        Some(account),
                        request,
                        account.mode.clone(),
                        "bookmaker account is disabled for execution",
                    )
                } else if matches!(account.mode, BookmakerExecutionMode::Armed) {
                    armed_receipt(
                        account,
                        request,
                        "account is armed, but semi-real submission mode is not enabled",
                    )
                } else if account.mode.allows_submission_path() {
                    if request
                        .reference
                        .as_deref()
                        .map(str::trim)
                        .filter(|reference| !reference.is_empty())
                        .is_none()
                    {
                        return Err(
                            "submission-path execution requires a non-empty approval reference"
                                .into(),
                        );
                    }

                    if request.allow_dry_run {
                        armed_receipt(
                            account,
                            request,
                            "submission path remains approval-gated because request is marked as dry-run",
                        )
                    } else if !capability.supports_real_money {
                        armed_receipt(
                            account,
                            request,
                            "submission path reached approval gate, but remote coupon submit remains disabled in safe mode",
                        )
                    } else if capability.supports_bet_placement {
                        adapter.place_bet(account, request).await?
                    } else {
                        blocked_receipt(
                            Some(account),
                            request,
                            account.mode.clone(),
                            "bookmaker adapter is not armed for submission; placement path remains disabled",
                        )
                    }
                } else {
                    adapter.dry_run(Some(account), request).await?
                };
                return Ok(receipt);
            }

            return adapter.dry_run(None, request).await;
        }

        NoopExecutionAdapter::new(bookmaker)
            .dry_run(account.as_ref(), request)
            .await
    }

    pub async fn dry_run_bet(
        &self,
        request: &BetExecutionRequest,
    ) -> Result<BetExecutionReceipt, String> {
        validate_execution_request(request)?;

        let bookmaker = request.bookmaker.as_str();
        let account = self.get_account(bookmaker);

        if let Some(adapter) = self
            .adapters
            .get(bookmaker)
            .map(|entry| Arc::clone(entry.value()))
        {
            return adapter.dry_run(account.as_ref(), request).await;
        }

        NoopExecutionAdapter::new(bookmaker)
            .dry_run(account.as_ref(), request)
            .await
    }

    fn persist_account(&self, account: BookmakerAccount) {
        let Some(persistence) = self.persistence.as_ref().map(Arc::clone) else {
            return;
        };

        spawn_persistence_task(async move { persistence.save_account(&account).await });
    }

    fn persist_session(&self, session: BookmakerSession) {
        let Some(persistence) = self.persistence.as_ref().map(Arc::clone) else {
            return;
        };

        spawn_persistence_task(async move { persistence.save_session(&session).await });
    }

    fn persist_balance_snapshot(&self, snapshot: BookmakerBalanceSnapshot) {
        let Some(persistence) = self.persistence.as_ref().map(Arc::clone) else {
            return;
        };

        spawn_persistence_task(async move { persistence.save_balance_snapshot(&snapshot).await });
    }

    fn persist_auth_snapshot(&self, snapshot: BookmakerAuthSnapshot) {
        let Some(persistence) = self.persistence.as_ref().map(Arc::clone) else {
            return;
        };

        spawn_persistence_task(async move { persistence.save_auth_snapshot(&snapshot).await });
    }
}

fn spawn_persistence_task<F>(future: F)
where
    F: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            if let Err(error) = future.await {
                tracing::warn!(error = %error, "execution registry persistence update failed");
            }
        });
    } else {
        tracing::warn!("execution registry persistence skipped because no Tokio runtime is active");
    }
}

fn validate_operator_control_state_transition(
    account: Option<&BookmakerAccount>,
    session: Option<&BookmakerSession>,
    balance: Option<&BookmakerBalanceSnapshot>,
    capability: &BookmakerExecutionCapability,
    enabled: bool,
    mode: &BookmakerExecutionMode,
) -> Result<(), String> {
    if account.is_none() {
        return Err("bookmaker account state not found".into());
    }

    if matches!(mode, BookmakerExecutionMode::Armed) {
        if !enabled {
            return Err("armed mode requires the account to stay enabled".into());
        }

        if !capability.supports_dry_run {
            return Err("armed mode requires a dry-run capable bookmaker adapter".into());
        }

        if !capability.supports_bet_placement {
            return Err("armed mode requires bookmaker placement support".into());
        }

        if capability.requires_session
            && !session
                .map(|item| matches!(item.state, BookmakerSessionState::Active))
                .unwrap_or(false)
        {
            return Err("armed mode requires an active bookmaker session".into());
        }

        if capability.supports_balance_snapshot && balance.is_none() {
            return Err("armed mode requires a cached bookmaker balance snapshot".into());
        }
    }

    Ok(())
}

fn blocked_receipt(
    account: Option<&BookmakerAccount>,
    request: &BetExecutionRequest,
    mode: BookmakerExecutionMode,
    message: impl Into<String>,
) -> BetExecutionReceipt {
    BetExecutionReceipt {
        ticket_id: None,
        account_id: account.map(|account| account.id),
        bookmaker: request.bookmaker.clone(),
        status: BetExecutionStatus::Blocked,
        mode,
        accepted_stake: 0.0,
        accepted_odds: request.odds,
        message: Some(message.into()),
        placed_at: Utc::now(),
    }
}

fn armed_receipt(
    account: &BookmakerAccount,
    request: &BetExecutionRequest,
    message: impl Into<String>,
) -> BetExecutionReceipt {
    BetExecutionReceipt {
        ticket_id: None,
        account_id: Some(account.id),
        bookmaker: request.bookmaker.clone(),
        status: BetExecutionStatus::Armed,
        mode: account.mode.clone(),
        accepted_stake: request.stake,
        accepted_odds: request.odds,
        message: Some(message.into()),
        placed_at: Utc::now(),
    }
}

impl Default for ExecutionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_execution_request(request: &BetExecutionRequest) -> Result<(), String> {
    if request.bookmaker.trim().is_empty() {
        return Err("bookmaker is required".into());
    }

    if request.event_id.trim().is_empty() {
        return Err("event_id is required".into());
    }

    if request.market.trim().is_empty() {
        return Err("market is required".into());
    }

    if request.selection.trim().is_empty() {
        return Err("selection is required".into());
    }

    if !request.odds.is_finite() || request.odds <= 1.0 {
        return Err("odds must be finite and greater than 1.0".into());
    }

    if !request.stake.is_finite() || request.stake <= 0.0 {
        return Err("stake must be finite and positive".into());
    }

    if let Some(reference) = request.reference.as_deref() {
        if reference.trim().is_empty() {
            return Err("reference must be non-empty when provided".into());
        }
    }

    Ok(())
}

fn map_sync_state_to_session_state(
    sync_state: &BookmakerSessionSyncState,
) -> BookmakerSessionState {
    match sync_state {
        BookmakerSessionSyncState::NoSession => BookmakerSessionState::Disconnected,
        BookmakerSessionSyncState::Configured => BookmakerSessionState::Configured,
        BookmakerSessionSyncState::Authenticated => BookmakerSessionState::Active,
        BookmakerSessionSyncState::Expired => BookmakerSessionState::Expired,
        BookmakerSessionSyncState::Locked => BookmakerSessionState::Locked,
        BookmakerSessionSyncState::Disconnected => BookmakerSessionState::Disconnected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::ExecutionRegistrySnapshot;
    use async_trait::async_trait;
    use shared::{
        BetExecutionStatus, BookmakerAdapterReadinessStage, BookmakerAuthState,
        BookmakerExecutionMode, BookmakerSessionState,
    };
    use std::sync::Mutex;
    use uuid::Uuid;

    #[derive(Default)]
    struct TestPersistence {
        snapshot: Mutex<ExecutionRegistrySnapshot>,
    }

    #[async_trait]
    impl ExecutionRegistryPersistence for TestPersistence {
        async fn load_snapshot(&self) -> Result<ExecutionRegistrySnapshot, String> {
            Ok(self.snapshot.lock().unwrap().clone())
        }

        async fn save_account(&self, account: &BookmakerAccount) -> Result<(), String> {
            let mut snapshot = self.snapshot.lock().unwrap();
            snapshot
                .accounts
                .retain(|item| item.bookmaker != account.bookmaker);
            snapshot.accounts.push(account.clone());
            Ok(())
        }

        async fn save_session(&self, session: &BookmakerSession) -> Result<(), String> {
            let mut snapshot = self.snapshot.lock().unwrap();
            snapshot
                .sessions
                .retain(|item| item.bookmaker != session.bookmaker);
            snapshot.sessions.push(session.clone());
            Ok(())
        }

        async fn save_balance_snapshot(
            &self,
            balance: &BookmakerBalanceSnapshot,
        ) -> Result<(), String> {
            let mut snapshot = self.snapshot.lock().unwrap();
            snapshot
                .balances
                .retain(|item| item.bookmaker != balance.bookmaker);
            snapshot.balances.push(balance.clone());
            Ok(())
        }

        async fn save_auth_snapshot(
            &self,
            auth_snapshot: &BookmakerAuthSnapshot,
        ) -> Result<(), String> {
            let mut snapshot = self.snapshot.lock().unwrap();
            snapshot
                .auth_snapshots
                .retain(|item| item.bookmaker != auth_snapshot.bookmaker);
            snapshot.auth_snapshots.push(auth_snapshot.clone());
            Ok(())
        }
    }

    #[test]
    fn stores_account_session_and_balance() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());
        registry.upsert_session(BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 7_500.0,
            exposure: 2_500.0,
            captured_at: Utc::now(),
        });

        assert!(registry.get_account("pari").is_some());
        assert!(registry.get_session("pari").is_some());
        assert_eq!(
            registry
                .get_balance_snapshot("pari")
                .unwrap()
                .available_balance,
            7_500.0
        );
        assert_eq!(
            registry
                .get_auth_snapshot("pari")
                .expect("auth snapshot should exist")
                .auth_state,
            BookmakerAuthState::Authenticated
        );
    }

    #[test]
    fn falls_back_to_noop_execution_when_bookmaker_is_unregistered() {
        let registry = ExecutionRegistry::new();

        let receipt = futures::executor::block_on(registry.execute_bet(&BetExecutionRequest {
            bookmaker: "olimp".into(),
            event_id: "event-1".into(),
            market: "1X2".into(),
            selection: "1".into(),
            odds: 2.15,
            stake: 500.0,
            allow_dry_run: true,
            reference: None,
        }))
        .expect("noop receipt should succeed");

        assert_eq!(receipt.status, BetExecutionStatus::DryRun);
        assert_eq!(receipt.mode, BookmakerExecutionMode::NoOp);
    }

    #[test]
    fn dry_run_stays_read_only_for_real_mode_accounts() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::Real,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());

        let receipt = futures::executor::block_on(registry.dry_run_bet(&BetExecutionRequest {
            bookmaker: account.bookmaker.clone(),
            event_id: "event-1".into(),
            market: "1X2".into(),
            selection: "1".into(),
            odds: 2.15,
            stake: 500.0,
            allow_dry_run: true,
            reference: Some("test".into()),
        }))
        .expect("dry run should succeed");

        assert_eq!(receipt.status, BetExecutionStatus::DryRun);
        assert_eq!(receipt.mode, BookmakerExecutionMode::DryRun);
        assert_eq!(receipt.account_id, Some(account.id));
    }

    #[test]
    fn armed_mode_returns_armed_receipt_without_submission() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::Armed,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());

        let receipt = futures::executor::block_on(registry.execute_bet(&BetExecutionRequest {
            bookmaker: account.bookmaker.clone(),
            event_id: "event-1".into(),
            market: "1X2".into(),
            selection: "1".into(),
            odds: 2.15,
            stake: 500.0,
            allow_dry_run: true,
            reference: Some("armed".into()),
        }))
        .expect("armed receipt should succeed");

        assert_eq!(receipt.status, BetExecutionStatus::Armed);
        assert_eq!(receipt.mode, BookmakerExecutionMode::Armed);
        assert_eq!(receipt.account_id, Some(account.id));
    }

    #[test]
    fn semi_real_mode_stays_approval_gated_for_dry_run_requests() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "fonbet".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::SemiRealReady,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());

        let receipt = futures::executor::block_on(registry.execute_bet(&BetExecutionRequest {
            bookmaker: account.bookmaker.clone(),
            event_id: "event-1".into(),
            market: "1X2".into(),
            selection: "1".into(),
            odds: 2.15,
            stake: 500.0,
            allow_dry_run: true,
            reference: Some("semi-real".into()),
        }))
        .expect("blocked receipt should succeed");

        assert_eq!(receipt.status, BetExecutionStatus::Armed);
        assert_eq!(receipt.mode, BookmakerExecutionMode::SemiRealReady);
        assert_eq!(receipt.accepted_stake, 500.0);
        assert!(receipt
            .message
            .unwrap_or_default()
            .contains("marked as dry-run"));
    }

    #[test]
    fn semi_real_mode_for_pari_stays_approval_gated() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::SemiRealReady,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());

        let receipt = futures::executor::block_on(registry.execute_bet(&BetExecutionRequest {
            bookmaker: account.bookmaker.clone(),
            event_id: "event-1".into(),
            market: "1X2".into(),
            selection: "1".into(),
            odds: 2.15,
            stake: 500.0,
            allow_dry_run: true,
            reference: Some("pari-submission".into()),
        }))
        .expect("pari approval gate receipt should succeed");

        assert_eq!(receipt.status, BetExecutionStatus::Armed);
        assert_eq!(receipt.mode, BookmakerExecutionMode::SemiRealReady);
        assert_eq!(receipt.account_id, Some(account.id));
        assert!(receipt
            .message
            .unwrap_or_default()
            .contains("marked as dry-run"));
    }

    #[test]
    fn operator_control_updates_only_allow_safe_modes() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account);

        let error = registry
            .update_account_control_state("pari", true, BookmakerExecutionMode::Real)
            .expect_err("unsafe mode must be rejected");

        assert!(error.contains("restricted"));
    }

    #[test]
    fn operator_control_updates_persist_safe_state_changes() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::Real,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());
        registry.upsert_session(BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            currency: account.currency.clone(),
            total_balance: 10_000.0,
            available_balance: 7_500.0,
            exposure: 2_500.0,
            captured_at: Utc::now(),
        });

        let updated = registry
            .update_account_control_state("pari", true, BookmakerExecutionMode::Armed)
            .expect("safe mode update should succeed");

        assert!(updated.enabled);
        assert_eq!(updated.mode, BookmakerExecutionMode::Armed);
        assert_eq!(
            registry
                .get_account("pari")
                .expect("account should remain present")
                .mode,
            BookmakerExecutionMode::Armed
        );
    }

    #[test]
    fn operator_control_updates_reject_arming_when_readiness_is_incomplete() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account);

        let error = registry
            .update_account_control_state("pari", true, BookmakerExecutionMode::Armed)
            .expect_err("arming without readiness should be rejected");

        assert!(error.contains("active bookmaker session") || error.contains("balance snapshot"));
    }

    #[test]
    fn execution_request_validation_rejects_invalid_odds() {
        let error = validate_execution_request(&BetExecutionRequest {
            bookmaker: "pari".into(),
            event_id: "event-1".into(),
            market: "1X2".into(),
            selection: "1".into(),
            odds: 1.0,
            stake: 500.0,
            allow_dry_run: true,
            reference: Some("approval-1".into()),
        })
        .expect_err("invalid odds must be rejected");

        assert!(error.contains("odds"));
    }

    #[test]
    fn submission_path_requires_audit_reference() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::SemiRealReady,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account);

        let error = futures::executor::block_on(registry.execute_bet(&BetExecutionRequest {
            bookmaker: "pari".into(),
            event_id: "event-1".into(),
            market: "1X2".into(),
            selection: "1".into(),
            odds: 2.15,
            stake: 500.0,
            allow_dry_run: false,
            reference: None,
        }))
        .expect_err("submission path without reference must be rejected");

        assert!(error.contains("approval reference"));
    }

    #[test]
    fn registers_builtin_execution_adapters() {
        let registry = ExecutionRegistry::new();

        let pari = registry.get_capability("pari");
        let fonbet = registry.get_capability("fonbet");

        assert_eq!(pari.bookmaker, "pari");
        assert!(pari.supports_dry_run);
        assert!(pari.requires_session);
        assert!(pari.supports_balance_snapshot);
        assert!(pari.supports_bet_placement);
        assert!(!pari.supports_real_money);
        assert!(pari.account_metadata.supports_read_only_session_sync);
        assert_eq!(pari.account_metadata.auth.flow, "manual_cookie_session");
        assert!(pari.account_metadata.readiness.safe_mode_only);
        assert_eq!(
            pari.account_metadata.api_base_url.as_deref(),
            Some("https://api.pari.ru")
        );

        assert_eq!(fonbet.bookmaker, "fonbet");
        assert!(fonbet.supports_dry_run);
        assert!(fonbet.requires_session);
        assert!(fonbet.supports_balance_snapshot);
        assert!(!fonbet.supports_bet_placement);
        assert!(!fonbet.supports_real_money);
        assert!(fonbet.account_metadata.supports_read_only_balance_refresh);
        assert!(fonbet.account_metadata.auth.requires_human_bootstrap);
        assert_eq!(
            fonbet.account_metadata.api_base_url.as_deref(),
            Some("https://clientsapi24.fonbet.ru")
        );
    }

    #[test]
    fn refresh_balance_snapshot_reports_cached_balance_when_session_is_active() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());
        registry.upsert_session(BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 7_500.0,
            exposure: 2_500.0,
            captured_at: Utc::now(),
        });

        let refresh = futures::executor::block_on(registry.refresh_balance_snapshot("pari"))
            .expect("refresh should succeed");

        assert_eq!(
            refresh.state,
            BookmakerBalanceRefreshState::CachedBalanceAvailable
        );
        assert_eq!(
            refresh.session_status.sync_state,
            BookmakerSessionSyncState::Authenticated
        );
        assert_eq!(refresh.snapshot.unwrap().available_balance, 7_500.0);
        assert_eq!(
            registry
                .get_auth_snapshot("pari")
                .expect("auth snapshot should exist")
                .readiness_stage,
            BookmakerAdapterReadinessStage::AuthenticatedReadOnly
        );
    }

    #[test]
    fn refresh_balance_snapshot_reports_configured_but_not_authenticated() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "fonbet".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());
        registry.upsert_session(BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Configured,
            token_hint: Some("cfg...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });

        let refresh = futures::executor::block_on(registry.refresh_balance_snapshot("fonbet"))
            .expect("refresh should succeed");

        assert_eq!(
            refresh.state,
            BookmakerBalanceRefreshState::SessionNotAuthenticated
        );
        assert_eq!(
            refresh.session_status.sync_state,
            BookmakerSessionSyncState::Configured
        );
        assert!(refresh.snapshot.is_none());
    }

    #[test]
    fn lists_cached_balance_snapshots_in_bookmaker_order() {
        let registry = ExecutionRegistry::new();
        let pari_account_id = Uuid::new_v4();
        let fonbet_account_id = Uuid::new_v4();

        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: pari_account_id,
            bookmaker: "pari".into(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 8_000.0,
            exposure: 2_000.0,
            captured_at: Utc::now(),
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: fonbet_account_id,
            bookmaker: "fonbet".into(),
            currency: "RUB".into(),
            total_balance: 7_000.0,
            available_balance: 6_500.0,
            exposure: 500.0,
            captured_at: Utc::now(),
        });

        let snapshots = registry.list_balance_snapshots();

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].bookmaker, "fonbet");
        assert_eq!(snapshots[1].bookmaker, "pari");
    }

    #[tokio::test]
    async fn restores_and_persists_registry_state_with_store() {
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        };
        let session = BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        };
        let balance = BookmakerBalanceSnapshot {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 9_500.0,
            exposure: 500.0,
            captured_at: Utc::now(),
        };
        let store = Arc::new(TestPersistence {
            snapshot: Mutex::new(ExecutionRegistrySnapshot {
                accounts: vec![account.clone()],
                sessions: vec![session.clone()],
                balances: vec![balance.clone()],
                auth_snapshots: vec![BookmakerAuthSnapshot {
                    account_id: Some(account.id),
                    bookmaker: account.bookmaker.clone(),
                    auth_state: BookmakerAuthState::Authenticated,
                    readiness_stage: BookmakerAdapterReadinessStage::AuthenticatedReadOnly,
                    mode: Some(BookmakerExecutionMode::DryRun),
                    enabled: true,
                    cached_balance_available: true,
                    submit_enabled: false,
                    real_money_enabled: false,
                    safe_mode_blocked: false,
                    session_last_synced_at: Some(session.last_synced_at),
                    balance_captured_at: Some(balance.captured_at),
                    last_authenticated_at: Some(session.last_synced_at),
                    detail: Some("persisted auth snapshot".into()),
                    captured_at: Utc::now(),
                }],
            }),
        });
        let registry = ExecutionRegistry::with_persistence(store.clone());

        registry.restore_persisted_state().await.unwrap();
        assert_eq!(registry.get_account("pari").unwrap().id, account.id);
        assert_eq!(registry.get_session("pari").unwrap().account_id, account.id);
        assert_eq!(
            registry
                .get_auth_snapshot("pari")
                .expect("auth snapshot should restore")
                .auth_state,
            BookmakerAuthState::Authenticated
        );
        assert_eq!(
            registry
                .get_balance_snapshot("pari")
                .unwrap()
                .available_balance,
            9_500.0
        );

        let mut updated_account = account.clone();
        updated_account.label = "updated".into();
        registry.register_account(updated_account.clone());
        tokio::task::yield_now().await;

        let persisted = store.load_snapshot().await.unwrap();
        assert_eq!(persisted.accounts.len(), 1);
        assert_eq!(persisted.accounts[0].label, "updated");
        assert_eq!(persisted.auth_snapshots.len(), 1);
    }
}
