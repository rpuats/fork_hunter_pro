use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use shared::{
    BetExecutionReceipt, BetExecutionRequest, BetExecutionStatus, BookmakerAccount,
    BookmakerBalanceRefresh, BookmakerBalanceRefreshState, BookmakerBalanceSnapshot,
    BookmakerExecutionCapability, BookmakerExecutionMode, BookmakerSession, BookmakerSessionState,
    BookmakerSessionStatus, BookmakerSessionSyncState,
};

use crate::adapters::register_builtin_adapters;
use crate::execution::{BookmakerExecutionAdapter, NoopExecutionAdapter};
use crate::persistence::ExecutionRegistryPersistence;

pub struct ExecutionRegistry {
    accounts: DashMap<String, BookmakerAccount>,
    sessions: DashMap<String, BookmakerSession>,
    balances: DashMap<String, BookmakerBalanceSnapshot>,
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
            balances: DashMap::new(),
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

        Ok(())
    }

    pub fn register_account(&self, account: BookmakerAccount) {
        self.accounts
            .insert(account.bookmaker.clone(), account.clone());
        self.persist_account(account);
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

        let mut account = self
            .accounts
            .get_mut(bookmaker)
            .ok_or_else(|| format!("bookmaker account '{bookmaker}' not found"))?;

        account.enabled = enabled;
        account.mode = mode;

        let updated = (*account).clone();
        drop(account);

        self.persist_account(updated.clone());
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

        for entry in self.adapters.iter() {
            bookmakers.insert(entry.key().clone());
        }

        bookmakers.into_iter().collect()
    }

    pub fn upsert_session(&self, session: BookmakerSession) {
        self.sessions
            .insert(session.bookmaker.clone(), session.clone());
        self.persist_session(session);
    }

    pub fn get_session(&self, bookmaker: &str) -> Option<BookmakerSession> {
        self.sessions.get(bookmaker).map(|entry| entry.clone())
    }

    pub fn upsert_balance_snapshot(&self, snapshot: BookmakerBalanceSnapshot) {
        self.balances
            .insert(snapshot.bookmaker.clone(), snapshot.clone());
        self.persist_balance_snapshot(snapshot);
    }

    pub fn get_balance_snapshot(&self, bookmaker: &str) -> Option<BookmakerBalanceSnapshot> {
        self.balances.get(bookmaker).map(|entry| entry.clone())
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

        let Some(account) = account else {
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
                .get_session_status(&account, session.as_ref())
                .await?
        } else {
            NoopExecutionAdapter::new(bookmaker)
                .get_session_status(&account, session.as_ref())
                .await?
        };

        if let Some(mut existing_session) = session {
            existing_session.state = map_sync_state_to_session_state(&status.sync_state);
            existing_session.last_synced_at = status.checked_at;
            self.upsert_session(existing_session);
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

        let refresh = if let Some(adapter) = self
            .adapters
            .get(bookmaker)
            .map(|entry| Arc::clone(entry.value()))
        {
            adapter
                .refresh_balance_snapshot(&account, &session_status, cached_snapshot.as_ref())
                .await?
        } else {
            NoopExecutionAdapter::new(bookmaker)
                .refresh_balance_snapshot(&account, &session_status, cached_snapshot.as_ref())
                .await?
        };

        if let Some(snapshot) = refresh.snapshot.clone() {
            self.upsert_balance_snapshot(snapshot.clone());
        }

        Ok(refresh)
    }

    pub async fn execute_bet(
        &self,
        request: &BetExecutionRequest,
    ) -> Result<BetExecutionReceipt, String> {
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
                    if capability.supports_bet_placement {
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
    use shared::{BetExecutionStatus, BookmakerExecutionMode, BookmakerSessionState};
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
    fn semi_real_mode_is_blocked_when_adapter_submission_is_unsupported() {
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

        assert_eq!(receipt.status, BetExecutionStatus::Blocked);
        assert_eq!(receipt.mode, BookmakerExecutionMode::SemiRealReady);
        assert_eq!(receipt.accepted_stake, 0.0);
    }

    #[test]
    fn semi_real_mode_reaches_safe_submission_path_for_pari() {
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
        .expect("pari submission receipt should succeed");

        assert_eq!(receipt.status, BetExecutionStatus::Submitted);
        assert_eq!(receipt.mode, BookmakerExecutionMode::SemiRealReady);
        assert_eq!(receipt.account_id, Some(account.id));
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

        registry.register_account(account);

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
            }),
        });
        let registry = ExecutionRegistry::with_persistence(store.clone());

        registry.restore_persisted_state().await.unwrap();
        assert_eq!(registry.get_account("pari").unwrap().id, account.id);
        assert_eq!(registry.get_session("pari").unwrap().account_id, account.id);
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
    }
}
