use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Error, Result};
use async_trait::async_trait;
use auto_betting::{
    ExecutionRegistryPersistence, ExecutionRegistrySnapshot, ExecutionStatePersistence,
    ExecutionStateSnapshot, ExecutionStateTransition,
};
use chrono::Utc;
use shared::{BookmakerAccount, BookmakerAuthSnapshot, BookmakerBalanceSnapshot, BookmakerSession};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use std::str::FromStr;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Default)]
struct PersistedBookmakerState {
    account: Option<BookmakerAccount>,
    session: Option<BookmakerSession>,
    balance: Option<BookmakerBalanceSnapshot>,
    auth_snapshot: Option<BookmakerAuthSnapshot>,
}

pub struct ExecutionStateStore {
    pool: Option<Arc<SqlitePool>>,
    data: Arc<Mutex<HashMap<String, PersistedBookmakerState>>>,
    snapshots: Arc<Mutex<HashMap<uuid::Uuid, ExecutionStateSnapshot>>>,
    transitions: Arc<Mutex<Vec<ExecutionStateTransition>>>,
}

impl ExecutionStateStore {
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        if database_url.is_empty() || database_url == "memory" {
            info!("Using in-memory execution state storage");
            return Ok(Self {
                pool: None,
                data: Arc::new(Mutex::new(HashMap::new())),
                snapshots: Arc::new(Mutex::new(HashMap::new())),
                transitions: Arc::new(Mutex::new(Vec::new())),
            });
        }

        let connect_options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

        match SqlitePool::connect_with(connect_options).await {
            Ok(pool) => {
                info!(
                    url = database_url,
                    "Connected to SQLite for execution state"
                );
                let store = Self {
                    pool: Some(Arc::new(pool)),
                    data: Arc::new(Mutex::new(HashMap::new())),
                    snapshots: Arc::new(Mutex::new(HashMap::new())),
                    transitions: Arc::new(Mutex::new(Vec::new())),
                };
                store.migrate().await?;
                store.preload().await?;
                Ok(store)
            }
            Err(error) => {
                warn!(error = %error, "Failed to connect to SQLite for execution state, falling back to in-memory");
                Ok(Self {
                    pool: None,
                    data: Arc::new(Mutex::new(HashMap::new())),
                    snapshots: Arc::new(Mutex::new(HashMap::new())),
                    transitions: Arc::new(Mutex::new(Vec::new())),
                })
            }
        }
    }

    async fn migrate(&self) -> Result<(), Error> {
        if let Some(pool) = &self.pool {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS execution_registry_state (
                    bookmaker TEXT PRIMARY KEY,
                    account_json TEXT,
                    session_json TEXT,
                    balance_json TEXT,
                    auth_json TEXT,
                    updated_at INTEGER NOT NULL
                )
                "#,
            )
            .execute(pool.as_ref())
            .await?;

            let _ = sqlx::query(
                r#"
                ALTER TABLE execution_registry_state ADD COLUMN auth_json TEXT
                "#,
            )
            .execute(pool.as_ref())
            .await;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS execution_state_snapshots (
                    placement_id TEXT PRIMARY KEY,
                    bookmaker TEXT NOT NULL,
                    phase TEXT NOT NULL,
                    status TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    payload_json TEXT NOT NULL
                )
                "#,
            )
            .execute(pool.as_ref())
            .await?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS execution_state_transitions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    placement_id TEXT NOT NULL,
                    bookmaker TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    occurred_at INTEGER NOT NULL,
                    payload_json TEXT NOT NULL
                )
                "#,
            )
            .execute(pool.as_ref())
            .await?;

            info!("Execution registry state migration complete");
        }

        Ok(())
    }

    async fn preload(&self) -> Result<(), Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };

        let rows = sqlx::query(
            r#"
            SELECT bookmaker, account_json, session_json, balance_json, auth_json
            FROM execution_registry_state
            "#,
        )
        .fetch_all(pool.as_ref())
        .await?;

        let mut data = self.data.lock().await;
        data.clear();

        for row in rows {
            let bookmaker: String = row.try_get("bookmaker")?;
            let account = parse_json_column::<BookmakerAccount>(&row, "account_json", &bookmaker);
            let session = parse_json_column::<BookmakerSession>(&row, "session_json", &bookmaker);
            let balance =
                parse_json_column::<BookmakerBalanceSnapshot>(&row, "balance_json", &bookmaker);
            let auth_snapshot =
                parse_json_column::<BookmakerAuthSnapshot>(&row, "auth_json", &bookmaker);

            data.insert(
                bookmaker,
                PersistedBookmakerState {
                    account,
                    session,
                    balance,
                    auth_snapshot,
                },
            );
        }

        drop(data);

        let snapshot_rows = sqlx::query(
            r#"
            SELECT payload_json
            FROM execution_state_snapshots
            "#,
        )
        .fetch_all(pool.as_ref())
        .await?;

        let mut snapshots = self.snapshots.lock().await;
        snapshots.clear();

        for row in snapshot_rows {
            let payload: String = row.try_get("payload_json")?;
            match serde_json::from_str::<ExecutionStateSnapshot>(&payload) {
                Ok(snapshot) => {
                    snapshots.insert(snapshot.placement_id, snapshot);
                }
                Err(error) => {
                    warn!(error = %error, "Failed to deserialize execution state snapshot")
                }
            }
        }

        let transition_rows = sqlx::query(
            r#"
            SELECT payload_json
            FROM execution_state_transitions
            ORDER BY id ASC
            "#,
        )
        .fetch_all(pool.as_ref())
        .await?;

        let mut transitions = self.transitions.lock().await;
        transitions.clear();

        for row in transition_rows {
            let payload: String = row.try_get("payload_json")?;
            match serde_json::from_str::<ExecutionStateTransition>(&payload) {
                Ok(transition) => transitions.push(transition),
                Err(error) => {
                    warn!(error = %error, "Failed to deserialize execution state transition")
                }
            }
        }

        Ok(())
    }

    async fn save_account_inner(&self, account: &BookmakerAccount) -> Result<(), String> {
        self.update_state(&account.bookmaker, |state| {
            state.account = Some(account.clone());
        })
        .await
    }

    async fn save_session_inner(&self, session: &BookmakerSession) -> Result<(), String> {
        self.update_state(&session.bookmaker, |state| {
            state.session = Some(session.clone());
        })
        .await
    }

    async fn save_balance_inner(&self, snapshot: &BookmakerBalanceSnapshot) -> Result<(), String> {
        self.update_state(&snapshot.bookmaker, |state| {
            state.balance = Some(snapshot.clone());
        })
        .await
    }

    async fn save_auth_snapshot_inner(
        &self,
        snapshot: &BookmakerAuthSnapshot,
    ) -> Result<(), String> {
        self.update_state(&snapshot.bookmaker, |state| {
            state.auth_snapshot = Some(snapshot.clone());
        })
        .await
    }

    async fn save_execution_snapshot_inner(
        &self,
        snapshot: &ExecutionStateSnapshot,
    ) -> Result<(), String> {
        self.snapshots
            .lock()
            .await
            .insert(snapshot.placement_id, snapshot.clone());

        if let Some(pool) = &self.pool {
            let payload_json =
                serde_json::to_string(snapshot).map_err(|error| error.to_string())?;

            sqlx::query(
                r#"
                INSERT INTO execution_state_snapshots (
                    placement_id,
                    bookmaker,
                    phase,
                    status,
                    sequence,
                    updated_at,
                    payload_json
                )
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(placement_id) DO UPDATE SET
                    bookmaker = excluded.bookmaker,
                    phase = excluded.phase,
                    status = excluded.status,
                    sequence = excluded.sequence,
                    updated_at = excluded.updated_at,
                    payload_json = excluded.payload_json
                "#,
            )
            .bind(snapshot.placement_id.to_string())
            .bind(&snapshot.bookmaker)
            .bind(format!("{:?}", snapshot.phase))
            .bind(format!("{:?}", snapshot.placement_status))
            .bind(snapshot.sequence as i64)
            .bind(snapshot.updated_at.timestamp())
            .bind(payload_json)
            .execute(pool.as_ref())
            .await
            .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    async fn record_transition_inner(
        &self,
        transition: &ExecutionStateTransition,
    ) -> Result<(), String> {
        self.transitions.lock().await.push(transition.clone());

        if let Some(pool) = &self.pool {
            let payload_json =
                serde_json::to_string(transition).map_err(|error| error.to_string())?;

            sqlx::query(
                r#"
                INSERT INTO execution_state_transitions (
                    placement_id,
                    bookmaker,
                    sequence,
                    occurred_at,
                    payload_json
                )
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(transition.placement_id.to_string())
            .bind(&transition.bookmaker)
            .bind(transition.sequence as i64)
            .bind(transition.occurred_at.timestamp())
            .bind(payload_json)
            .execute(pool.as_ref())
            .await
            .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    pub async fn transition_count(&self) -> usize {
        self.transitions.lock().await.len()
    }

    pub async fn load_state_snapshots(&self) -> Vec<ExecutionStateSnapshot> {
        let mut snapshots = self
            .snapshots
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        snapshots.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        snapshots
    }

    pub async fn load_transitions(&self) -> Vec<ExecutionStateTransition> {
        self.transitions.lock().await.clone()
    }

    async fn update_state<F>(&self, bookmaker: &str, update: F) -> Result<(), String>
    where
        F: FnOnce(&mut PersistedBookmakerState),
    {
        let persisted = {
            let mut data = self.data.lock().await;
            let state = data.entry(bookmaker.to_string()).or_default();
            update(state);
            state.clone()
        };

        if let Some(pool) = &self.pool {
            let account_json = serialize_optional(&persisted.account)?;
            let session_json = serialize_optional(&persisted.session)?;
            let balance_json = serialize_optional(&persisted.balance)?;
            let auth_json = serialize_optional(&persisted.auth_snapshot)?;

            sqlx::query(
                r#"
                INSERT INTO execution_registry_state (
                    bookmaker,
                    account_json,
                    session_json,
                    balance_json,
                    auth_json,
                    updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT(bookmaker) DO UPDATE SET
                    account_json = excluded.account_json,
                    session_json = excluded.session_json,
                    balance_json = excluded.balance_json,
                    auth_json = excluded.auth_json,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(bookmaker)
            .bind(account_json)
            .bind(session_json)
            .bind(balance_json)
            .bind(auth_json)
            .bind(Utc::now().timestamp())
            .execute(pool.as_ref())
            .await
            .map_err(|error| error.to_string())?;
        }

        Ok(())
    }
}

#[async_trait]
impl ExecutionRegistryPersistence for ExecutionStateStore {
    async fn load_snapshot(&self) -> Result<ExecutionRegistrySnapshot, String> {
        let data = self.data.lock().await;

        Ok(ExecutionRegistrySnapshot {
            accounts: data
                .values()
                .filter_map(|item| item.account.clone())
                .collect(),
            sessions: data
                .values()
                .filter_map(|item| item.session.clone())
                .collect(),
            balances: data
                .values()
                .filter_map(|item| item.balance.clone())
                .collect(),
            auth_snapshots: data
                .values()
                .filter_map(|item| item.auth_snapshot.clone())
                .collect(),
        })
    }

    async fn save_account(&self, account: &BookmakerAccount) -> Result<(), String> {
        self.save_account_inner(account).await
    }

    async fn save_session(&self, session: &BookmakerSession) -> Result<(), String> {
        self.save_session_inner(session).await
    }

    async fn save_balance_snapshot(
        &self,
        snapshot: &BookmakerBalanceSnapshot,
    ) -> Result<(), String> {
        self.save_balance_inner(snapshot).await
    }

    async fn save_auth_snapshot(&self, snapshot: &BookmakerAuthSnapshot) -> Result<(), String> {
        self.save_auth_snapshot_inner(snapshot).await
    }
}

#[async_trait]
impl ExecutionStatePersistence for ExecutionStateStore {
    async fn load_snapshots(&self) -> Result<Vec<ExecutionStateSnapshot>, String> {
        Ok(self.snapshots.lock().await.values().cloned().collect())
    }

    async fn save_snapshot(&self, snapshot: &ExecutionStateSnapshot) -> Result<(), String> {
        self.save_execution_snapshot_inner(snapshot).await
    }

    async fn record_transition(&self, transition: &ExecutionStateTransition) -> Result<(), String> {
        self.record_transition_inner(transition).await
    }
}

fn serialize_optional<T: serde::Serialize>(value: &Option<T>) -> Result<Option<String>, String> {
    value
        .as_ref()
        .map(|item| serde_json::to_string(item).map_err(|error| error.to_string()))
        .transpose()
}

fn parse_json_column<T>(row: &sqlx::sqlite::SqliteRow, column: &str, bookmaker: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let raw = row.try_get::<Option<String>, _>(column).ok().flatten()?;
    match serde_json::from_str::<T>(&raw) {
        Ok(value) => Some(value),
        Err(error) => {
            warn!(bookmaker, column, error = %error, "Failed to deserialize persisted execution state column");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_betting::{ExecutionLedgerAction, ExecutionStatePhase};
    use shared::{
        BetStatus, BookmakerAdapterReadinessStage, BookmakerAuthSnapshot, BookmakerAuthState,
        BookmakerExecutionMode, BookmakerSessionState,
    };
    use uuid::Uuid;

    fn make_account(bookmaker: &str) -> BookmakerAccount {
        BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: bookmaker.into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        }
    }

    #[tokio::test]
    async fn stores_and_loads_execution_state_in_memory() {
        let store = ExecutionStateStore::new("memory").await.unwrap();
        let account = make_account("pari");
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
            total_balance: 15_000.0,
            available_balance: 12_000.0,
            exposure: 3_000.0,
            bonus_balance: Some(0.0),
            source: Some("test".into()),
            captured_at: Utc::now(),
        };
        let auth_snapshot = BookmakerAuthSnapshot {
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
            detail: Some("cached auth snapshot".into()),
            captured_at: Utc::now(),
        };

        store.save_account(&account).await.unwrap();
        store.save_session(&session).await.unwrap();
        store.save_balance_snapshot(&balance).await.unwrap();
        store.save_auth_snapshot(&auth_snapshot).await.unwrap();

        let snapshot = store.load_snapshot().await.unwrap();
        assert_eq!(snapshot.accounts.len(), 1);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.balances.len(), 1);
        assert_eq!(snapshot.auth_snapshots.len(), 1);
        assert_eq!(snapshot.accounts[0].bookmaker, "pari");
        assert_eq!(snapshot.balances[0].available_balance, 12_000.0);
        assert_eq!(
            snapshot.auth_snapshots[0].auth_state,
            BookmakerAuthState::Authenticated
        );
    }

    #[tokio::test]
    async fn stores_and_loads_execution_state_machine_snapshots() {
        let store = ExecutionStateStore::new("memory").await.unwrap();
        let placement_id = Uuid::new_v4();
        let snapshot = ExecutionStateSnapshot {
            placement_id,
            bookmaker: "pari".into(),
            phase: ExecutionStatePhase::PendingPlacement,
            placement_status: BetStatus::Pending,
            sequence: 1,
            updated_at: Utc::now(),
            last_action: ExecutionLedgerAction::Placed,
            last_error: None,
        };
        let transition = ExecutionStateTransition {
            placement_id,
            bookmaker: "pari".into(),
            from_phase: None,
            to_phase: ExecutionStatePhase::PendingPlacement,
            placement_status: BetStatus::Pending,
            sequence: 1,
            action: ExecutionLedgerAction::Placed,
            occurred_at: Utc::now(),
            error: None,
        };

        store.record_transition(&transition).await.unwrap();
        store.save_snapshot(&snapshot).await.unwrap();

        let snapshots = store.load_snapshots().await.unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].placement_id, placement_id);
        assert_eq!(store.transition_count().await, 1);
    }

    #[tokio::test]
    async fn state_snapshot_helpers_return_recent_first() {
        let store = ExecutionStateStore::new("memory").await.unwrap();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();

        store
            .save_snapshot(&ExecutionStateSnapshot {
                placement_id: first_id,
                bookmaker: "pari".into(),
                phase: ExecutionStatePhase::PendingPlacement,
                placement_status: BetStatus::Pending,
                sequence: 1,
                updated_at: Utc::now(),
                last_action: ExecutionLedgerAction::Placed,
                last_error: None,
            })
            .await
            .unwrap();
        store
            .record_transition(&ExecutionStateTransition {
                placement_id: first_id,
                bookmaker: "pari".into(),
                from_phase: None,
                to_phase: ExecutionStatePhase::PendingPlacement,
                placement_status: BetStatus::Pending,
                sequence: 1,
                action: ExecutionLedgerAction::Placed,
                occurred_at: Utc::now(),
                error: None,
            })
            .await
            .unwrap();
        store
            .save_snapshot(&ExecutionStateSnapshot {
                placement_id: second_id,
                bookmaker: "fonbet".into(),
                phase: ExecutionStatePhase::Settled,
                placement_status: BetStatus::Settled,
                sequence: 2,
                updated_at: Utc::now() + chrono::Duration::seconds(5),
                last_action: ExecutionLedgerAction::Updated,
                last_error: None,
            })
            .await
            .unwrap();

        let snapshots = store.load_state_snapshots().await;
        let transitions = store.load_transitions().await;

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].placement_id, second_id);
        assert_eq!(snapshots[1].placement_id, first_id);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].placement_id, first_id);
    }
}
